#!/usr/bin/env bash
# Disassembles named functions out of a release binary and summarises each:
# instruction count, call count, and which callees.
#
# `objdump --disassemble-symbols` does not restrict output on the Apple toolchain
# (it dumps the whole binary), so slice by address instead: take the symbol's own
# address and the next symbol's address from a sorted `nm`, and hand both to
# `--start-address`/`--stop-address`.
#
# Instruction and call counts answer codegen questions without a profiler run, so
# they sidestep the 1-2% wall-clock noise floor on this machine.
#
# Usage:
#   scripts/fn_asm.sh <binary> <symbol-substring> [symbol-substring ...]
#   scripts/fn_asm.sh target/release/deps/descriptor_set-* write_field write_message
#
# Add -v to also print the disassembly.
set -euo pipefail

verbose=0
if [ "${1:-}" = "-v" ]; then verbose=1; shift; fi
bin=${1:?usage: fn_asm.sh [-v] <binary> <symbol-substring> ...}
shift

# One sorted pass: "<hex addr> <demangled name>", used both to find a symbol and to
# find where it ends. Demangled, so the substrings to match are readable Rust paths
# — but that means a name can contain spaces, hence `$1" "$3` is not enough.
syms=$(nm -C "$bin" | awk 'NF>=3 && $1 ~ /^[0-9a-f]+$/ {
    name = $3; for (i = 4; i <= NF; i++) name = name " " $i
    print $1" "name
}' | sort)

for pat in "$@"; do
    while read -r addr name; do
        # Reads all of `syms` rather than `exit`ing on the first match: an early exit
        # SIGPIPEs the upstream `echo`, which `pipefail` then turns into a fatal 141.
        # `$1"" > a""` forces a string compare. An all-digit address like
        # 0000000100033180 is "numeric" to awk (read as decimal), so an unforced compare
        # mixes numeric and string semantics against the addresses containing a-f and
        # silently finds no end — the function then gets skipped with no output.
        end=$(echo "$syms" | awk -v a="$addr" '$1"" > a"" && !seen { print $1; seen = 1 }')
        [ -n "$end" ] || continue
        # Tolerate a bad range rather than dying on it: `nm` lists symbols that are
        # not code, and an aliased or zero-length one yields stop <= start.
        asm=$(objdump -d --start-address="0x$addr" --stop-address="0x$end" "$bin" 2>/dev/null) || continue
        echo "=== $name"
        # `bl` is tab-separated from its operand in objdump's output, and BSD awk has
        # no `\b` word boundary (it reads as a backspace), so match the tabs.
        echo "$asm" | awk '
            /^[[:space:]]*[0-9a-f]+:/ { n++ }
            /\tbl\t/ { c++; if (match($0, /<[^>]+>/)) callee[substr($0, RSTART+1, RLENGTH-2)]++ }
            END {
                printf "  %d instructions, %d calls\n", n, c
                for (k in callee) printf "    %dx %s\n", callee[k], k
            }'
        # Not `[ … ] && echo`: under `set -e` a false test as the loop body's last
        # command kills the shell.
        if [ "$verbose" -eq 1 ]; then echo "$asm"; fi
    done < <(echo "$syms" | grep -- "$pat" | sed 's/^\([0-9a-f]*\) _*/\1 /')
done
