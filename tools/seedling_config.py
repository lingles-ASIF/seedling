"""Shared helpers for Seedling's governance tools (stdlib only).

Every script reads governance/config.toml through `load()` and discovers the
Cargo layout through `cargo_metadata()`, so nothing hard-codes crate paths or
.wasm filenames.
"""
from __future__ import annotations

import fnmatch
import glob
import json
import os
import pathlib
import subprocess
import tomllib
from functools import lru_cache

ROOT = pathlib.Path(__file__).resolve().parent.parent
CONFIG_PATH = ROOT / "governance" / "config.toml"


@lru_cache(maxsize=1)
def load() -> dict:
    with CONFIG_PATH.open("rb") as f:
        return tomllib.load(f)


def cfg(section: str, key: str, default=None):
    return load().get(section, {}).get(key, default)


def run(cmd, cwd=ROOT, env=None, timeout=None, check=False):
    """Run a command, capture text output. Never raises on non-zero exit."""
    merged = dict(os.environ)
    if env:
        merged.update(env)
    try:
        p = subprocess.run(cmd, cwd=cwd, env=merged, capture_output=True,
                           text=True, timeout=timeout)
    except FileNotFoundError as e:
        return subprocess.CompletedProcess(cmd, 127, "", str(e))
    except subprocess.TimeoutExpired as e:
        return subprocess.CompletedProcess(cmd, 124, e.stdout or "", f"timeout after {timeout}s")
    if check and p.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)} failed:\n{p.stderr}")
    return p


def git(*args, cwd=ROOT) -> str:
    return run(["git", *args], cwd=cwd).stdout.strip()


# ---------------------------------------------------------------- cargo layout

@lru_cache(maxsize=1)
def cargo_metadata() -> dict:
    p = run(["cargo", "metadata", "--format-version", "1", "--no-deps"])
    if p.returncode != 0:
        raise RuntimeError("cargo metadata failed: " + p.stderr.strip())
    return json.loads(p.stdout)


def workspace_root() -> pathlib.Path:
    return pathlib.Path(cargo_metadata()["workspace_root"])


def packages() -> list[dict]:
    md = cargo_metadata()
    members = set(md.get("workspace_members", []))
    return [p for p in md["packages"] if not members or p["id"] in members]


def publish_package() -> dict:
    """The package release.yml publishes (config override, else auto)."""
    want = cfg("publish", "package", "")
    pk = packages()
    if want:
        for p in pk:
            if p["name"] == want:
                return p
        raise RuntimeError(f"[publish].package = {want!r} is not a workspace member")
    cdylibs = [p for p in pk if any("cdylib" in t.get("crate_types", []) for t in p["targets"])]
    if len(cdylibs) == 1:
        return cdylibs[0]
    root_manifest = str(workspace_root() / "Cargo.toml")
    for p in pk:
        if p["manifest_path"] == root_manifest:
            return p
    return pk[0]


def rust_sources() -> list[pathlib.Path]:
    """All .rs files under each workspace member's source directories."""
    files: set[pathlib.Path] = set()
    for p in packages():
        pkg_dir = pathlib.Path(p["manifest_path"]).parent
        for t in p["targets"]:
            src = pathlib.Path(t["src_path"])
            base = src.parent
            # Never walk outside the package (e.g. a target in the package root).
            if pkg_dir in base.parents or base == pkg_dir:
                scan = base if base != pkg_dir else None
                if scan:
                    files.update(scan.rglob("*.rs"))
            files.add(src)
    return sorted(f for f in files if "target" not in f.relative_to(ROOT).parts)


def wasm_files(root: pathlib.Path = ROOT) -> list[pathlib.Path]:
    return sorted(pathlib.Path(p) for p in glob.glob(str(root / cfg("build", "wasm_glob"))))


def build_std() -> bool:
    return os.environ.get("SEEDLING_BUILD_STD") == "1"


def cargo_target_args() -> tuple[list[str], dict]:
    """Extra cargo args + env for wasm builds (build-std only on request)."""
    args = ["--target", cfg("build", "wasm_target")]
    env = {}
    if build_std():
        args += list(cfg("build", "build_std_flags"))
        env["RUSTC_BOOTSTRAP"] = "1"
    return args, env


# ---------------------------------------------------------------- misc

def glob_match(path: str, pattern: str) -> bool:
    """fnmatch with `**/` meaning 'any depth, including none'."""
    if fnmatch.fnmatch(path, pattern):
        return True
    if pattern.startswith("**/") and fnmatch.fnmatch(path, pattern[3:]):
        return True
    return False


def repo_slug() -> str:
    return os.environ.get("GITHUB_REPOSITORY") or cfg("project", "repo_slug")
