#!/usr/bin/env zsh
# Fetch the Dart SDK sources that dartr ports and tests against, and the
# Dart formatter (package:dart_style) at the revision that the SDK pins.
# The tag must match the `dart` on PATH, because the differential tests
# compare dartr with the pinned package:analyzer and `dart format`.
set -e
root="$(cd "$(dirname "$0")/.." && pwd)"
tag="${DART_SDK_TAG:-3.13.3}"
dst="$root/third_party/dart-sdk"
style="$root/third_party/dart_style"
# In a worktree, link the checkouts of the primary worktree instead of cloning.
# Keep Spotlight away from build output: macOS does not index folders named *.noindex.
# `target` stays a valid path (symlink), so scripts that use target/... still work.
if [[ ! -e "$root/target" ]]; then
  mkdir -p "$root/target.noindex" && ln -s target.noindex "$root/target"
fi
primary="$(git -C "$root" worktree list --porcelain | awk '/^worktree /{print $2; exit}')"
if [[ "$primary" != "$root" && -d "$primary/third_party/dart-sdk" ]]; then
  mkdir -p "$root/third_party" && ln -sfn "$primary/third_party/dart-sdk" "$dst"
  if [[ -d "$primary/third_party/dart_style" ]]; then
    ln -sfn "$primary/third_party/dart_style" "$style"
  fi
  exit 0
fi
[[ -d "$dst" ]] || git clone --depth 1 --filter=blob:none --sparse https://github.com/dart-lang/sdk.git "$dst"
git -C "$dst" sparse-checkout set pkg/analyzer pkg/_fe_analyzer_shared pkg/analysis_server \
  pkg/analyzer_cli pkg/analyzer_utilities pkg/analyzer_testing pkg/linter pkg/analysis_server_plugin pkg/analyzer_plugin pkg/dartdev sdk/lib tests/language pkg/front_end/parser_testcases
git -C "$dst" fetch --depth 1 --filter=blob:none origin tag "$tag"
git -C "$dst" checkout -q "$tag"
# dart_style: the revision is `dart_style_rev` in the SDK DEPS file.
style_rev="$(sed -n 's/^ *"dart_style_rev": "\([0-9a-f]*\)".*/\1/p' "$dst/DEPS")"
[[ -d "$style" ]] || git clone --filter=blob:none https://github.com/dart-lang/dart_style.git "$style"
git -C "$style" fetch --filter=blob:none origin "$style_rev"
git -C "$style" checkout -q "$style_rev"
