#!/usr/bin/env zsh
# Baseline: time `dart analyze` (cold, no persistent cache) on corpus packages.
set -e
cache="${TMPDIR:-/tmp}/dartr-bench-cache"  # own cache: no race with other Dart processes
corpus=(
  "$HOME/fvm/default/packages/flutter"
  "$HOME/fvm/default/packages/flutter_tools"
  "$(cd "$(dirname "$0")" && pwd)/corpus/analyzer-9.0.0"
  "$(cd "$(dirname "$0")" && pwd)/corpus/visible-app"
)
(( $# )) && corpus=(${(M)corpus:#*/(${(j:|:)~@})})
for p in "${corpus[@]}"; do
  echo "== $p ($(find $p -path "*/.dart_tool" -prune -o -path "*/node_modules" -prune -o -name "*.dart" -print | xargs cat | wc -l) Dart lines, excl. .dart_tool)"
  (cd $p && dart pub get --offline >/dev/null 2>&1 || true)
  hyperfine -i -w 1 -r 3 --export-json "$(dirname $0)/results/baseline-$(basename $p).json" \
    --prepare "rm -rf $cache" \
    "dart analyze --cache=$cache --no-fatal-warnings $p" 2>&1 | grep -E 'Time|Range'
done
