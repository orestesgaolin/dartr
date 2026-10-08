#!/usr/bin/env zsh
# Prepare benchmark corpus packages that need local changes to resolve.
# analyzer-9.0.0 from pub is a pub-workspace member with SDK-only dev
# dependencies: remove `resolution: workspace` and dev_dependencies.
set -e
dir="$(cd "$(dirname "$0")" && pwd)/corpus"
mkdir -p "$dir"
if [[ ! -d "$dir/analyzer-9.0.0" ]]; then
  dart pub cache add analyzer --version 9.0.0 >/dev/null
  cp -R "$HOME/.pub-cache/hosted/pub.dev/analyzer-9.0.0" "$dir/"
  cd "$dir/analyzer-9.0.0"
  sed -i '' '/^resolution: workspace/d' pubspec.yaml
  awk '/^dev_dependencies:/{skip=1;next} skip && /^[^ #]/{skip=0} !skip' pubspec.yaml > p && mv p pubspec.yaml
fi
(cd "$dir/analyzer-9.0.0" && dart pub get >/dev/null)
