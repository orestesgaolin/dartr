#!/usr/bin/env zsh
# Fetch the Dart SDK sources that dartr ports and tests against.
# The tag must match the `dart` on PATH, because the differential tests
# compare dartr with the pinned package:analyzer.
set -e
root="$(cd "$(dirname "$0")/.." && pwd)"
tag="${DART_SDK_TAG:-3.13.3}"
dst="$root/third_party/dart-sdk"
# In a worktree, link the checkout of the primary worktree instead of cloning.
primary="$(git -C "$root" worktree list --porcelain | awk '/^worktree /{print $2; exit}')"
if [[ "$primary" != "$root" && -d "$primary/third_party/dart-sdk" ]]; then
  mkdir -p "$root/third_party" && ln -sfn "$primary/third_party/dart-sdk" "$dst"
  exit 0
fi
[[ -d "$dst" ]] || git clone --depth 1 --filter=blob:none --sparse https://github.com/dart-lang/sdk.git "$dst"
git -C "$dst" sparse-checkout set pkg/analyzer pkg/_fe_analyzer_shared pkg/analysis_server \
  pkg/analyzer_cli pkg/linter pkg/analysis_server_plugin pkg/analyzer_plugin pkg/dartdev sdk/lib tests/language
git -C "$dst" fetch --depth 1 --filter=blob:none origin tag "$tag"
git -C "$dst" checkout -q "$tag"
