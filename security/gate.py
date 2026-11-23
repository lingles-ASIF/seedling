#!/usr/bin/env python3
"""Seedling security gate: the testing manager that stands between a change
and its publication.

    python3 security/gate.py --full          # everything (CI, releases)
    python3 security/gate.py --fast          # pre-commit: short fuzz, no browser
    python3 security/gate.py --frames 200000 # override fuzz length
    python3 security/gate.py --write-works-cited   # regenerate WORKS_CITED.md

Each check has an id, a weight, pass/fail/skipped, an evidence string and the
CWE / standard it addresses. Score = weighted passed / weighted applicable x 100
(skipped checks leave the denominator but are listed prominently). Exit 0 only
if the score is at least the threshold AND no critical check failed.

Build-std mode (no downloadable wasm32 std) is used only when the environment
variable SEEDLING_BUILD_STD=1 is set. Every setting lives in
governance/config.toml.

The gate never rewrites tracked files: it embeds the page, fuzzes and drives
the browser inside a scratch copy of the repo.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import platform
import re
import shutil
import sys
import tempfile
import tomllib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "tools"))
import seedling_config as sc  # noqa: E402

ROOT = sc.ROOT
ACCESSED = "2026-11-12"

# --------------------------------------------------------------------------
# Works cited: every URL below was opened and confirmed on the access date.
REFS = {
    "clippy-lints": ("Clippy Lints (full lint list, incl. undocumented_unsafe_blocks)",
                     "https://clippy.rust-lang.org/master/index.html"),
    "clippy-book": ("Clippy Documentation: lint groups (pedantic, nursery, ...)",
                    "https://doc.rust-lang.org/clippy/"),
    "rust-ub": ("The Rust Reference: Behavior considered undefined",
                "https://doc.rust-lang.org/reference/behavior-considered-undefined.html"),
    "nomicon-unsafe": ("The Rustonomicon: What Unsafe Rust Can Do",
                       "https://doc.rust-lang.org/nomicon/what-unsafe-does.html"),
    "cargo-profiles": ("The Cargo Book: Profiles (overflow-checks, panic)",
                       "https://doc.rust-lang.org/cargo/reference/profiles.html"),
    "cargo-test": ("The Cargo Book: cargo test", "https://doc.rust-lang.org/cargo/commands/cargo-test.html"),
    "cargo-publishing": ("The Cargo Book: Publishing on crates.io",
                         "https://doc.rust-lang.org/cargo/reference/publishing.html"),
    "rustsec": ("RustSec Advisory Database (and cargo-audit)", "https://rustsec.org/"),
    "cargo-deny": ("cargo-deny: lint your dependency graph",
                   "https://embarkstudios.github.io/cargo-deny/"),
    "cwe-20": ("MITRE CWE-20: Improper Input Validation",
               "https://cwe.mitre.org/data/definitions/20.html"),
    "cwe-79": ("MITRE CWE-79: Improper Neutralization of Input During Web Page Generation (XSS)",
               "https://cwe.mitre.org/data/definitions/79.html"),
    "cwe-190": ("MITRE CWE-190: Integer Overflow or Wraparound",
                "https://cwe.mitre.org/data/definitions/190.html"),
    "cwe-191": ("MITRE CWE-191: Integer Underflow (Wrap or Wraparound)",
                "https://cwe.mitre.org/data/definitions/191.html"),
    "cwe-758": ("MITRE CWE-758: Reliance on Undefined, Unspecified, or Implementation-Defined Behavior",
                "https://cwe.mitre.org/data/definitions/758.html"),
    "cwe-1104": ("MITRE CWE-1104: Use of Unmaintained Third Party Components",
                 "https://cwe.mitre.org/data/definitions/1104.html"),
    "csp3": ("W3C Content Security Policy Level 3 (Working Draft)",
             "https://www.w3.org/TR/CSP3/"),
    "mdn-wasm-unsafe-eval": ("MDN: CSP script-src, including 'wasm-unsafe-eval'",
                             "https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/script-src"),
    "owasp-csp": ("OWASP Cheat Sheet: Content Security Policy",
                  "https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html"),
    "wasm-spec": ("WebAssembly Specification (Core)", "https://webassembly.github.io/spec/core/"),
    "mdn-wasm-imports": ("MDN: WebAssembly.Module.imports()",
                         "https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Module/imports_static"),
    "owasp-fuzzing": ("OWASP Community: Fuzzing", "https://owasp.org/www-community/Fuzzing"),
    "owasp-asvs": ("OWASP Application Security Verification Standard (ASVS 5.0)",
                   "https://asvs.owasp.org/"),
    "scorecard": ("OpenSSF Scorecard (incl. License, Security-Policy, Fuzzing, SAST checks)",
                  "https://scorecard.dev/"),
    "scorecard-checks": ("OpenSSF Scorecard checks documentation",
                         "https://github.com/ossf/scorecard"),
    "gh-pvr": ("GitHub Docs: Configuring private vulnerability reporting for a repository",
               "https://docs.github.com/en/code-security/security-advisories/working-with-repository-security-advisories/configuring-private-vulnerability-reporting-for-a-repository"),
    "gh-secure-use": ("GitHub Docs: Secure use reference (pin actions to commit SHAs)",
                      "https://docs.github.com/en/actions/reference/security/secure-use"),
    "cratesio-tp-blog": ("Rust Blog: crates.io development update (Trusted Publishing), 2025-07-11",
                         "https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07"),
    "cratesio-tp-rfc": ("RFC 3691: Trusted Publishing for crates.io",
                        "https://rust-lang.github.io/rfcs/3691-trusted-publishing-cratesio.html"),
    "cratesio-auth-action": ("rust-lang/crates-io-auth-action",
                             "https://github.com/rust-lang/crates-io-auth-action"),
    "cratesio-policy": ("RFC 3463: crates.io policy update (API User-Agent and rate limit)",
                        "https://rust-lang.github.io/rfcs/3463-crates-io-policy-update.html"),
}

# id -> (title, critical, standard/CWE, refs)
CHECKS = {
    "build.wasm": ("Release build to wasm32 succeeds", True, "build integrity", ["cargo-profiles"]),
    "tests.unit": ("Library unit and doc tests pass (host)", True, "CWE-20 (behaviour incl. hostile input)",
                   ["cargo-test", "cwe-20", "owasp-asvs"]),
    "wasm.imports_empty": ("WebAssembly module imports nothing from the host", True,
                           "least privilege; WebAssembly JS API", ["wasm-spec", "mdn-wasm-imports"]),
    "clippy.default": ("Clippy default lints, -D warnings", False, "CWE-758 (static analysis)",
                       ["clippy-book", "clippy-lints", "cwe-758"]),
    "clippy.pedantic": ("Clippy pedantic, -D warnings", False, "CWE-190/191 (cast & arithmetic lints)",
                        ["clippy-book", "clippy-lints", "cwe-190", "cwe-191"]),
    "clippy.nursery": ("Clippy nursery, -D warnings", False, "static analysis (experimental lints)",
                       ["clippy-book", "clippy-lints"]),
    "clippy.undocumented_unsafe": ("clippy::undocumented_unsafe_blocks denied", True, "CWE-758",
                                   ["clippy-lints", "nomicon-unsafe", "rust-ub", "cwe-758"]),
    "unsafe.safety_comments": ("Every unsafe block/impl carries a // SAFETY: comment", True, "CWE-758",
                               ["nomicon-unsafe", "rust-ub", "cwe-758"]),
    "supply.audit": ("No vulnerable dependencies (zero deps, or cargo-audit clean)", False, "CWE-1104",
                     ["rustsec", "cwe-1104", "scorecard"]),
    "supply.deny": ("Dependency policy (zero deps, or cargo-deny clean)", False, "CWE-1104",
                    ["cargo-deny", "cwe-1104"]),
    "profile.overflow_checks": ("Release profile: overflow-checks = true", False, "CWE-190 / CWE-191",
                                ["cargo-profiles", "cwe-190", "cwe-191"]),
    "profile.panic_abort": ('Release profile: panic = "abort"', False, "CWE-758 (no unwinding across the wasm boundary)",
                            ["cargo-profiles", "cwe-758"]),
    "fuzz.harness": ("Fuzz harness over every export passes", True, "CWE-20; OWASP fuzzing",
                     ["owasp-fuzzing", "cwe-20", "cwe-190", "cwe-191", "owasp-asvs"]),
    "browser.check": ("Headless browser behaviour + CSP enforcement probes", False, "CWE-79; OWASP ASVS",
                      ["csp3", "cwe-79", "owasp-asvs"]),
    "csp.policy": ("Built page carries a strict Content-Security-Policy", True, "CWE-79; W3C CSP3",
                   ["csp3", "mdn-wasm-unsafe-eval", "owasp-csp", "cwe-79"]),
    "hygiene.files": ("Repository hygiene files present", False, "OpenSSF Scorecard License / Security-Policy",
                      ["scorecard", "scorecard-checks", "gh-pvr"]),
    # Informational (weight 0): reported, never scored.
    "info.html_in_sync": ("Committed page matches a fresh build", False, "release reproducibility", []),
    "info.publish_ready": ("Crate manifest has the fields crates.io needs", False, "Cargo Book: publishing",
                           ["cargo-publishing"]),
}



ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")  # terminal colour/style codes

class Gate:
    def __init__(self, mode: str, frames: int, browser: bool, hook: bool, artifact_dir: str | None = None):
        self.mode, self.frames, self.browser, self.hook = mode, frames, browser, hook
        self.artifact_dir = artifact_dir
        self.results: dict[str, dict] = {}
        self.weights = sc.cfg("gate", "weights", {})
        self.stage: pathlib.Path | None = None
        self.wasm: list[pathlib.Path] = []
        self.html: str | None = None
        self.args, self.env = sc.cargo_target_args()

    # ------------------------------------------------------------ recording
    def record(self, cid, status, evidence, **extra):
        title, critical, std, refs = CHECKS[cid]
        weight = 0 if cid.startswith("info.") else float(self.weights.get(cid, 1))
        self.results[cid] = dict(id=cid, title=title, status=status, weight=weight,
                                 critical=critical, standard=std, refs=refs,
                                 evidence=evidence.strip()[:600], **extra)
        mark = {"pass": "PASS", "fail": "FAIL", "skipped": "SKIP", "info": "INFO"}[status]
        print(f"  {mark:4}  {cid:28} {evidence.strip().splitlines()[0][:110] if evidence.strip() else ''}",
              flush=True)

    # ------------------------------------------------------------ checks
    def check_build(self):
        cmd = ["cargo", "build", "--release", "--workspace", *self.args]
        p = sc.run(cmd, env=self.env, timeout=1200)
        self.wasm = [w for w in sc.wasm_files() if w.read_bytes()[:4] == b"\0asm"]
        limit = int(sc.cfg("gate", "max_wasm_bytes", 0) or 0)
        too_big = [w for w in self.wasm if limit and w.stat().st_size > limit]
        if p.returncode == 0 and self.wasm and not too_big:
            sizes = ", ".join(f"{w.name} ({w.stat().st_size:,} B)" for w in self.wasm)
            self.record("build.wasm", "pass", f"{' '.join(cmd)} -> {sizes}" + (f"; limit {limit:,} B" if limit else ""))
        elif p.returncode == 0 and too_big:
            sizes = ", ".join(f"{w.name} ({w.stat().st_size:,} B)" for w in too_big)
            self.record("build.wasm", "fail", f"module over the {limit:,} B limit: {sizes}. State has probably moved "
                        "into the data section; keep blank state all-zero (see governance/config.toml max_wasm_bytes)")
        else:
            tail = (p.stderr or p.stdout).strip().splitlines()[-6:]
            self.record("build.wasm", "fail", f"exit {p.returncode}; wasm found: {len(self.wasm)}; " + " | ".join(tail))

    def clippy(self, cid, lint_args):
        cmd = ["cargo", "clippy", "--release", "--workspace", *self.args, "--", *lint_args]
        p = sc.run(cmd, env={**(self.env or {}), "CARGO_TERM_COLOR": "never"}, timeout=1200)
        # Strip ANSI colour codes too: CI sets CARGO_TERM_COLOR=always, and a
        # coloured "error" line would otherwise go uncounted ("0 diagnostics").
        out = ANSI.sub("", p.stderr + p.stdout)
        diags = re.findall(r"^(warning|error)(?:\[\S+\])?: (.*)$", out, re.M)
        diags = [d for d in diags if not re.match(r"(could not compile|aborting due|\d+ warnings? emitted|build failed)", d[1])]
        flags = " ".join(lint_args)
        if p.returncode == 0 and not diags:
            self.record(cid, "pass", f"`cargo clippy -- {flags}`: 0 diagnostics")
        else:
            tail = out.strip().splitlines()[-1] if out.strip() else ""
            first = "; ".join(f"{k}: {m}" for k, m in diags[:3]) or tail
            self.record(cid, "fail", f"`cargo clippy -- {flags}`: exit {p.returncode}, {len(diags)} diagnostics. {first}")

    def check_tests(self):
        """Run the published library's tests natively (no wasm target, no
        build-std): the engine is platform-independent by design."""
        try:
            pkg = sc.publish_package()["name"]
        except RuntimeError as e:
            return self.record("tests.unit", "fail", str(e))
        cmd = ["cargo", "test", "-p", pkg]
        p = sc.run(cmd, env={"CARGO_TERM_COLOR": "never"}, timeout=1200)
        out = ANSI.sub("", p.stdout + p.stderr)
        runs = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed", out)
        passed = sum(int(r[1]) for r in runs)
        failed = sum(int(r[2]) for r in runs)
        if p.returncode == 0 and runs and failed == 0:
            self.record("tests.unit", "pass", f"`{' '.join(cmd)}`: {passed} passed, 0 failed")
        else:
            bad = [ln for ln in out.splitlines() if "FAILED" in ln or "panicked" in ln or ln.startswith("error")][:3]
            self.record("tests.unit", "fail", f"`{' '.join(cmd)}`: exit {p.returncode}, {passed} passed, {failed} failed. " + " | ".join(bad))

    def check_unsafe(self):
        """Count unsafe blocks / fns / impls; every block and impl needs a
        `// SAFETY:` comment on the same line or in the comment run directly
        above it (the rule clippy::undocumented_unsafe_blocks enforces)."""
        counts = {"block": 0, "fn": 0, "impl": 0, "extern": 0, "attribute": 0}
        missing, unsafe_fns_without_doc = [], []
        for f in sc.rust_sources():
            lines = f.read_text(encoding="utf-8").splitlines()
            code = _strip_comments_and_strings(lines)
            rel = f.relative_to(ROOT)
            for i, line in enumerate(code):
                for m in re.finditer(r"\bunsafe\b", line):
                    before, after = line[:m.start()], line[m.end():].lstrip()
                    if re.search(r"#!?\[\s*$", before) or after.startswith("("):
                        counts["attribute"] += 1  # #[unsafe(no_mangle)] etc.
                        continue
                    kind = ("fn" if re.match(r"(extern\s+\"[^\"]*\"\s+)?fn\b", after) else
                            "impl" if after.startswith("impl") else
                            "extern" if after.startswith("extern") else
                            "trait" if after.startswith("trait") else "block")
                    if kind == "trait":
                        continue
                    counts[kind] += 1
                    if kind == "fn":
                        doc = _comment_run_above(lines, i)
                        if "# Safety" not in doc:
                            unsafe_fns_without_doc.append(f"{rel}:{i + 1}")
                        continue
                    if "SAFETY:" in lines[i] or "SAFETY:" in _comment_run_above(lines, i):
                        continue
                    missing.append(f"{rel}:{i + 1}")
        summary = (f"{counts['block']} unsafe block(s), {counts['fn']} unsafe fn(s), "
                   f"{counts['impl']} unsafe impl(s), {counts['extern']} unsafe extern block(s), "
                   f"{counts['attribute']} unsafe attribute(s) across {len(sc.rust_sources())} file(s)")
        if unsafe_fns_without_doc:
            summary += f"; unsafe fn without `# Safety` doc: {', '.join(unsafe_fns_without_doc)}"
        if missing or unsafe_fns_without_doc:
            self.record("unsafe.safety_comments", "fail", f"{summary}; missing SAFETY: {', '.join(missing) or 'none'}",
                        counts=counts)
        else:
            self.record("unsafe.safety_comments", "pass", summary + "; all documented", counts=counts)

    def check_imports(self):
        if not self.wasm:
            return self.record("wasm.imports_empty", "fail", "no .wasm to inspect (build failed)")
        js = ("const fs=require('fs');const out={};for(const f of process.argv.slice(1)){"
              "const m=new WebAssembly.Module(fs.readFileSync(f));"
              "out[f]={imports:WebAssembly.Module.imports(m),exports:WebAssembly.Module.exports(m)};}"
              "console.log(JSON.stringify(out));")
        p = sc.run(["node", "-e", js, *map(str, self.wasm)])
        if p.returncode != 0:
            return self.record("wasm.imports_empty", "fail", "node could not load the module: " + p.stderr[-300:])
        data = json.loads(p.stdout)
        bad, parts = [], []
        for f, d in data.items():
            name = pathlib.Path(f).name
            exports = ", ".join(f"{e['name']}({e['kind']})" for e in d["exports"])
            parts.append(f"{name}: {len(d['imports'])} imports; exports: {exports}")
            bad += [f"{name}:{i['module']}.{i['name']}" for i in d["imports"]]
        status = "fail" if bad else "pass"
        self.record("wasm.imports_empty", status, ("imports found: " + ", ".join(bad) + ". " if bad else "") + " | ".join(parts),
                    modules={pathlib.Path(k).name: v for k, v in data.items()})

    def check_supply_chain(self):
        lock = sc.workspace_root() / "Cargo.lock"
        third = []
        if lock.exists():
            data = tomllib.loads(lock.read_text(encoding="utf-8"))
            third = [f"{p['name']} {p['version']}" for p in data.get("package", []) if "source" in p]
        if not third:
            ev = "Cargo.lock lists 0 third-party packages (only workspace members): nothing to audit"
            self.record("supply.audit", "pass", ev)
            self.record("supply.deny", "pass", ev)
            return
        n = f"{len(third)} third-party package(s): {', '.join(third[:8])}{' ...' if len(third) > 8 else ''}"
        for cid, tool, cmd in (("supply.audit", "cargo-audit", ["cargo", "audit", "--deny", "warnings"]),
                               ("supply.deny", "cargo-deny", ["cargo", "deny", "check"])):
            if not shutil.which(tool):
                self.record(cid, "skipped", f"{n}; {tool} is not installed (install with `cargo install --locked {tool}`)")
                continue
            p = sc.run(cmd, timeout=600)
            tail = " | ".join((p.stderr + p.stdout).strip().splitlines()[-4:])
            self.record(cid, "pass" if p.returncode == 0 else "fail", f"{n}; `{' '.join(cmd)}` exit {p.returncode}: {tail}")

    def check_profile(self):
        manifest = sc.workspace_root() / "Cargo.toml"
        rel = tomllib.loads(manifest.read_text(encoding="utf-8")).get("profile", {}).get("release", {})
        oc, pa = rel.get("overflow-checks"), rel.get("panic")
        self.record("profile.overflow_checks", "pass" if oc is True else "fail",
                    f"[profile.release] overflow-checks = {json.dumps(oc)} in {manifest.name}")
        self.record("profile.panic_abort", "pass" if pa == "abort" else "fail",
                    f"[profile.release] panic = {json.dumps(pa)} in {manifest.name}")

    def make_stage(self):
        """Scratch copy: the page is embedded and tested here, never in place."""
        self.stage = pathlib.Path(tempfile.mkdtemp(prefix="seedling-gate-"))
        pats = list(sc.cfg("build", "stage_paths")) + [sc.cfg("build", "wasm_glob")]
        for pat in pats:
            for src in ROOT.glob(pat):
                if src.is_file():
                    dst = self.stage / src.relative_to(ROOT)
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(src, dst)
        p = sc.run(list(sc.cfg("build", "embed_command")), cwd=self.stage, timeout=300)
        out = self.stage / sc.cfg("build", "html_artifact")
        if p.returncode == 0 and out.exists():
            self.html = out.read_text(encoding="utf-8")
            if self.artifact_dir:  # release: ship exactly the page that was tested
                dest = pathlib.Path(self.artifact_dir)
                dest.mkdir(parents=True, exist_ok=True)
                shutil.copy2(out, dest / out.name)
        return p

    def check_csp(self, embed_proc):
        if self.html is None:
            return self.record("csp.policy", "fail", "page could not be built: " +
                               (embed_proc.stderr or embed_proc.stdout).strip()[-300:])
        m = re.search(r'<meta\s+http-equiv="Content-Security-Policy"\s+content="([^"]*)"', self.html, re.I)
        if not m:
            return self.record("csp.policy", "fail", "no Content-Security-Policy <meta> in built page")
        csp = m.group(1)
        directives = [d.strip() for d in csp.split(";")]
        missing = [r for r in sc.cfg("gate", "csp_required") if r not in directives]
        forbidden = [f for f in sc.cfg("gate", "csp_forbidden") if f in csp]
        ok = not missing and not forbidden
        ev = (f"directives: {len(directives)}; required present: {not missing}"
              f"{' (missing ' + ', '.join(missing) + ')' if missing else ''}; "
              f"forbidden keywords: {', '.join(forbidden) or 'none'}; policy: {csp[:300]}")
        self.record("csp.policy", "pass" if ok else "fail", ev, policy=csp)

    def check_fuzz(self):
        if not self.wasm or self.stage is None:
            return self.record("fuzz.harness", "fail", "no .wasm to fuzz (build failed)")
        script = self.stage / sc.cfg("gate", "fuzz_script")
        if not script.exists():
            return self.record("fuzz.harness", "fail", f"{sc.cfg('gate', 'fuzz_script')} not found")
        p = sc.run(["node", str(script.relative_to(self.stage)), str(self.frames)], cwd=self.stage, timeout=3600)
        out = (p.stdout + p.stderr).strip()
        cap = re.search(r"capacity: peak draw buffer ([\d,]+) of ([\d,]+) floats", out)
        lines = [l for l in out.splitlines() if l.startswith(("PASS", "FAIL", "fuzzed", "capacity"))]
        ev = f"{self.frames:,} frames ({self.mode} mode); " + " | ".join(lines or out.splitlines()[-3:])
        extra = {}
        if cap:
            extra["capacity_peak"] = int(cap.group(1).replace(",", ""))
            extra["capacity_limit"] = int(cap.group(2).replace(",", ""))
        self.record("fuzz.harness", "pass" if p.returncode == 0 else "fail", ev, **extra)

    def check_browser(self):
        if not self.browser:
            return self.record("browser.check", "skipped", f"not run in {self.mode} mode (runs with --full)")
        if self.html is None:
            return self.record("browser.check", "fail", "page could not be built")
        probe = sc.run([sys.executable, "-c",
                        "from playwright.sync_api import sync_playwright\n"
                        "with sync_playwright() as p:\n b=p.chromium.launch(); b.close()"], timeout=120)
        if probe.returncode != 0:
            why = probe.stderr.strip().splitlines()[-1:] or ["unknown"]
            return self.record("browser.check", "skipped",
                               "Playwright/Chromium unavailable (pip install playwright && "
                               f"python -m playwright install chromium): {why[0][:200]}")
        script = self.stage / sc.cfg("gate", "browser_script")
        p = sc.run([sys.executable, str(script.relative_to(self.stage))], cwd=self.stage, timeout=600)
        lines = [l for l in p.stdout.splitlines() if l.startswith(("PASS", "FAIL"))]
        npass = sum(l.startswith("PASS") for l in lines)
        ev = f"{npass}/{len(lines)} probes pass: " + " | ".join(lines)
        if p.returncode != 0 and not lines:
            ev = "browser check crashed: " + (p.stderr.strip().splitlines() or ["?"])[-1]
        self.record("browser.check", "pass" if p.returncode == 0 else "fail", ev)

    def check_hygiene(self):
        req = sc.cfg("gate", "required_files")
        missing = [f for f in req if not (ROOT / f).is_file()]
        self.record("hygiene.files", "fail" if missing else "pass",
                    f"missing: {', '.join(missing)}" if missing else f"present: {', '.join(req)}")

    def check_info(self):
        committed = ROOT / sc.cfg("build", "html_artifact")
        if self.html is not None and committed.exists():
            same = committed.read_text(encoding="utf-8") == self.html
            mode = " (build-std toolchain may emit different bytes)" if sc.build_std() and not same else ""
            self.record("info.html_in_sync", "info",
                        ("identical to fresh build" if same else "differs from fresh build: run `python3 build.py`") + mode)
        try:
            pkg = sc.publish_package()
            missing = [f for f in sc.cfg("publish", "required_manifest_fields")
                       if not pkg.get(f) and not (f == "license" and pkg.get("license_file"))]
            self.record("info.publish_ready", "info",
                        f"package `{pkg['name']}` {pkg['version']}: " +
                        (f"missing {', '.join(missing)} (release.yml will refuse to publish)" if missing else "ready"))
        except RuntimeError as e:
            self.record("info.publish_ready", "info", str(e))

    # ------------------------------------------------------------ driver
    def run(self):
        print(f"Seedling security gate ({self.mode} mode, build-std={'on' if sc.build_std() else 'off'})", flush=True)
        try:
            self.check_build()
            self.check_tests()
            self.clippy("clippy.default", ["-D", "warnings"])
            self.clippy("clippy.pedantic", ["-D", "warnings", "-D", "clippy::pedantic"])
            self.clippy("clippy.nursery", ["-D", "warnings", "-D", "clippy::nursery"])
            self.clippy("clippy.undocumented_unsafe", ["-D", "clippy::undocumented_unsafe_blocks"])
            self.check_unsafe()
            self.check_imports()
            self.check_supply_chain()
            self.check_profile()
            embed = self.make_stage()
            self.check_csp(embed)
            self.check_fuzz()
            self.check_browser()
            self.check_hygiene()
            self.check_info()
        finally:
            if self.stage:
                shutil.rmtree(self.stage, ignore_errors=True)
        return self.finish()

    def finish(self):
        scored = [r for r in self.results.values() if r["status"] in ("pass", "fail") and r["weight"] > 0]
        total = sum(r["weight"] for r in scored)
        got = sum(r["weight"] for r in scored if r["status"] == "pass")
        score = round(100.0 * got / total, 1) if total else 0.0
        threshold = float(sc.cfg("gate", "threshold"))
        crit = [r["id"] for r in self.results.values() if r["critical"] and r["status"] == "fail"]
        passed = score >= threshold and not crit
        report = dict(
            tool="seedling-security-gate", schema=1, mode=self.mode, fuzz_frames=self.frames,
            build_std=sc.build_std(), score=score, threshold=threshold,
            verdict="PASS" if passed else "FAIL", critical_failures=crit,
            weighted_passed=got, weighted_applicable=total,
            skipped=[r["id"] for r in self.results.values() if r["status"] == "skipped"],
            checks=list(self.results.values()), environment=environment(), git=git_state(self.hook),
            timestamp=dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
            accessed=ACCESSED,
        )
        write_reports(report)
        print(f"\nScore {score:.1f} / threshold {threshold:.0f}: {report['verdict']}"
              + (f" (critical failures: {', '.join(crit)})" if crit else ""))
        if report["skipped"]:
            print("Skipped (excluded from score): " + ", ".join(report["skipped"]))
        return 0 if passed else 1


# ------------------------------------------------------------------ helpers

def _strip_comments_and_strings(lines):
    """Blank out comments and string/char literals, keeping line structure."""
    out, in_block = [], 0
    for line in lines:
        res, i = [], 0
        while i < len(line):
            two = line[i:i + 2]
            if in_block:
                if two == "*/":
                    in_block -= 1; i += 2
                elif two == "/*":
                    in_block += 1; i += 2
                else:
                    i += 1
                continue
            if two == "//":
                break
            if two == "/*":
                in_block += 1; i += 2; continue
            if line[i] == '"':
                j = i + 1
                while j < len(line) and line[j] != '"':
                    j += 2 if line[j] == "\\" else 1
                res.append('""'); i = j + 1; continue
            res.append(line[i]); i += 1
        out.append("".join(res))
    return out


def _comment_run_above(lines, i):
    """The contiguous run of comment/attribute lines directly above line i.
    For unsafe blocks inside a statement, clippy accepts the comment above the
    statement, so we also walk up through the statement's own first line."""
    text, j = [], i - 1
    while j >= 0:
        s = lines[j].strip()
        if s.startswith(("//", "/*", "*", "#[")) or (s.endswith("*/")):
            text.append(s); j -= 1
        else:
            break
    return "\n".join(reversed(text))


