# Works cited

Authoritative background for every check in `security/gate.py` and for the release process. Every URL was opened and confirmed on **2026-11-12** (access date). Regenerate with `python3 security/gate.py --write-works-cited`.

| Source | URL | Supports |
|---|---|---|
| Clippy Lints (full lint list, incl. undocumented_unsafe_blocks) | <https://clippy.rust-lang.org/master/index.html> | `clippy.default`, `clippy.pedantic`, `clippy.nursery`, `clippy.undocumented_unsafe` |
| Clippy Documentation: lint groups (pedantic, nursery, ...) | <https://doc.rust-lang.org/clippy/> | `clippy.default`, `clippy.pedantic`, `clippy.nursery` |
| The Rust Reference: Behavior considered undefined | <https://doc.rust-lang.org/reference/behavior-considered-undefined.html> | `clippy.undocumented_unsafe`, `unsafe.safety_comments` |
| The Rustonomicon: What Unsafe Rust Can Do | <https://doc.rust-lang.org/nomicon/what-unsafe-does.html> | `clippy.undocumented_unsafe`, `unsafe.safety_comments` |
| The Cargo Book: Profiles (overflow-checks, panic) | <https://doc.rust-lang.org/cargo/reference/profiles.html> | `build.wasm`, `profile.overflow_checks`, `profile.panic_abort` |
| The Cargo Book: Publishing on crates.io | <https://doc.rust-lang.org/cargo/reference/publishing.html> | `info.publish_ready` |
| RustSec Advisory Database (and cargo-audit) | <https://rustsec.org/> | `supply.audit` |
| cargo-deny: lint your dependency graph | <https://embarkstudios.github.io/cargo-deny/> | `supply.deny` |
| MITRE CWE-20: Improper Input Validation | <https://cwe.mitre.org/data/definitions/20.html> | `fuzz.harness` |
| MITRE CWE-79: Improper Neutralization of Input During Web Page Generation (XSS) | <https://cwe.mitre.org/data/definitions/79.html> | `browser.check`, `csp.policy` |
| MITRE CWE-190: Integer Overflow or Wraparound | <https://cwe.mitre.org/data/definitions/190.html> | `clippy.pedantic`, `profile.overflow_checks`, `fuzz.harness` |
| MITRE CWE-191: Integer Underflow (Wrap or Wraparound) | <https://cwe.mitre.org/data/definitions/191.html> | `clippy.pedantic`, `profile.overflow_checks`, `fuzz.harness` |
| MITRE CWE-758: Reliance on Undefined, Unspecified, or Implementation-Defined Behavior | <https://cwe.mitre.org/data/definitions/758.html> | `clippy.default`, `clippy.undocumented_unsafe`, `unsafe.safety_comments`, `profile.panic_abort` |
| MITRE CWE-1104: Use of Unmaintained Third Party Components | <https://cwe.mitre.org/data/definitions/1104.html> | `supply.audit`, `supply.deny` |
| W3C Content Security Policy Level 3 (Working Draft) | <https://www.w3.org/TR/CSP3/> | `browser.check`, `csp.policy` |
| MDN: CSP script-src, including 'wasm-unsafe-eval' | <https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/script-src> | `csp.policy` |
| OWASP Cheat Sheet: Content Security Policy | <https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html> | `csp.policy` |
| WebAssembly Specification (Core) | <https://webassembly.github.io/spec/core/> | `wasm.imports_empty` |
| MDN: WebAssembly.Module.imports() | <https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Module/imports_static> | `wasm.imports_empty` |
| OWASP Community: Fuzzing | <https://owasp.org/www-community/Fuzzing> | `fuzz.harness` |
| OWASP Application Security Verification Standard (ASVS 5.0) | <https://asvs.owasp.org/> | `fuzz.harness`, `browser.check` |
| OpenSSF Scorecard (incl. License, Security-Policy, Fuzzing, SAST checks) | <https://scorecard.dev/> | `supply.audit`, `hygiene.files` |
| OpenSSF Scorecard checks documentation | <https://github.com/ossf/scorecard> | `hygiene.files` |
| GitHub Docs: Configuring private vulnerability reporting for a repository | <https://docs.github.com/en/code-security/security-advisories/working-with-repository-security-advisories/configuring-private-vulnerability-reporting-for-a-repository> | `hygiene.files` |
| GitHub Docs: Secure use reference (pin actions to commit SHAs) | <https://docs.github.com/en/actions/reference/security/secure-use> | release / governance process |
| Rust Blog: crates.io development update (Trusted Publishing), 2025-07-11 | <https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07> | release / governance process |
| RFC 3691: Trusted Publishing for crates.io | <https://rust-lang.github.io/rfcs/3691-trusted-publishing-cratesio.html> | release / governance process |
| rust-lang/crates-io-auth-action | <https://github.com/rust-lang/crates-io-auth-action> | release / governance process |
| RFC 3463: crates.io policy update (API User-Agent and rate limit) | <https://rust-lang.github.io/rfcs/3463-crates-io-policy-update.html> | release / governance process |

## Notes

- CWE-20 is marked by MITRE as discouraged for mapping specific vulnerabilities; it is cited here as the class the fuzz harness exercises (hostile NaN/Infinity/huge inputs at every export).
- `'wasm-unsafe-eval'` is required for the page to compile its embedded WebAssembly; it permits WebAssembly compilation only, unlike `'unsafe-eval'`, which the gate forbids (MDN script-src).
- The CSP3 document is a W3C Working Draft (version of 27 October 2026 at the access date).
- crates.io requires API clients to send a User-Agent that identifies the application and to stay at or below 1 request per second (RFC 3463); `tools/tracker.py` complies.
- Trusted Publishing: the first release of a crate must be published manually; later releases can use `rust-lang/crates-io-auth-action@v1` from GitHub Actions (Rust Blog, 2025-07-11).
- GitHub recommends pinning actions to full commit SHAs (Secure use reference); the workflows pin to major versions and say so in comments.
- The OWASP fuzzing URL redirects to community.owasp.org/Fuzzing; the Clippy lint list formerly at rust-lang.github.io/rust-clippy now lives at clippy.rust-lang.org.
