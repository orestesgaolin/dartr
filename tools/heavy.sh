#!/bin/sh
# Run one heavy command (cargo build/test/clippy, difftest, corpus comparisons) in one of two
# shared build slots, so that parallel agents never run more than two heavy commands at once.
#
#   tools/heavy.sh <command> [args...]
#
# Slot 1 is the lock file that single-slot commands use, so both styles share it.
# The command runs with CARGO_BUILD_JOBS=4, RUST_TEST_THREADS=4 and nice 10 by default (see quiet mode below).
# DARTR_HEAVY_SLOTS=1 limits it to one slot (used when the machine gets too busy).

dir=${DARTR_HEAVY_LOCK_DIR:-/private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad}
slots=${DARTR_HEAVY_SLOTS:-$(cat "$dir/heavy.slots" 2>/dev/null || echo 2)}
# Quiet mode: heavy.jobs, heavy.test_threads and heavy.nice in the lock directory override the
# defaults for every command (the coordinator lowers them when the machine must stay quiet).
export CARGO_BUILD_JOBS=$(cat "$dir/heavy.jobs" 2>/dev/null || echo "${CARGO_BUILD_JOBS:-4}")
# Parity tests start `dart analyze` / `dart language-server` per test; limit parallel tests so a
# workspace test run does not start dozens of Dart processes at once.
export RUST_TEST_THREADS=$(cat "$dir/heavy.test_threads" 2>/dev/null || echo "${RUST_TEST_THREADS:-4}")
niceness=$(cat "$dir/heavy.nice" 2>/dev/null || echo 10)

lock_for() {
  if [ "$1" = 1 ]; then echo "$dir/heavy.lock"; else echo "$dir/heavy.lock.$1"; fi
}

# lockf -k keeps the lock file: without it, lockf deletes the file when the command ends, and a
# process that waits on the deleted file and a process that creates a new one can both hold the slot.
# Take a free slot without waiting, highest slot first: the inline lock command that agents use
# when they cannot run this script takes only slot 1, so keep slot 1 free for them when possible.
# lockf exits 75 when the lock is busy.
i=$slots
while [ "$i" -ge 1 ]; do
  /usr/bin/lockf -k -s -t 0 "$(lock_for "$i")" nice -n "$niceness" "$@"
  rc=$?
  [ "$rc" -ne 75 ] && exit "$rc"
  i=$((i - 1))
done

# All slots are busy: poll every slot until one is free, so a command never waits behind a
# long build while another slot is idle. Read the slot count again on every poll, so a change of
# heavy.slots also reaches commands that are already waiting.
echo "heavy.sh: all $slots build slots busy, waiting" >&2
# Gemini agents (worktrees gm-*) run commands as background tasks, and their runtime ends the whole
# session when a background task takes longer than 30 minutes. Give up waiting after 10 minutes
# there, so the agent can try again instead of losing its session.
max_wait=${DARTR_HEAVY_MAX_WAIT:-0}
case "$PWD" in */worktrees/dartr/gm-*) [ "$max_wait" = 0 ] && max_wait=600 ;; esac
waited=0
while :; do
  sleep 5
  waited=$((waited + 5))
  if [ "$max_wait" -gt 0 ] && [ "$waited" -ge "$max_wait" ]; then
    echo "heavy.sh: build slots still busy after $((max_wait / 60)) minutes; run this command again later (in the foreground)" >&2
    exit 75
  fi
  slots=${DARTR_HEAVY_SLOTS:-$(cat "$dir/heavy.slots" 2>/dev/null || echo 2)}
  i=$slots
  while [ "$i" -ge 1 ]; do
    /usr/bin/lockf -k -s -t 0 "$(lock_for "$i")" nice -n "$niceness" "$@"
    rc=$?
    [ "$rc" -ne 75 ] && exit "$rc"
    i=$((i - 1))
  done
done
