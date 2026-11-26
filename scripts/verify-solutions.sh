#!/usr/bin/env bash
# Copies each starter crate to a scratch dir, overlays the reference solution's src/,
# then runs the given tests. Usage: scripts/verify-solutions.sh [exNN-slug ...]
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
work="${KIROCREW_SCRATCH:-${TMPDIR:-/tmp}}/verify-all"
mkdir -p "$work"
names=("$@")
if [ ${#names[@]} -eq 0 ]; then
  names=($(cd "$root/crates" && ls -d ex* | sort))
fi
fail=0
for n in "${names[@]}"; do
  rm -rf "$work/$n"
  cp -r "$root/crates/$n" "$work/$n"
  rm -rf "$work/$n/target"
  cp -r "$root/solutions/$n/src/." "$work/$n/src/"
  export CARGO_TARGET_DIR="$work/target-$n"
  if (cd "$work/$n" && cargo test --quiet 2>&1 | tail -n 40 > "$work/$n.log"; exit "${PIPESTATUS[0]}"); then
    echo "PASS  $n"
  else
    echo "FAIL  $n  (see $work/$n.log)"
    fail=1
  fi
done
exit $fail
