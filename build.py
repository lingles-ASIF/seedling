#!/usr/bin/env python3
"""
Build Seedling into ONE self-contained HTML file.

    python3 build.py            # compile Rust -> WebAssembly, then embed it
    python3 build.py --no-cargo # just re-embed an already-built .wasm

Why embed?  Browsers refuse to fetch a separate .wasm file from a page opened
with file:// (double-clicked).  Encoding the bytes as base64 text inside the
HTML sidesteps that, so the result opens offline in any modern browser.

Prerequisites (one time):
    rustup target add wasm32-unknown-unknown
"""
import base64
import hashlib
import re
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent
WASM = ROOT / "target/wasm32-unknown-unknown/release/seedling.wasm"
TEMPLATE = ROOT / "web/template.html"
OUT = ROOT / "seedling.html"
PLACEHOLDER = "__WASM_BASE64__"
CSP_PLACEHOLDER = "__CSP__"


def sha256_source(block: str) -> str:
    return "'sha256-" + base64.b64encode(hashlib.sha256(block.encode("utf-8")).digest()).decode() + "'"


def content_security_policy(html: str) -> str:
    """Allow exactly this page's own inline script and style (by hash), the
    WebAssembly compile it needs, and nothing else: no network, no frames,
    no plugins, no other scripts."""
    scripts = re.findall(r"<script>(.*?)</script>", html, re.S)
    styles = re.findall(r"<style>(.*?)</style>", html, re.S)
    if len(scripts) != 1 or len(styles) != 1:
        sys.exit("Expected exactly one inline <script> and one <style> to hash.")
    return "; ".join([
        "default-src 'none'",
        f"script-src {sha256_source(scripts[0])} 'wasm-unsafe-eval'",
        f"style-src {sha256_source(styles[0])}",
        "connect-src 'none'",
        "img-src 'none'",
        "base-uri 'none'",
        "form-action 'none'",
    ])


def compile_rust() -> None:
    cmd = ["cargo", "build", "--release", "--target", "wasm32-unknown-unknown"]
    print("$", " ".join(cmd))
    try:
        subprocess.run(cmd, cwd=ROOT, check=True)
    except FileNotFoundError:
        sys.exit("cargo not found. Install Rust from https://rustup.rs, then rerun.")
    except subprocess.CalledProcessError:
        sys.exit(
            "Rust build failed. If the error mentions wasm32-unknown-unknown, run:\n"
            "    rustup target add wasm32-unknown-unknown"
        )


def embed() -> None:
    if not WASM.exists():
        sys.exit(f"Missing {WASM.relative_to(ROOT)} - run without --no-cargo first.")
    wasm_bytes = WASM.read_bytes()
    if wasm_bytes[:4] != b"\0asm":
        sys.exit("That file isn't WebAssembly (bad magic number).")
    template = TEMPLATE.read_text(encoding="utf-8")
    if template.count(PLACEHOLDER) != 1:
        sys.exit(f"Template must contain {PLACEHOLDER} exactly once.")
    b64 = base64.b64encode(wasm_bytes).decode("ascii")
    html = template.replace(PLACEHOLDER, b64)
    html = html.replace(CSP_PLACEHOLDER, content_security_policy(html))
    OUT.write_text(html, encoding="utf-8")
    print(f"wrote {OUT.name}: {OUT.stat().st_size:,} bytes "
          f"(engine {len(wasm_bytes):,} bytes of WebAssembly)")


if __name__ == "__main__":
    if "--no-cargo" not in sys.argv:
        compile_rust()
    embed()
