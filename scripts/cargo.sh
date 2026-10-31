#!/usr/bin/env bash
# Cargo entry point for this workspace.
#
# Builds run through mbx (mr_boxington) when it is installed: mbx caches every
# compilation unit and shares the results across checkouts, which speeds up
# rebuilds without changing what rustc emits. Without mbx — CI, or a fresh
# machine — the same command falls back to plain cargo.
#
# Usage mirrors cargo, minus the leading `cargo`:
#   ./scripts/cargo.sh build --release
#   ./scripts/cargo.sh run --release --bin smoke -- 斗破
#   ./scripts/cargo.sh test
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# `mbx <cargo-subcommand>` replaces `cargo <cargo-subcommand>`; mbx has no
# `mbx cargo ...` form. Keep plain cargo for the version probe, where mbx
# would print its own version instead.
case "${1:-}" in
  --version|-V) exec cargo "$@" ;;
esac

if command -v mbx >/dev/null 2>&1; then
  exec mbx "$@"
fi
exec cargo "$@"