def environment():
    def ver(cmd):
        p = sc.run(cmd)
        return (p.stdout or p.stderr).strip().splitlines()[0] if p.returncode == 0 else "unavailable"
    return {
        "rustc": ver(["rustc", "--version"]), "cargo": ver(["cargo", "--version"]),
        "clippy": ver(["cargo", "clippy", "--version"]), "node": ver(["node", "--version"]),
        "python": platform.python_version(), "os": platform.platform(),
        "playwright": ver([sys.executable, "-m", "playwright", "--version"]),
    }


def git_state(hook: bool):
    head = sc.git("rev-parse", "HEAD") or "unborn"
    dirty = bool(sc.git("status", "--porcelain", "--untracked-files=no"))
    st = {"head": head, "branch": sc.git("rev-parse", "--abbrev-ref", "HEAD"), "dirty": dirty}
    if hook:  # tree id of what is about to be committed (pre-commit)
        st["index_tree"] = sc.git("write-tree")
    return st


def write_reports(report):
    d = ROOT / sc.cfg("gate", "reports_dir")
    hist = d / "history"
    hist.mkdir(parents=True, exist_ok=True)
    md = render_markdown(report)
    js = json.dumps(report, indent=2)
    (d / "latest.json").write_text(js + "\n", encoding="utf-8")
    (d / "latest.md").write_text(md, encoding="utf-8")
    g = report["git"]
    name = g["head"] + ("-dirty" if g["dirty"] else "")
    for stem in [name] + ([f"tree-{g['index_tree']}"] if g.get("index_tree") else []):
        (hist / f"{stem}.json").write_text(js + "\n", encoding="utf-8")
        (hist / f"{stem}.md").write_text(md, encoding="utf-8")


