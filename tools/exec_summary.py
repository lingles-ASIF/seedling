#!/usr/bin/env python3
"""Plain-English executive summary of changes between two git refs.

    python3 tools/exec_summary.py                       # last v* tag (or first commit) .. HEAD
    python3 tools/exec_summary.py --from v0.1.0 --to HEAD
    python3 tools/exec_summary.py --report security/reports/latest.json
    python3 tools/exec_summary.py --changelog --version 0.2.0   # Keep a Changelog section

Works offline from git alone: it reads conventional-commit subjects
(feat/fix/perf/...), the Security-Gate trailers collected by tools/audit.py and,
when given, a gate report JSON. Sections: What changed for users, Security
posture (score delta), Risks / open items.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys

import seedling_config as sc
from audit import commits, last_tag, score_of

USER_FACING_TYPES = {"feat": "New", "fix": "Fixed", "perf": "Faster"}
INTERNAL_TYPES = {"docs": "documentation", "ci": "CI/automation", "build": "build", "chore": "maintenance",
                  "refactor": "refactoring (no behaviour change)", "test": "tests", "style": "formatting",
                  "security": "security hardening", "revert": "reverts"}
USER_PATH_GLOBS = ["**/src/**", "src/**", "web/**", "build.py", "seedling.html"]


def resolve_range(frm, to):
    if frm is None:
        frm = last_tag(to + "^") if sc.git("rev-parse", "-q", "--verify", to + "^") else None
    return frm, (f"{frm}..{to}" if frm else to)


def gate_score_at(ref: str | None) -> tuple[float | None, str | None]:
    """Most recent numeric Security-Gate trailer at or before ref."""
    if not ref:
        return None, None
    for c in reversed(commits(ref)):
        s = score_of(c["security_gate"])
        if s is not None:
            return s, c["short"]
    return None, None


def describe(c) -> str:
    d = re.sub(r"^[a-z]+(\([^)]*\))?!?:\s*", "", c["subject"])
    d = d[:1].upper() + d[1:]
    scope = f" ({c['scope']})" if c.get("scope") else ""
    return f"{d}{scope}"


def residual_risks() -> list[str]:
    rep = sc.ROOT / "security" / "REPORT.md"
    if not rep.exists():
        return []
    m = re.search(r"^## Residual risk.*?\n(.*?)(?=^## |\Z)", rep.read_text(encoding="utf-8"), re.M | re.S)
    return [l[2:].strip() for l in m.group(1).splitlines() if l.startswith("- ")] if m else []


def build(frm, to, report_path):
    frm, rng = resolve_range(frm, to)
    cs = commits(rng)
    user, internal, breaking, other_user = [], {}, [], []
    for c in cs:
        t = c["type"]
        if c["breaking"]:
            breaking.append(c)
        if t in USER_FACING_TYPES:
            user.append(c)
        elif t in INTERNAL_TYPES:
            internal[t] = internal.get(t, 0) + 1
        elif any(sc.glob_match(f["path"], g) for f in c["files"] for g in USER_PATH_GLOBS):
            other_user.append(c)
        else:
            internal["other"] = internal.get("other", 0) + 1

    report = None
    if report_path and pathlib.Path(report_path).exists():
        report = json.loads(pathlib.Path(report_path).read_text(encoding="utf-8"))
    end_score, end_src = (report["score"], "gate report " + report["git"]["head"][:10]) if report else gate_score_at(to)
    start_score, start_src = gate_score_at(frm)
    bypasses = [c for c in cs if "BYPASS" in c["security_gate"].upper()]
    ungated = [c for c in cs if c["gated_paths"] and score_of(c["security_gate"]) is None and c not in bypasses]

    title = f"Executive summary: {frm or 'project start'} to {to}"
    L = [f"## {title}", "",
         f"{len(cs)} change(s) recorded, touching {len({f['path'] for c in cs for f in c['files']})} file(s).", "",
         "### What changed for users", ""]
    if not user and not other_user:
        L.append("- No user-visible changes in this range.")
    for c in user:
        L.append(f"- **{USER_FACING_TYPES[c['type']]}:** {describe(c)} (`{c['short']}`)")
    for c in other_user:
        L.append(f"- {describe(c)} (`{c['short']}`)")
    for c in breaking:
        L.append(f"- **Breaking:** {describe(c)} (`{c['short']}`). Existing users may need to adapt.")
    if internal:
        parts = ", ".join(f"{n} {INTERNAL_TYPES.get(k, 'other internal')}" for k, n in sorted(internal.items()))
        L.append(f"- Behind the scenes: {parts} change(s), no direct effect on how the plant behaves.")

    L += ["", "### Security posture", ""]
    thr = float(sc.cfg("gate", "threshold"))
    if end_score is None:
        L.append(f"- No security gate result is recorded for `{to}` yet (threshold {thr:.0f}).")
    else:
        verdict = report["verdict"] if report else ("PASS" if end_score >= thr else "below threshold")
        L.append(f"- Current score: **{end_score:.1f}** (threshold {thr:.0f}, {verdict}; source: {end_src}).")
        if start_score is not None:
            delta = end_score - start_score
            word = "up" if delta > 0 else "down" if delta < 0 else "unchanged"
            L.append(f"- Change since `{frm}`: {word} {abs(delta):.1f} points (from {start_score:.1f} at `{start_src}`).")
        else:
            L.append("- No earlier score recorded, so no change can be computed.")
    if report:
        sk = report.get("skipped", [])
        L.append(f"- Checks: {sum(c['status'] == 'pass' for c in report['checks'])} passed, "
                 f"{sum(c['status'] == 'fail' for c in report['checks'])} failed, {len(sk)} skipped"
                 f"{' (' + ', '.join(sk) + ')' if sk else ''}.")

    L += ["", "### Risks and open items", ""]
    items = []
    for c in bypasses:
        items.append(f"Security gate was bypassed for `{c['short']}` ({c['subject']}).")
    for c in ungated:
        items.append(f"`{c['short']}` changed gated files without a recorded gate result "
                     "(hooks not installed, or committed elsewhere); CI re-checks it.")
    if report:
        for c in report["checks"]:
            if c["status"] == "fail":
                items.append(f"Failing check `{c['id']}`: {c['evidence'][:160]}")
            elif c["status"] == "skipped":
                items.append(f"Skipped check `{c['id']}`: {c['evidence'][:160]}")
            elif c["status"] == "info" and "missing" in c["evidence"]:
                items.append(f"{c['title']}: {c['evidence'][:160]}")
    for c in breaking:
        items.append(f"Breaking change in `{c['short']}`; check downstream users.")
    items += [f"Accepted residual risk: {r}" for r in residual_risks()]
    L += [f"- {i}" for i in items] or ["- None recorded."]
    return "\n".join(L) + "\n", cs


def changelog(cs, version):
    sections = {"Added": [], "Changed": [], "Fixed": [], "Removed": [], "Security": []}
    for c in cs:
        d = describe(c)
        if c["type"] == "security" or (c["scope"] or "").startswith("security"):
            sections["Security"].append(d)
        elif c["type"] == "feat":
            sections["Added"].append(d)
        elif c["type"] == "fix":
            sections["Fixed"].append(d)
        elif re.match(r"(?i)remove", d):
            sections["Removed"].append(d)
        elif c["type"] in ("perf", "refactor") or c["breaking"] or c["type"] is None:
            sections["Changed"].append(("**BREAKING:** " if c["breaking"] else "") + d)
    L = [f"## [{version}] - {dt.date.today().isoformat()}", ""]
    for name, items in sections.items():
        if items:
            L += [f"### {name}", ""] + [f"- {i}" for i in items] + [""]
    if len(L) == 2:
        L += ["- Maintenance release; no user-facing changes.", ""]
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--from", dest="frm", help="start ref (exclusive); default: last v* tag before --to")
    ap.add_argument("--to", default="HEAD")
    ap.add_argument("--report", help="gate report JSON for the current score")
    ap.add_argument("--changelog", action="store_true", help="print a Keep a Changelog section instead")
    ap.add_argument("--version", help="version for --changelog (default: crate version)")
    a = ap.parse_args()
    text, cs = build(a.frm, a.to, a.report)
    if a.changelog:
        version = a.version or sc.publish_package()["version"]
        print(changelog(cs, version))
    else:
        print(text)


if __name__ == "__main__":
    sys.exit(main())
