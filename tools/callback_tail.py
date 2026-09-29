#!/usr/bin/env python3
"""What the slowest callbacks have in common.

Reads the per-callback CSV `examples/rt_scenario.rs --callbacks FILE` writes
(add `--stages` for per-stage times) and compares the slowest callbacks with
the typical ones, per preset:

    cargo run --release --example rt_scenario -- --preset "Jazz Chorus" \\
        --pipeline --stages --callbacks /tmp/cb.csv
    python3 tools/callback_tail.py /tmp/cb.csv

If the slow callbacks carry more solver passes, the tail is solver work and
the fix is in the circuit's convergence. If they carry the same work in more
time, per stage, the thread doing that stage was slowed by something else --
scheduling or a shared core -- and no solver change will move it.
"""
import csv
import math
import sys
from collections import defaultdict

COLUMNS = [
    "us",
    "gain_passes",
    "power_passes",
    "power_backtracks",
    "power_fallbacks",
    "power_rescues",
    "pedal_us",
    "gain_us",
    "power_us",
    "cabinet_us",
]


def percentile(sorted_values, p):
    return sorted_values[min(len(sorted_values) - 1, int(round((len(sorted_values) - 1) * p)))]


def mean(rows, column):
    return sum(float(r[column]) for r in rows) / len(rows) if rows else math.nan


def correlation(rows, a, b):
    xs = [float(r[a]) for r in rows]
    ys = [float(r[b]) for r in rows]
    mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    sxx = sum((x - mx) ** 2 for x in xs)
    syy = sum((y - my) ** 2 for y in ys)
    return sxy / math.sqrt(sxx * syy) if sxx > 0 and syy > 0 else math.nan


def per_pass(rows, time, passes):
    total = sum(float(r[passes]) for r in rows)
    return sum(float(r[time]) for r in rows) / total if total else math.nan


def report(preset, rows):
    rows.sort(key=lambda r: float(r["us"]))
    n = len(rows)
    times = [float(r["us"]) for r in rows]
    print(f"\n{preset}: {n} callbacks, p50 {percentile(times, 0.5):.0f}  p99 "
          f"{percentile(times, 0.99):.0f}  p99.9 {percentile(times, 0.999):.0f}  max {times[-1]:.0f} us")
    bands = [
        ("p40-p60", rows[int(n * 0.40):int(n * 0.60)]),
        ("p99-p99.9", rows[int(n * 0.99):int(n * 0.999)]),
        ("top 0.1 %", rows[int(n * 0.999):]),
    ]
    print(f"  {'':10s}" + "".join(f"{c:>17s}" for c in COLUMNS))
    for name, band in bands:
        print(f"  {name:10s}" + "".join(f"{mean(band, c):17.1f}" for c in COLUMNS))
    print("  time per pass, us (caller half: gain; worker half: power):")
    for name, band in bands:
        print(f"    {name:10s} gain {per_pass(band, 'gain_us', 'gain_passes'):.2f}   "
              f"power {per_pass(band, 'power_us', 'power_passes'):.2f}")
    print(f"  corr(callback, power passes) {correlation(rows, 'us', 'power_passes'):.2f}   "
          f"corr(callback, gain passes) {correlation(rows, 'us', 'gain_passes'):.2f}")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    by_preset = defaultdict(list)
    with open(sys.argv[1], newline="") as f:
        for row in csv.DictReader(f):
            by_preset[row["preset"]].append(row)
    for preset, rows in by_preset.items():
        report(preset, rows)


if __name__ == "__main__":
    main()
