#!/usr/bin/env python3
"""Per-preset A-B-B-A timing of `examples/rt_scenario`: one flag on against off.

A median over the catalogue hides a preset that got worse: the speculative
power stage moved every catalogue median by under 0.3 % while costing Puppet
Master '86 28 % at p99.9. This runs each preset four times -- with the flag,
without, without, with -- pinned to two cores, and reports each preset's own
change, so drift in the machine's load cancels and no preset hides.

    uv run tools/preset_ab.py --flag=--no-speculation --swap --all
    uv run tools/preset_ab.py --flag=--no-speculation --swap \\
        --preset "Jazz Chorus" --preset "Puppet Master '86"

`--flag` is what side A adds to the command line; `--swap` reports B against A
instead (for a flag that turns something *off*). Anything after `--` is passed
to every run; `--pipeline` is the default because that is how the plugin runs
a heavy preset. Build first: `cargo build --release --example rt_scenario`.
The max column is a single callback and moves by several times between
identical runs; read p99 and p99.9.
"""

import argparse
import re
import statistics
import subprocess
import sys

BIN = "./target/release/examples/rt_scenario"
COLUMNS = ["mean", "p50", "p95", "p99", "p99.9", "max"]
LINE = re.compile(
    r"mean +([\d.]+) +p50 +([\d.]+) +p95 +([\d.]+) +p99 +([\d.]+) +p99\.9 +([\d.]+) +max +([\d.]+)"
)


def run(preset, extra, cpus):
    command = ["taskset", "-c", cpus, BIN, "--preset", preset, *extra]
    out = subprocess.run(command, capture_output=True, text=True, check=True).stdout
    return [float(v) for v in LINE.search(out).groups()]


def presets():
    out = subprocess.run(
        [BIN, "--all", "--seconds", "0.01"], capture_output=True, text=True, check=True
    ).stdout
    return [
        line.strip()
        for line in out.splitlines()
        if line.strip() and not line.startswith(" ") and not line.startswith("tests/")
    ]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--flag", required=True, help="what side A adds")
    parser.add_argument("--swap", action="store_true", help="report B against A")
    parser.add_argument("--preset", action="append", default=[])
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--seconds", default="8")
    parser.add_argument("--cpus", default="3,5", help="two distinct physical cores")
    args, passthrough = parser.parse_known_args()
    passthrough = [a for a in passthrough if a != "--"] or ["--pipeline"]
    names = presets() if args.all else args.preset
    if not names:
        sys.exit("name a --preset or pass --all")
    base = ["--seconds", args.seconds, *passthrough]
    header = "".join(f"{c:>18}" for c in COLUMNS)
    print(f"{'preset':28}{header}")
    changes = {c: [] for c in COLUMNS}
    for name in names:
        runs = {"a": [], "b": []}
        for side in "abba":
            extra = base + ([args.flag] if side == "a" else [])
            runs[side].append(run(name, extra, args.cpus))
        mean = {s: [statistics.fmean(r[i] for r in runs[s]) for i in range(6)] for s in runs}
        old, new = (mean["a"], mean["b"]) if args.swap else (mean["b"], mean["a"])
        cells = []
        for i, c in enumerate(COLUMNS):
            change = new[i] / old[i] - 1
            changes[c].append(change)
            cells.append(f"{old[i]:6.0f}->{new[i]:5.0f} {100 * change:+4.0f}%")
        print(f"{name:28}" + "".join(f"{cell:>18}" for cell in cells), flush=True)
    print("median change: " + "  ".join(
        f"{c} {100 * statistics.median(v):+.1f} %" for c, v in changes.items()
    ))


if __name__ == "__main__":
    main()
