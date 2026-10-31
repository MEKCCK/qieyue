#!/usr/bin/env bash
# Sync the vendored backend with the upstream project.
#
# `rust/` is a pristine copy of https://github.com/ch6vip/fqapp's `rust/`
# directory: the GUI and the headless daemon consume it as a path dependency,
# and nothing in it may be edited here. That invariant is what makes upstream
# updates possible at any time: this script replaces the whole directory and the
# result must diff cleanly.
#
# Usage:
#   ./scripts/sync-backend.sh                # sync to the upstream default branch
#   ./scripts/sync-backend.sh master         # an explicit branch
#   ./scripts/sync-backend.sh v1.2.3         # a tag
#   ./scripts/sync-backend.sh --check        # verify rust/ matches the default branch
#
# Upstream refactors that would break the desktop build; run
# `./scripts/cargo.sh check --workspace` after a sync to surface them.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
upstream_url="${FQAPP_UPSTREAM_URL:-https://github.com/ch6vip/fqapp.git}"
upstream_ref=""
check_only=0

usage() {
  sed -n '/^# Usage:/,/^# Upstream refactors/p' "$0" | sed 's/^# \{0,1\}//'
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --check) check_only=1 ;;
    -h|--help) usage; exit 0 ;;
    -*)
      echo "错误：未知参数：$1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [ -n "$upstream_ref" ]; then
        echo "错误：只能指定一个上游 ref。" >&2
        exit 2
      fi
      upstream_ref="$1"
      ;;
  esac
  shift
done

if ! command -v git >/dev/null 2>&1; then
  echo "错误：未找到 git。" >&2
  exit 1
fi
if [ ! -d "$repo_root/rust" ]; then
  echo "错误：找不到 $repo_root/rust。" >&2
  exit 1
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/qieyue-upstream.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

if [ -z "$upstream_ref" ]; then
  upstream_ref="$(git ls-remote --symref "$upstream_url" HEAD \
    | awk '/^ref:/ { sub("refs/heads/", "", $2); print $2; exit }')"
  if [ -z "$upstream_ref" ]; then
    echo "错误：无法确定上游默认分支；请显式传入 ref。" >&2
    exit 1
  fi
fi

# Sparse + blobless: only the `rust/` tree is materialised, so syncing does not
# download the Flutter/Android half of the upstream repository.
echo "==> 拉取上游 $upstream_url ($upstream_ref)…"
if ! git clone --quiet --depth 1 --filter=blob:none --sparse \
  --branch "$upstream_ref" "$upstream_url" "$tmp/upstream"; then
  echo "错误：拉取上游失败。--branch 只接受分支或标签，不接受完整 commit SHA。" >&2
  exit 1
fi
git -C "$tmp/upstream" sparse-checkout set rust >/dev/null
upstream_rust="$tmp/upstream/rust"
upstream_rev="$(git -C "$tmp/upstream" rev-parse --short HEAD)"

if [ ! -d "$upstream_rust" ]; then
  echo "错误：上游 $upstream_ref 中没有 rust/ 目录。" >&2
  exit 1
fi

if [ "$check_only" -eq 1 ]; then
  if diff -rq "$upstream_rust" "$repo_root/rust" >/dev/null; then
    echo "rust/ 与上游 $upstream_rev 一致。"
    exit 0
  fi
  echo "rust/ 与上游 $upstream_rev 不一致：" >&2
  diff -rq "$upstream_rust" "$repo_root/rust" >&2 || true
  exit 1
fi

# rsync keeps obsolete files from surviving a sync (deletions included). The
# fallback is for machines without rsync, typically minimal containers.
if command -v rsync >/dev/null 2>&1; then
  rsync -a --delete "$upstream_rust/" "$repo_root/rust/"
else
  rm -rf "$repo_root/rust"
  cp -R "$upstream_rust" "$repo_root/rust"
fi

if ! diff -rq "$upstream_rust" "$repo_root/rust" >/dev/null; then
  echo "错误：同步后 rust/ 仍与上游不一致。" >&2
  exit 1
fi

echo "==> rust/ 已同步到上游 $upstream_rev。"
echo "下一步：./scripts/cargo.sh check --workspace（上游依赖变化会在这里暴露）"
