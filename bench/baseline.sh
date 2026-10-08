#!/usr/bin/env zsh
# Baseline: time `dart analyze` (cold, no persistent cache) on corpus packages.
set -e
corpus=(
  "$HOME/fvm/default/packages/flutter"
  "$HOME/fvm/default/packages/flutter_tools"
  "$(cd "$(dirname "$0")/.." && pwd)/third_party/dart-sdk/pkg/analyzer"
)
for p in "${corpus[@]}"; do
  echo "== $p ($(find $p/lib -name '*.dart' | xargs cat | wc -l) lines in lib)"
  (cd $p && dart pub get --offline >/dev/null 2>&1 || true)
  hyperfine -w 1 -r 3 --export-json "$(dirname $0)/results/baseline-$(basename $p).json" \
    --prepare "rm -rf $HOME/.dartServer/.analysis-driver" \
    "dart analyze --no-fatal-warnings $p" 2>&1 | grep -E 'Time|Range'
done
