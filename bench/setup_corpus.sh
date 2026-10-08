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

# visible-app: pinned commit of the Visible Flutter app (pub workspace, Flutter 3.47.4 =
# Dart 3.13.3). Nested packages outside the root workspace need their own pub get,
# else `dart analyze` reports URI_DOES_NOT_EXIST for them.
visible_src="${VISIBLE_APP_SRC:-$HOME/Projects/visible/visible-app}"
visible_rev="005d4cfad"
flutter="${FLUTTER:-$HOME/fvm/versions/3.47.4/bin/flutter}"
if [[ ! -d "$dir/visible-app" ]]; then
  git clone -q --no-checkout "$visible_src" "$dir/visible-app"
  git -C "$dir/visible-app" checkout -q "$visible_rev"
fi
cd "$dir/visible-app"
"$flutter" pub get >/dev/null
for p in $(find . -name pubspec.yaml -not -path '*/.dart_tool/*' -not -path '*/node_modules/*'); do
  grep -q '^resolution: workspace' "$p" && continue
  (cd "$(dirname "$p")" && "$flutter" pub get >/dev/null)
done
