#!/bin/sh
# Install Seedling's governance git hooks for this clone.
#
#   sh tools/install-hooks.sh            # install
#   sh tools/install-hooks.sh --uninstall
#
# Points core.hooksPath at the tracked tools/hooks/ directory, so hook updates
# arrive with `git pull`. Hooks:
#   pre-commit   fast security gate when Rust/web/build files are staged
#                (bypass, logged: SEEDLING_SKIP_GATE=1 git commit ...)
#   commit-msg   Security-Gate trailer + the configured Co-Authored-By/Claude-Session trailers
#   post-commit  regenerates governance/audit/ from git history (never commits)
set -eu
ROOT=$(git rev-parse --show-toplevel)
GITDIR=$(git rev-parse --absolute-git-dir)
cd "$ROOT"

if [ "${1:-}" = "--uninstall" ]; then
  git config --unset core.hooksPath || true
  rm -f "$GITDIR/seedling-hooks.env"
  echo "seedling hooks uninstalled"
  exit 0
fi

chmod +x tools/hooks/* tools/*.py security/gate.py 2>/dev/null || true
git config core.hooksPath tools/hooks

# Build-std detection: if the wasm32 standard library is not installed and
# cannot be (offline sandboxes), the gate must build core itself. Recorded in
# an untracked per-clone file read by the hooks; never committed.
TARGET=wasm32-unknown-unknown
SYSROOT=$(rustc --print sysroot 2>/dev/null || true)
if [ -n "$SYSROOT" ] && [ ! -d "$SYSROOT/lib/rustlib/$TARGET" ]; then
  if [ -d "$SYSROOT/lib/rustlib/src/rust/library" ]; then
    echo "SEEDLING_BUILD_STD=1" > "$GITDIR/seedling-hooks.env"
    echo "note: $TARGET std not installed; hooks will build with -Zbuild-std (SEEDLING_BUILD_STD=1)."
    echo "      On a normal machine run: rustup target add $TARGET"
  else
    echo "warning: $TARGET std not installed. Run: rustup target add $TARGET" >&2
  fi
else
  rm -f "$GITDIR/seedling-hooks.env"
fi

echo "seedling hooks installed (core.hooksPath=tools/hooks): pre-commit, commit-msg, post-commit"