def _cell(s):
    return str(s).replace("|", "\\|").replace("\n", " ")


def render_markdown(r):
    icon = {"pass": "pass", "fail": "**FAIL**", "skipped": "skipped", "info": "info"}
    L = [f"# Seedling security gate report", "",
         f"**Verdict: {r['verdict']}**  Score **{r['score']:.1f}** / threshold {r['threshold']:.0f} "
         f"({r['weighted_passed']:g} of {r['weighted_applicable']:g} weighted points)  ",
         f"Mode: {r['mode']} (fuzz {r['fuzz_frames']:,} frames; build-std {'on' if r['build_std'] else 'off'})  ",
         f"Commit: `{r['git']['head']}`{' (working tree dirty)' if r['git']['dirty'] else ''} on `{r['git']['branch']}`  ",
         f"Generated: {r['timestamp']}", ""]
    if r["critical_failures"]:
        L += [f"**Critical failures:** {', '.join(r['critical_failures'])}", ""]
    if r["skipped"]:
        L += ["> **Skipped checks (excluded from the score denominator):**"]
        for c in r["checks"]:
            if c["status"] == "skipped":
                L.append(f"> - `{c['id']}`: {_cell(c['evidence'])}")
        L.append("")
    L += ["## Checks", "", "| Result | Check | Weight | Critical | Standard / CWE | Evidence |",
          "|---|---|---:|:---:|---|---|"]
    for c in r["checks"]:
        L.append(f"| {icon[c['status']]} | `{c['id']}` {c['title']} | {c['weight']:g} | "
                 f"{'yes' if c['critical'] else ''} | {_cell(c['standard'])} | {_cell(c['evidence'][:400])} |")
    e = r["environment"]
    L += ["", "## Environment", ""] + [f"- {k}: {v}" for k, v in e.items()]
    L += ["", "## How the score works", "",
          "Score = weighted passed / weighted applicable x 100. Skipped checks are excluded from the "
          "denominator; `info` rows are never scored. The gate passes only when the score is at least the "
          "threshold and no check marked critical failed. This is a project-internal measure, not an "
          "industry rating or certification.", "",
          "## Works Cited", "", f"All sources accessed {r['accessed']}. Full annotated list: `security/WORKS_CITED.md`.", ""]
    for c in r["checks"]:
        if c["refs"]:
            refs = "; ".join(f"[{REFS[k][0]}]({REFS[k][1]})" for k in c["refs"])
            L.append(f"- `{c['id']}`: {refs}")
    return "\n".join(L) + "\n"


