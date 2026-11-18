#!/usr/bin/env bash
# Prints the threads and the memory of a running spike process, for docs/spikes/.
#
# Pss is the figure the roadmap's budget is stated in: shared pages are split between the
# processes that map them, so libraries the desktop already loaded are not charged in full.
set -euo pipefail

pid="${1:?usage: measure.sh <pid>}"
if [[ $# != 1 || ! "$pid" =~ ^[1-9][0-9]*$ ]]; then
    echo 'usage: measure.sh <pid>' >&2
    exit 2
fi

if ! threads="$(ps -T -o spid=,comm= -p "$pid")"; then
    printf 'could not list threads for PID %s\n' "$pid" >&2
    exit 1
fi
printf 'threads (%s):\n%s\n' "$(wc -l <<< "$threads")" "$threads"
grep -E '^(Rss|Pss):' "/proc/$pid/smaps_rollup"
