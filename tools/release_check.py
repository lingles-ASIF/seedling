#!/usr/bin/env python3
"""Pre-release checks used by .github/workflows/release.yml.

    python3 tools/release_check.py --tag v0.2.0            # verify, print summary
    python3 tools/release_check.py --tag v0.2.0 --github-output "$GITHUB_OUTPUT"

Verifies that the tag equals `v<version>` of the package being published
(discovered via cargo metadata, or [publish].package in governance/config.toml)
and that the manifest has the fields crates.io needs. Writes `package`,
`version`, `target` and `prev_tag` outputs for later steps.
"""
from __future__ import annotations

import argparse
import re
import sys

import seedling_config as sc


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", required=True)
    ap.add_argument("--github-output")
    ap.add_argument("--allow-mismatch", action="store_true", help="dry runs from a branch")
    a = ap.parse_args()
    pkg = sc.publish_package()
    errors = []
    tag = a.tag.removeprefix("refs/tags/")
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", tag) and not a.allow_mismatch:
        errors.append(f"tag {tag!r} is not v<MAJOR>.<MINOR>.<PATCH>")
    if tag != f"v{pkg['version']}" and not a.allow_mismatch:
        errors.append(f"tag {tag} does not match {pkg['name']} version {pkg['version']} (expected v{pkg['version']})")
    for field in sc.cfg("publish", "required_manifest_fields"):
        if not pkg.get(field) and not (field == "license" and pkg.get("license_file")):
            errors.append(f"Cargo.toml of {pkg['name']} is missing `{field}` (crates.io needs it)")
    if pkg.get("publish") == []:
        errors.append(f"{pkg['name']} has publish = false")
    prev = sc.run(["git", "describe", "--tags", "--abbrev=0", "--match", "v*", "HEAD^"]).stdout.strip()
    outputs = {"package": pkg["name"], "version": pkg["version"],
               "target": sc.cfg("publish", "publish_target"), "prev_tag": prev}
    for k, v in outputs.items():
        print(f"{k}={v}")
    if a.github_output:
        with open(a.github_output, "a", encoding="utf-8") as f:
            f.writelines(f"{k}={v}\n" for k, v in outputs.items())
    for e in errors:
        print(f"::error::{e}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
