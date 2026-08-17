#!/usr/bin/env bash
# Summarises a release binary as one "<instructions> <symbol>" line per matched symbol,
# sorted, plus a total. Diffing two of these answers "did that refactor change codegen?"
# without a criterion run — which matters here, where p<0.05 shows up on 1-2% drift.
#
# Usage:
#   scripts/insn_counts.sh <binary> <symbol-substring> [symbol-substring ...]
#
# Before/after, without committing anything:
#   cargo build --release -p testing --bench descriptor_set
#   scripts/insn_counts.sh target/release/deps/descriptor_set-* write_field write_msg > /tmp/before.txt
#   git stash -u && cargo build --release -p testing --bench descriptor_set
#   scripts/insn_counts.sh target/release/deps/descriptor_set-* write_field write_msg > /tmp/after.txt
#   git stash pop
#   diff <(cut -d' ' -f1 /tmp/before.txt) <(cut -d' ' -f1 /tmp/after.txt)   # counts only
#
# Compare the *counts* column, not the names: monomorphisation hashes and enclosing
# module paths move around under refactors even when the emitted code is identical.
set -euo pipefail
bin=${1:?usage: insn_counts.sh <binary> <symbol-substring> ...}
shift
here=$(cd "$(dirname "$0")" && pwd)

"$here/fn_asm.sh" "$bin" "$@" 2>/dev/null | awk '
    /^=== / { name = $2 }
    /instructions/ { gsub(",", "", $1); print $1" "name; total += $1; n++ }
    END { printf "# %d symbols, %d instructions total\n", n, total }
' | sort -n
