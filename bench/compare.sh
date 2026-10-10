#!/usr/bin/env zsh
# Compare `dart analyze` (its normal disk cache in ~/.dartServer, warmed by the first run) with
# `dartr analyze` (no disk cache) on the corpus packages: hyperfine, 1 warm-up run, 3 measured runs.
#
#   bench/compare.sh [corpus-name ...]       e.g. bench/compare.sh flutter_tools
#   DARTR=path/to/dartr bench/compare.sh     (default: target/release/dartr)
#   COLD=1 bench/compare.sh                  also time `dart analyze` with an empty cache
#
# Results: bench/results/compare-<corpus>.json (git-ignored). The load average before and after
# each corpus is printed, because a busy machine changes wall times.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
dartr="${DARTR:-$here/../target/release/dartr}"
dartr="$(cd "$(dirname "$dartr")" && pwd)/$(basename "$dartr")"
corpus=(
  "$HOME/fvm/default/packages/flutter"
  "$HOME/fvm/default/packages/flutter_tools"
  "$here/corpus/analyzer-9.0.0"
  "$here/corpus/visible-app"
)
(( $# )) && corpus=(${(M)corpus:#*/(${(j:|:)~@})})
mkdir -p "$here/results"
echo "dartr: $("$dartr" --version | head -1), dart: $(dart --version 2>&1 | head -1)"
for p in "${corpus[@]}"; do
  name="$(basename "$p")"
  echo "== $name (load before: $(sysctl -n vm.loadavg))"
  (cd "$p" && hyperfine -i -N -w 1 -r 3 --export-json "$here/results/compare-$name.json" \
    -n dart "dart analyze" -n dartr "$dartr analyze" 2>&1 | grep -E 'Benchmark|Time|Range')
  if [[ -n "$COLD" ]]; then
    cache="${TMPDIR:-/tmp}/dartr-bench-cold-cache"
    (cd "$p" && hyperfine -i -N -w 1 -r 3 --export-json "$here/results/compare-$name-cold.json" \
      --prepare "rm -rf $cache" -n "dart (empty cache)" "dart analyze --cache=$cache" 2>&1 \
      | grep -E 'Benchmark|Time|Range')
  fi
  echo "   load after: $(sysctl -n vm.loadavg)"
done
