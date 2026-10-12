#!/usr/bin/env zsh
# Measure peak RSS of `dart analyze` (its normal disk cache in ~/.dartServer, warmed by one warm-up
# run) and `dartr analyze` (no disk cache) on the corpus packages: `/usr/bin/time -l`, 3 runs each,
# reporting the median.
#
#   bench/memory.sh [corpus-name ...]        e.g. bench/memory.sh flutter_tools
#   DARTR=path/to/dartr bench/memory.sh      (default: target/release/dartr)
#
# Note: `dart analyze` runs the analysis server in a child process and waits for it before exiting;
# on macOS, `wait4` folds the child's `ru_maxrss` into the parent's `rusage`, so `/usr/bin/time -l`
# ("maximum resident set size") reports the peak RSS of the analysis server process.
#
# Results: bench/results/memory-<corpus>.json (git-ignored).
set -e
here="$(cd "$(dirname "$0")" && pwd)"
common_root="$(cd "$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)/.." && pwd)"
dartr="${DARTR:-$here/../target/release/dartr}"
[[ -x "$dartr" ]] || dartr="$common_root/target/release/dartr"
dartr="$(cd "$(dirname "$dartr")" && pwd)/$(basename "$dartr")"
corpus_dir="$here/corpus"
[[ -d "$corpus_dir" ]] || corpus_dir="$common_root/bench/corpus"
corpus=(
  "$HOME/fvm/default/packages/flutter"
  "$HOME/fvm/default/packages/flutter_tools"
  "$corpus_dir/analyzer-9.0.0"
  "$corpus_dir/visible-app"
)
(( $# )) && corpus=(${(M)corpus:#*/(${(j:|:)~@})})
mkdir -p "$here/results"
echo "dartr: $("$dartr" --version | head -1), dart: $(dart --version 2>&1 | head -1)"

measure_rss() {
  local tmp
  tmp="$(mktemp "${TMPDIR:-/tmp}/dartr-mem.XXXXXX")"
  /usr/bin/time -l "$@" >/dev/null 2>"$tmp" || true
  awk '/maximum resident set size/ {print $1}' "$tmp"
  rm -f "$tmp"
}

median3() {
  printf '%s\n' "$@" | sort -n | sed -n '2p'
}

for p in "${corpus[@]}"; do
  name="$(basename "$p")"
  load_before="$(sysctl -n vm.loadavg)"
  echo "== $name (load before: $load_before)"
  (
    cd "$p"
    dart analyze >/dev/null 2>&1 || true
    d1="$(measure_rss dart analyze)"
    d2="$(measure_rss dart analyze)"
    d3="$(measure_rss dart analyze)"
    dm="$(median3 "$d1" "$d2" "$d3")"

    r1="$(measure_rss "$dartr" analyze)"
    r2="$(measure_rss "$dartr" analyze)"
    r3="$(measure_rss "$dartr" analyze)"
    rm="$(median3 "$r1" "$r2" "$r3")"

    load_after="$(sysctl -n vm.loadavg)"
    python3 -I -c '
import json, sys
name, lb, la = sys.argv[1], sys.argv[2], sys.argv[3]
d_runs = [int(x) for x in sys.argv[4:7]]
d_med = int(sys.argv[7])
r_runs = [int(x) for x in sys.argv[8:11]]
r_med = int(sys.argv[11])
out_path = sys.argv[12]
def entry(cmd, med, runs):
    return {
        "command": cmd,
        "median_bytes": med,
        "median_mb": round(med / (1024 * 1024), 2),
        "median_gb": round(med / (1024 ** 3), 4),
        "runs_bytes": runs,
    }
data = {
    "corpus": name,
    "load_before": lb.strip(),
    "load_after": la.strip(),
    "results": [entry("dart", d_med, d_runs), entry("dartr", r_med, r_runs)],
}
with open(out_path, "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
print(f"  dart : median {d_med / (1024*1024):7.1f} MB ({d_med / (1024**3):.2f} GB)  runs: {[round(x/(1024*1024), 1) for x in d_runs]} MB")
print(f"  dartr: median {r_med / (1024*1024):7.1f} MB ({r_med / (1024**3):.2f} GB)  runs: {[round(x/(1024*1024), 1) for x in r_runs]} MB")
' "$name" "$load_before" "$load_after" "$d1" "$d2" "$d3" "$dm" "$r1" "$r2" "$r3" "$rm" "$here/results/memory-$name.json"
    echo "   load after: $load_after"
  )
done
