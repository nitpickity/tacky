#!/usr/bin/env bash
# Tabulates the last `cargo bench` run from criterion's JSON.
#
# Parsing criterion's stdout is a trap: a benchmark whose name is long enough gets
# its `time:` line wrapped onto the next line, and the `change:` block prints a
# second `time:` line right after the first, so line-oriented extraction silently
# attributes numbers to the wrong arm. `target/criterion/<group>/<arm>/new/` has the
# same figures as structured data.
#
# Usage:
#   scripts/bench_report.sh                    # every group
#   scripts/bench_report.sh encode_otlp_traces # groups matching any argument
#
# Only reads what is already on disk — run `cargo bench` first.
set -euo pipefail

cd "$(dirname "$0")/.."
[ -d target/criterion ] || { echo "no target/criterion; run cargo bench first" >&2; exit 1; }

python3 - "$@" <<'PY'
import json, os, sys

filters = sys.argv[1:]
rows = {}
for dirpath, _, files in os.walk("target/criterion"):
    if not dirpath.endswith("/new") or "benchmark.json" not in files:
        continue
    with open(f"{dirpath}/benchmark.json") as f:
        bench = json.load(f)
    with open(f"{dirpath}/estimates.json") as f:
        est = json.load(f)
    group = bench["group_id"]
    if filters and not any(k in group for k in filters):
        continue
    ns = est["median"]["point_estimate"]
    lo = est["median"]["confidence_interval"]["lower_bound"]
    hi = est["median"]["confidence_interval"]["upper_bound"]
    thr = bench.get("throughput") or {}
    rows.setdefault(group, []).append(
        (bench["function_id"] or "", bench["value_str"] or "", ns, lo, hi,
         thr.get("Bytes"), os.path.getmtime(f"{dirpath}/estimates.json"))
    )


def human(ns):
    for unit, scale in (("ns", 1), ("µs", 1e3), ("ms", 1e6), ("s", 1e9)):
        if ns < 1000 * scale:
            return f"{ns / scale:.4g} {unit}"
    return f"{ns / 1e9:.4g} s"


for group in sorted(rows):
    arms = rows[group]
    # `value_str` is criterion's parameter (the /10, /100, /1000 suffix); sort
    # numerically where it is one so the sweep reads in order.
    def key(r):
        try:
            return (float(r[1]), r[0])
        except ValueError:
            return (0.0, r[1] + r[0])

    print(f"\n{group}")
    for fn, val, ns, lo, hi, nbytes, _ in sorted(arms, key=key):
        name = f"{fn}/{val}" if val else fn
        thr = ""
        if nbytes:
            thr = f"  {nbytes * 1e9 / ns / 2**30:8.3f} GiB/s"
        print(f"  {name:<26} {human(ns):>10}  [{human(lo)} {human(hi)}]{thr}")
PY