def write_works_cited():
    used = {}
    for cid, (_, _, _, refs) in CHECKS.items():
        for k in refs:
            used.setdefault(k, []).append(cid)
    L = ["# Works cited", "",
         f"Authoritative background for every check in `security/gate.py` and for the release process. "
         f"Every URL was opened and confirmed on **{ACCESSED}** (access date). Regenerate with "
         "`python3 security/gate.py --write-works-cited`.", "",
         "| Source | URL | Supports |", "|---|---|---|"]
    for k, (title, url) in REFS.items():
        sup = ", ".join(f"`{c}`" for c in used.get(k, [])) or "release / governance process"
        L.append(f"| {title} | <{url}> | {sup} |")
    L += ["", "## Notes", "",
          "- CWE-20 is marked by MITRE as discouraged for mapping specific vulnerabilities; it is cited "
          "here as the class the fuzz harness exercises (hostile NaN/Infinity/huge inputs at every export).",
          "- `'wasm-unsafe-eval'` is required for the page to compile its embedded WebAssembly; it permits "
          "WebAssembly compilation only, unlike `'unsafe-eval'`, which the gate forbids (MDN script-src).",
          "- The CSP3 document is a W3C Working Draft (version of 27 October 2026 at the access date).",
          "- crates.io requires API clients to send a User-Agent that identifies the application and to "
          "stay at or below 1 request per second (RFC 3463); `tools/tracker.py` complies.",
          "- Trusted Publishing: the first release of a crate must be published manually; later releases "
          "can use `rust-lang/crates-io-auth-action@v1` from GitHub Actions (Rust Blog, 2025-07-11).",
          "- GitHub recommends pinning actions to full commit SHAs (Secure use reference); the workflows "
          "pin to major versions and say so in comments.",
          "- The OWASP fuzzing URL redirects to community.owasp.org/Fuzzing; the Clippy lint list formerly at "
          "rust-lang.github.io/rust-clippy now lives at clippy.rust-lang.org.", ""]
    (ROOT / "security" / "WORKS_CITED.md").write_text("\n".join(L), encoding="utf-8")
    print("wrote security/WORKS_CITED.md")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--full", action="store_true", help="all checks, long fuzz, browser (default)")
    g.add_argument("--fast", action="store_true", help="short fuzz, no browser (pre-commit)")
    ap.add_argument("--frames", type=int, help="override fuzz frame count")
    ap.add_argument("--no-browser", action="store_true", help="skip the Playwright check")
    ap.add_argument("--hook", action="store_true", help="pre-commit mode: implies --fast, records index tree")
    ap.add_argument("--artifact-dir", help="copy the tested page here (used by release.yml)")
    ap.add_argument("--write-works-cited", action="store_true")
    a = ap.parse_args()
    if a.write_works_cited:
        return write_works_cited()
    fast = a.fast or a.hook
    mode = "fast" if fast else "full"
    frames = a.frames or int(sc.cfg("gate", "fuzz_frames_fast" if fast else "fuzz_frames_full"))
    return Gate(mode, frames, browser=not (fast or a.no_browser), hook=a.hook,
                artifact_dir=a.artifact_dir).run()


if __name__ == "__main__":
    sys.exit(main() or 0)
