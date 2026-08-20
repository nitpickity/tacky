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
#   scripts/bench_report.sh                       # per-group detail, every group
#   scripts/bench_report.sh encode_otlp_traces    # groups matching any argument
#   scripts/bench_report.sh --table               # markdown grid, one row per group
#   scripts/bench_report.sh --table --base prost  # ratios against another arm
#
# `--table` reports each arm's time and its ratio to the base arm, where 2.2x means the
# arm took 2.2x as long — i.e. the base was 2.2x faster. Cells are blank where an arm
# does not exist for that group: proto2 schemas have no `cpp-noutf8`, `google_message1`
# has no `tacky-rev-owned`, and decode groups have no C++ arms at all.
#
# criterion keeps only the latest result per benchmark id, so a narrow filter refreshes
# some arms and leaves others behind. Comparing across runs like that is meaningless on a
# machine that drifts, so cells measured well before the newest one in their row are marked
# `!` — re-run the whole group before trusting those ratios.
#
# Only reads what is already on disk — run `cargo bench` first.
set -euo pipefail

cd "$(dirname "$0")/.."
[ -d target/criterion ] || { echo "no target/criterion; run cargo bench first" >&2; exit 1; }

python3 - "$@" <<'PY'
import json, os, sys

args = sys.argv[1:]
table = "--table" in args
args = [a for a in args if a != "--table"]
base = "tacky"
if "--base" in args:
    i = args.index("--base")
    base = args[i + 1]
    del args[i:i + 2]
filters = args

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


# `value_str` is criterion's parameter (the /10, /100, /1000 suffix); sort numerically
# where it is one so a sweep reads in order.
def key(r):
    try:
        return (float(r[1]), r[0])
    except ValueError:
        return (0.0, r[1] + r[0])


def label(fn, val):
    return f"{fn}/{val}" if val else fn


if not table:
    for group in sorted(rows):
        print(f"\n{group}")
        for fn, val, ns, lo, hi, nbytes, _ in sorted(rows[group], key=key):
            thr = f"  {nbytes * 1e9 / ns / 2**30:8.3f} GiB/s" if nbytes else ""
            print(f"  {label(fn, val):<26} {human(ns):>10}  [{human(lo)} {human(hi)}]{thr}")
    sys.exit()

# Stable, meaningful column order rather than whatever the walk found; unknown arms keep
# their first-seen order at the end so a new one still shows up.
PREFERRED = ["tacky", "tacky-slice", "tacky-rev", "tacky-rev-owned", "tacky-walk",
             "prost", "cpp", "cpp-noutf8", "cpp-cached", "cpp-noutf8-cached"]
seen = []
for arms in rows.values():
    for fn, val, *_ in arms:
        if label(fn, val) not in seen:
            seen.append(label(fn, val))
cols = [c for c in PREFERRED if c in seen] + [c for c in seen if c not in PREFERRED]
if base not in cols:
    print(f"base arm {base!r} not present; have: {', '.join(cols)}", file=sys.stderr)
    sys.exit(1)

stale_seen = [False]
width = max([len(g) for g in rows] + [9])
head = f"| {'benchmark':<{width}} | " + " | ".join(f"{c:^18}" for c in cols) + " |"
print(head)
print(f"|{'-' * (width + 2)}|" + "|".join("-" * 20 for _ in cols) + "|")

for group in sorted(rows):
    by_arm = {label(fn, val): (ns, mt) for fn, val, ns, _, _, _, mt in rows[group]}
    b = by_arm.get(base, (None, None))[0]
    newest = max(mt for _, mt in by_arm.values())
    cells = []
    for c in cols:
        if c not in by_arm:
            cells.append(f"{'':^18}")
            continue
        ns, mt = by_arm[c]
        txt = human(ns) if c == base or b is None else f"{human(ns)} ({ns / b:.2f}x)"
        if newest - mt > 600:
            txt += "!"
            stale_seen[0] = True
        cells.append(f"{txt:^18}")
    print(f"| {group:<{width}} | " + " | ".join(cells) + " |")

print(f"\nratio is arm / {base}: 2.20x means {base} was 2.2x faster.")
if stale_seen[0]:
    print("! measured >10 min before the newest arm in that row — re-run the whole group.")
missing = [g for g in sorted(rows) if base not in {label(f, v) for f, v, *_ in rows[g]}]
if missing:
    print(f"no {base} arm, so no ratios: {', '.join(missing)}")
PY
