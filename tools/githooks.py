#!/usr/bin/env python3
"""Logic behind the git hooks in tools/hooks/ (thin shell wrappers call this).

    pre-commit  : if staged paths match [gate].trigger_globs, run the fast gate;
                  block the commit on failure. SEEDLING_SKIP_GATE=1 bypasses it,
                  and the bypass is logged (local log + commit trailer).
    commit-msg  : add the Security-Gate trailer, then make sure the message ends
                  with exactly the configured [audit].trailers.
    post-commit : regenerate the audit log from git history (never commits).
"""
from __future__ import annotations

import datetime as dt
import json
import os
import pathlib
import re
import subprocess
import sys

import seedling_config as sc

TRAILER_RE = re.compile(r"^[A-Za-z][A-Za-z0-9-]*:\s*\S")


def git_dir() -> pathlib.Path:
    return pathlib.Path(sc.git("rev-parse", "--absolute-git-dir"))


def staged_paths() -> list[str]:
    out = sc.git("diff", "--cached", "--name-only", "--diff-filter=ACMRD")
    return [p for p in out.splitlines() if p]


def triggered(paths: list[str]) -> list[str]:
    globs = sc.cfg("gate", "trigger_globs")
    return [p for p in paths if any(sc.glob_match(p, g) for g in globs)]


def bypass() -> bool:
    return os.environ.get("SEEDLING_SKIP_GATE") == "1"


def log_bypass(paths):
    entry = {"time": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
             "head": sc.git("rev-parse", "HEAD"), "index_tree": sc.git("write-tree"), "paths": paths}
    with (git_dir() / "seedling-gate-bypass.jsonl").open("a", encoding="utf-8") as f:
        f.write(json.dumps(entry) + "\n")


def pre_commit() -> int:
    hits = triggered(staged_paths())
    if not hits:
        return 0
    if bypass():
        log_bypass(hits)
        print(f"seedling: SECURITY GATE BYPASSED (SEEDLING_SKIP_GATE=1) for {len(hits)} gated path(s); "
              "recorded in the commit trailer and .git/seedling-gate-bypass.jsonl", file=sys.stderr)
        return 0
    print(f"seedling: {len(hits)} gated path(s) staged ({', '.join(hits[:4])}{'...' if len(hits) > 4 else ''}); "
          "running the fast security gate", file=sys.stderr)
    p = subprocess.run([sys.executable, str(sc.ROOT / "security" / "gate.py"), "--hook"], cwd=sc.ROOT)
    if p.returncode != 0:
        print("seedling: commit BLOCKED by the security gate. See security/reports/latest.md. "
              "To bypass (logged): SEEDLING_SKIP_GATE=1 git commit ...", file=sys.stderr)
    return p.returncode


def gate_trailer_value() -> str | None:
    hits = triggered(staged_paths())
    if not hits:
        return None
    if bypass():
        return "BYPASSED (SEEDLING_SKIP_GATE=1)"
    tree = sc.git("write-tree")
    rep = sc.ROOT / sc.cfg("gate", "reports_dir") / "history" / f"tree-{tree}.json"
    if not rep.exists():
        return "no gate record for this tree"
    r = json.loads(rep.read_text(encoding="utf-8"))
    return f"{r['score']:.1f}/{r['threshold']:.0f} {r['verdict']} ({r['mode']}, tree {tree[:10]})"


def commit_msg(path: str) -> int:
    msgfile = pathlib.Path(path)
    raw = msgfile.read_text(encoding="utf-8")
    # Git strips '#' comment lines (and everything below a scissors line) after
    # this hook runs, so set them aside and put them back at the end untouched.
    scissors = re.search(r"^# -+ >8 -+$", raw, re.M)
    head, tail = (raw[:scissors.start()], raw[scissors.start():]) if scissors else (raw, "")
    lines = [l for l in head.splitlines() if not l.startswith("#")]
    comments = [l for l in head.splitlines() if l.startswith("#")] + ([tail.rstrip("\n")] if tail else [])
    body = "\n".join(lines).strip()
    if not body:
        return 0  # empty message: let git abort as usual
    required = list(sc.cfg("audit", "trailers"))
    gate_key = sc.cfg("audit", "gate_trailer")
    paras = re.split(r"\n\s*\n", body)
    trailer_block: list[str] = []
    if len(paras) > 1 and all(TRAILER_RE.match(l) for l in paras[-1].splitlines()):
        trailer_block = paras[-1].splitlines()
        paras = paras[:-1]
    kept = [t for t in trailer_block
            if t.strip() not in required and not t.lower().startswith(gate_key.lower() + ":")]
    existing_gate = [t for t in trailer_block if t.lower().startswith(gate_key.lower() + ":")]
    value = gate_trailer_value()
    gate_line = [f"{gate_key}: {value}"] if value else existing_gate
    out = "\n\n".join(paras).rstrip() + "\n\n" + "\n".join(kept + gate_line + required) + "\n"
    if comments:
        out += "\n" + "\n".join(comments) + "\n"
    if out != raw:
        msgfile.write_text(out, encoding="utf-8")
    return 0


def post_commit() -> int:
    # Never commit from here (that would loop); just refresh the files.
    p = subprocess.run([sys.executable, str(sc.ROOT / "tools" / "audit.py"), "rebuild", "--quiet"], cwd=sc.ROOT)
    if p.returncode == 0:
        print("seedling: audit log refreshed (governance/audit/); it will be included in your next commit",
              file=sys.stderr)
    return 0  # an audit hiccup must never make a finished commit look failed


def main() -> int:
    hook = sys.argv[1]
    if hook == "pre-commit":
        return pre_commit()
    if hook == "commit-msg":
        return commit_msg(sys.argv[2])
    if hook == "post-commit":
        return post_commit()
    print(f"unknown hook {hook}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
