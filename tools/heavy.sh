#!/bin/sh
# Run one heavy command (cargo build/test/clippy, difftest, corpus comparisons) in one of two
# shared build slots, so that parallel agents never run more than two heavy commands at once.
#
#   tools/heavy.sh <command> [args...]
#
# Slot 1 is the lock file that single-slot commands use, so both styles share it.
# The command runs with CARGO_BUILD_JOBS=4, RUST_TEST_THREADS=4 (unless set) and nice 10.
# DARTR_HEAVY_SLOTS=1 limits it to one slot (used when the machine gets too busy).

dir=${DARTR_HEAVY_LOCK_DIR:-/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad}
slots=${DARTR_HEAVY_SLOTS:-$(cat "$dir/heavy.slots" 2>/dev/null || echo 2)}
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
# Parity tests start `dart analyze` / `dart language-server` per test; limit parallel tests so a
# workspace test run does not start dozens of Dart processes at once.
export RUST_TEST_THREADS=${RUST_TEST_THREADS:-4}

lock_for() {
  if [ "$1" = 1 ]; then echo "$dir/heavy.lock"; else echo "$dir/heavy.lock.$1"; fi
}

# Take a free slot without waiting. lockf exits 75 when the lock is busy.
i=1
while [ "$i" -le "$slots" ]; do
  /usr/bin/lockf -s -t 0 "$(lock_for "$i")" nice -n 10 "$@"
  rc=$?
  [ "$rc" -ne 75 ] && exit "$rc"
  i=$((i + 1))
done

# All slots are busy: poll every slot until one is free, so a command never waits behind a
# long build while another slot is idle.
echo "heavy.sh: all $slots build slots busy, waiting" >&2
while :; do
  sleep 5
  i=1
  while [ "$i" -le "$slots" ]; do
    /usr/bin/lockf -s -t 0 "$(lock_for "$i")" nice -n 10 "$@"
    rc=$?
    [ "$rc" -ne 75 ] && exit "$rc"
    i=$((i + 1))
  done
done
