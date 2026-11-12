#!/usr/bin/env python3
"""Seedling audit log: a deterministic record of every commit, rebuilt from git.

    python3 tools/audit.py rebuild            # regenerate governance/audit/* from git log
    python3 tools/audit.py summary [--since REF] [--until REF] [--markdown]

The log is a pure function of the git history (no wall-clock "generated at"
stamps), so any clone regenerates byte-identical files. The post-commit hook
calls `rebuild`; it never commits, so the refreshed files ride along with the
next commit and the log is always complete up to the previous commit.

Security scores come from the `Security-Gate:` trailer that the commit-msg hook
adds when the pre-commit gate ran. Email addresses are never written.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
from zoneinfo import ZoneInfo

import seedling_config as sc

TRAILER_RE = re.compile(r"^([A-Za-z][A-Za-z0-9-]*):\s*(.+)$")
CONVENTIONAL_RE = re.compile(r"^(?P<type>[a-z]+)(?:\((?P<scope>[^)]*)\))?(?P<bang>!)?:\s*(?P<desc>.+)$")
SEP_REC, SEP_FLD = "\x1e", "\x1f"


def strip_email(s: str) -> str:
    return re.sub(r"\s*<[^>]*>", "", s).strip()


def parse_trailers(body: str) -> list[tuple[str, str]]:
    paras = [p for p in re.split(r"\n\s*\n", body.strip()) if p.strip()]
    if len(paras) < 2:
        return []
    lines = paras[-1].splitlines()
    pairs = [TRAILER_RE.match(l.strip()) for l in lines]
    if not all(pairs):
        return []
    return [(m.group(1), m.group(2).strip()) for m in pairs]


def commits(rev_range: str | None = None) -> list[dict]:
    if not sc.git("rev-parse", "--verify", "-q", "HEAD"):
        return []
    fmt = SEP_REC + SEP_FLD.join(["%H", "%an", "%cI", "%s", "%B"]) + SEP_FLD
    args = ["log", "--reverse", "--no-merges", "--numstat", f"--format={fmt}"]
    args.append(rev_range or "HEAD")
    out = sc.run(["git", *args]).stdout
    tz = ZoneInfo(sc.cfg("project", "timezone"))
    triggers = sc.cfg("gate", "trigger_globs")
    result = []
    for rec in out.split(SEP_REC)[1:]:
        sha, author, cdate, subject, body, numstat = rec.split(SEP_FLD)
        files = []
        for line in numstat.strip().splitlines():
            parts = line.split("\t")
            if len(parts) == 3:
                add, dele, path = parts
                files.append({"path": path, "added": None if add == "-" else int(add),
                              "deleted": None if dele == "-" else int(dele)})
        trailers = parse_trailers(body)
        coauthors = [strip_email(v) for k, v in trailers if k.lower() == "co-authored-by"]
        gate = next((v for k, v in trailers if k.lower() == sc.cfg("audit", "gate_trailer").lower()), None)
        gated_paths = any(sc.glob_match(f["path"], g) for f in files for g in triggers)
        if gate is None:
            gate = "not run (no gated paths changed)" if not gated_paths else "no gate record"
        m = CONVENTIONAL_RE.match(subject)
        ts = dt.datetime.fromisoformat(cdate).astimezone(tz)
        result.append({
            "sha": sha, "short": sha[:10],
            "timestamp": ts.isoformat(), "timestamp_display": ts.strftime("%Y-%m-%d %H:%M:%S %Z"),
            "author": strip_email(author), "co_authors": coauthors, "subject": subject,
            "type": m.group("type") if m else None, "scope": m.group("scope") if m else None,
            "breaking": bool(m and m.group("bang")) or "BREAKING CHANGE" in body,
            "files": files,
            "added": sum(f["added"] or 0 for f in files), "deleted": sum(f["deleted"] or 0 for f in files),
            "security_gate": gate, "gated_paths": gated_paths,
            "session": next((v for k, v in trailers if k.lower() == "claude-session"), None),
        })
    return result


def score_of(gate_value: str) -> float | None:
    m = re.match(r"\s*([\d.]+)\s*/", gate_value or "")
    return float(m.group(1)) if m else None


def render_entry(c: dict, heading="###") -> list[str]:
    co = ", ".join(c["co_authors"]) or "none"
    L = [f"{heading} {c['timestamp_display']} `{c['short']}` {c['subject']}", "",
         f"- Commit: `{c['sha']}`",
         f"- Author: {c['author']}; co-author(s): {co}",
         f"- Security gate: {c['security_gate']}",
         f"- Changes: {len(c['files'])} file(s), +{c['added']} / -{c['deleted']}", ""]
    if c["files"]:
        L += ["  | File | + | - |", "  |---|---:|---:|"]
        for f in c["files"]:
            a = "bin" if f["added"] is None else f["added"]
            d = "bin" if f["deleted"] is None else f["deleted"]
            L.append(f"  | `{f['path']}` | {a} | {d} |")
        L.append("")
    return L


def rebuild(quiet=False):
    cs = commits()
    md_path = sc.ROOT / sc.cfg("audit", "log_md")
    js_path = sc.ROOT / sc.cfg("audit", "log_jsonl")
    md_path.parent.mkdir(parents=True, exist_ok=True)
    L = ["# Seedling audit log", "",
         "Every commit in this repository, oldest first, regenerated deterministically from `git log` by "
         "`python3 tools/audit.py rebuild` (run automatically by the post-commit hook). Do not edit by hand. "
         f"Timestamps are commit times in {sc.cfg('project', 'timezone')}. Because a hook may not create "
         "commits, this file is always complete up to the commit *before* the one that contains it.", "",
         "| # | When | Commit | Subject | Files | +/- | Security gate |", "|---:|---|---|---|---:|---|---|"]
    for i, c in enumerate(cs, 1):
        subj = c["subject"].replace("|", "\\|")
        L.append(f"| {i} | {c['timestamp_display']} | `{c['short']}` | {subj} | {len(c['files'])} | "
                 f"+{c['added']}/-{c['deleted']} | {c['security_gate']} |")
    L += ["", "## Entries", ""]
    for c in cs:
        L += render_entry(c)
    md_path.write_text("\n".join(L).rstrip() + "\n", encoding="utf-8")
    js_path.write_text("".join(json.dumps(c, sort_keys=True) + "\n" for c in cs), encoding="utf-8")
    if not quiet:
        print(f"audit: {len(cs)} commit(s) -> {md_path.relative_to(sc.ROOT)}, {js_path.relative_to(sc.ROOT)}")


def last_tag(before: str = "HEAD") -> str | None:
    t = sc.run(["git", "describe", "--tags", "--abbrev=0", "--match", "v*", before]).stdout.strip()
    return t or None


def summary(since=None, until="HEAD", markdown=False):
    if since is None and sc.git("rev-parse", "-q", "--verify", until + "^"):
        since = last_tag(until + "^")
    rng = f"{since}..{until}" if since else until
    cs = commits(rng)
    files = sorted({f["path"] for c in cs for f in c["files"]})
    add, dele = sum(c["added"] for c in cs), sum(c["deleted"] for c in cs)
    head = f"{len(cs)} commit(s) since {since or 'the first commit'}: {len(files)} file(s), +{add}/-{dele}"
    if markdown:
        L = [f"**{head}**", ""]
        if cs:
            L += ["| When | Commit | Subject | Author / co-author | Gate |", "|---|---|---|---|---|"]
            for c in reversed(cs):
                who = c["author"] + (" / " + ", ".join(c["co_authors"]) if c["co_authors"] else "")
                L.append(f"| {c['timestamp_display']} | `{c['short']}` | {c['subject'].replace('|', '/')} | {who} | {c['security_gate']} |")
        print("\n".join(L))
    else:
        print(head)
        for c in reversed(cs):
            print(f"  {c['timestamp_display']}  {c['short']}  {c['subject']}  (+{c['added']}/-{c['deleted']}, gate: {c['security_gate']})")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("rebuild", help="regenerate the audit log from git history")
    r.add_argument("--quiet", action="store_true")
    s = sub.add_parser("summary", help="print changes since a ref (default: last v* tag)")
    s.add_argument("--since")
    s.add_argument("--until", default="HEAD")
    s.add_argument("--markdown", action="store_true")
    a = ap.parse_args()
    if a.cmd == "rebuild":
        rebuild(a.quiet)
    else:
        summary(a.since, a.until, a.markdown)


if __name__ == "__main__":
    sys.exit(main())
