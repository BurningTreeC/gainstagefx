#!/usr/bin/env python3
"""Read GainStageFx realtime traces and say what the host actually did.

A trace is one CSV per plugin instance, written by `src/rt_trace.rs` when the
host runs with GAINSTAGEFX_RT_TRACE=1 (and optionally GAINSTAGEFX_RT_TRACE_DIR).
Every row is one host callback with CLOCK_MONOTONIC start/end timestamps, the
thread and CPU it ran on, per-stage time and per-stage solver work.

    python3 tools/rt_trace.py /tmp/gainstagefx-trace/gainstagefx-rt-*-instance-*.csv

For each instance it reports the callback-time tail, the callback cadence the
host really delivered, the gaps in it (a gap after a callback is the host's
xrun recovery: that audio was lost), how likely a gap is after a callback of a
given length and in which cadence slot, deadline aborts and their bursts, and
which stage and how much solver work grows in the slow callbacks.

For two or more instances it lines their callbacks up on the shared clock and
says whether they overlapped, ran one after the other, or neither; the start
offset between them; whether they shared a thread, a core or an SMT pair; how
much of the time both were busy; and whether a gap follows the *sum* of their
durations (serialised) or the *larger* of them (parallel).

Files from before the timestamped format (no start_ns column) are accepted:
time is rebuilt from the intervals, and instances are aligned on the gaps they
share. That alignment is approximate and is reported as such.

Standard library only.
"""

import argparse
import bisect
import csv
import math
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path

STAGES = ("pedal", "gain", "power", "iron", "tone", "cabinet", "other")
SOLVED = ("pedal", "gain", "power", "iron")


def pct(values, p):
    if not values:
        return float("nan")
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(round(p / 100 * (len(ordered) - 1))))]


def spread(values):
    return (
        f"mean {statistics.fmean(values):7.1f}  p50 {pct(values, 50):7.1f}  "
        f"p95 {pct(values, 95):7.1f}  p99 {pct(values, 99):7.1f}  "
        f"p99.9 {pct(values, 99.9):7.1f}  max {max(values):7.1f}"
    )


def number(text):
    try:
        return float(text)
    except ValueError:
        return text


class Instance:
    def __init__(self, path):
        self.path = Path(path)
        with open(path, newline="") as f:
            self.rows = [{k: number(v) for k, v in row.items()} for row in csv.DictReader(f)]
        if not self.rows:
            raise SystemExit(f"{path}: no callbacks")
        self.absolute = "start_ns" in self.rows[0]
        if self.absolute:
            self.start = [r["start_ns"] / 1000.0 for r in self.rows]
            self.end = [r["end_ns"] / 1000.0 for r in self.rows]
        else:
            t, self.start = 0.0, []
            for r in self.rows:
                t += r["callback_interval_us"]
                self.start.append(t)
            self.end = [s + r["process_us"] for s, r in zip(self.start, self.rows)]
        self.duration = [r["process_us"] for r in self.rows]
        self.interval = [r["callback_interval_us"] for r in self.rows]
        samples = self.rows[0].get("samples", 64)
        rate = self.rows[0].get("sample_rate", 48000.0) or 48000.0
        self.period = samples / rate * 1e6
        self.label = self.path.stem.replace("gainstagefx-rt-", "")

    def column(self, name, default=0.0):
        return [r.get(name, default) for r in self.rows]

    def gaps(self, threshold):
        return [i for i in range(1, len(self.rows)) if self.interval[i] > threshold]


def cadence(inst, gap):
    """The interval pattern the host delivered, and a slot predictor.

    Intervals are quantised to 100 us. The most common successor of each pair
    of preceding (non-gap) intervals predicts the slot a callback is about to
    run in -- on a USB interface with 0.5 ms packets and a 64-frame quantum
    that is 1.5, 1.5, 1.0 ms, and the 1.0 ms slot is the one that bites.
    """
    q = [round(v / 100) * 100 for v in inst.interval]
    following = defaultdict(Counter)
    for i in range(2, len(q)):
        if max(inst.interval[i - 2 : i + 1]) < gap:
            following[(q[i - 2], q[i - 1])][q[i]] += 1
    predict = {k: c.most_common(1)[0][0] for k, c in following.items() if sum(c.values()) >= 20}

    def slot(i):
        if i < 2 or max(inst.interval[i - 1 : i + 1]) >= gap:
            return None
        return predict.get((q[i - 1], q[i]))

    common = Counter(v for v in q[1:] if v < gap).most_common(4)
    return common, slot


def report_instance(inst, args):
    gap = args.gap_us or 3 * inst.period
    rows = inst.rows
    presets = Counter(r.get("preset", "") for r in rows if r.get("preset"))
    voices = Counter(r.get("voice", "?") for r in rows)
    modes = Counter(r.get("mode", "?") for r in rows)
    print(f"\n=== {inst.label}  ({len(rows)} callbacks, period {inst.period:.1f} us"
          f"{'' if inst.absolute else ', OLD FORMAT: time rebuilt from intervals'})")
    if presets:
        print("  preset   " + ", ".join(f"{k} ({v})" for k, v in presets.most_common(3)))
    if "voice" in rows[0]:
        print("  voice    " + ", ".join(f"{k} ({v})" for k, v in voices.most_common(3))
              + "   mode " + ", ".join(f"{k} ({v})" for k, v in modes.most_common(3)))

    reservoir = int(rows[0].get("reservoir_samples", 0) or 0)
    if reservoir:
        # Behind a reservoir the rows are the worker's: its time per host
        # call, judged against the reservoir, not the host's period. The
        # host callback itself takes microseconds and is not traced.
        print(f"  RESERVOIR {reservoir} samples: rows are the DSP worker's, not host callbacks")
        print(f"  worker queue us  {spread([r.get('queue_us', 0.0) for r in rows])}")
    print(f"  {'worker' if reservoir else 'callback'} us  {spread(inst.duration)}")
    if "pipeline" in rows[0]:
        use = Counter(r["pipeline"] for r in rows)
        print("  second half ran on: " + ", ".join(f"{k} {100 * v / len(rows):.1f} %" for k, v in use.most_common()))
    wall = inst.end[-1] - inst.start[0]
    audio = sum(r.get("samples", 64) / (r.get("sample_rate", 48000.0) or 48000.0) for r in rows) * 1e6
    gaps = inst.gaps(gap)
    lost = sum(inst.interval[i] - inst.period for i in gaps)
    print(f"  wall {wall / 1e6:.1f} s for {audio / 1e6:.1f} s of audio;"
          f" {len(gaps)} gaps > {gap:.0f} us losing {lost / 1e6:.2f} s"
          f" ({len(gaps) / max(wall / 1e6, 1e-9):.1f} per second)")
    late = sum(1 for v in inst.interval[1:] if v > 1.5 * inst.period)
    print(f"  late starts (> 1.5 periods after the last): {late}")
    for limit in (args.short_us, inst.period):
        over = sum(d > limit for d in inst.duration)
        print(f"  callbacks over {limit:7.1f} us: {over} ({100 * over / len(rows):.2f} %)")

    common, slot = cadence(inst, gap)
    print("  cadence (100 us bins): " + ", ".join(f"{k:.0f} us x{v}" for k, v in common))

    # How likely is a gap after a callback of a given length, per predicted slot?
    table = defaultdict(lambda: defaultdict(lambda: [0, 0]))
    for i in range(len(rows) - 1):
        s = slot(i)
        if s is None:
            continue
        band = min(int(inst.duration[i] // 100) * 100, 1600)
        cell = table[s][band]
        cell[0] += 1
        cell[1] += inst.interval[i + 1] > gap
    if table:
        print("  P(gap next | duration, next slot)  -- the effective deadline:")
        for s in sorted(table):
            cells = " ".join(
                f"{b:>4}:{c[1] / c[0]:.2f}({c[0]})" for b, c in sorted(table[s].items()) if b >= 500
            )
            print(f"    slot {s:>5.0f} us  {cells}")

    if "deadline_aborts" in rows[0]:
        aborts = inst.column("deadline_aborts")
        hit = [a > 0 for a in aborts]
        longest = run = 0
        for h in hit:
            run = run + 1 if h else 0
            longest = max(longest, run)
        stages = Counter()
        for r in rows:
            mask = int(r.get("abort_stages", 0) or 0)
            for bit, name in ((1, "pedal"), (2, "gain"), (4, "power"), (8, "iron"), (16, "line")):
                if mask & bit:
                    stages[name] += 1
        print(f"  deadline aborts: {int(sum(aborts))} samples in {sum(hit)} callbacks;"
              f" abort samples {int(sum(inst.column('abort_samples')))};"
              f" longest run of aborting callbacks {longest}"
              + (f"; stages {dict(stages)}" if stages else ""))

    # Where the slow callbacks' time goes: more work, or the same work slower?
    order = sorted(range(len(rows)), key=lambda i: inst.duration[i])
    n = len(order)
    bands = [("p0-50", order[: n // 2]), ("p50-90", order[n // 2 : int(n * 0.9)]),
             ("p90-99", order[int(n * 0.9) : int(n * 0.99)]), ("p99+", order[int(n * 0.99) :])]
    present = [s for s in STAGES if f"{s}_us" in rows[0]]
    print("  band      n   total | " + " ".join(f"{s:>7}" for s in present)
          + " | " + " ".join(f"{s[:5]}_ps" for s in SOLVED if f"{s}_passes" in rows[0])
          + "  pw_bt | ns/pass gain power")
    for name, idx in bands:
        if not idx:
            continue
        m = lambda k: statistics.fmean(rows[i].get(k, 0.0) or 0.0 for i in idx)
        stage = " ".join(f"{m(f'{s}_us'):7.0f}" for s in present)
        passes = " ".join(f"{m(f'{s}_passes'):8.0f}" for s in SOLVED if f"{s}_passes" in rows[0])
        per = lambda s: 1000 * m(f"{s}_us") / max(m(f"{s}_passes"), 1e-9)
        print(f"  {name:7s} {len(idx):5d} {m('process_us'):7.0f} | {stage} | {passes} "
              f"{m('power_backtracks'):6.1f} | {per('gain'):7.0f} {per('power'):5.0f}")

    if inst.absolute:
        tids = Counter(int(r["tid"]) for r in rows)
        cpus = Counter(int(r["cpu_start"]) for r in rows)
        moves = sum(rows[i]["cpu_start"] != rows[i - 1]["cpu_end"] for i in range(1, n))
        inside = sum(r["cpu_start"] != r["cpu_end"] for r in rows)
        print(f"  threads {dict(tids.most_common(4))}  cpus {dict(cpus.most_common(6))}")
        print(f"  migrations between callbacks {moves}, during a callback {inside}")


def core_of(cpu):
    try:
        return int(Path(f"/sys/devices/system/cpu/cpu{cpu}/topology/core_id").read_text())
    except (OSError, ValueError):
        return cpu


def align_old(a, b, gap):
    """Offset that best lines up two old-format traces' shared gaps (approximate)."""
    ga = [a.start[i] for i in a.gaps(gap)]
    gb = [b.start[i] for i in b.gaps(gap)]
    if not ga or not gb:
        return None, 0
    votes = Counter(round((x - y) / 200) for y in gb[:500] for x in ga)
    coarse = votes.most_common(1)[0][0] * 200

    def score(d):
        hits = 0
        for y in gb:
            j = bisect.bisect_left(ga, y + d - 300)
            hits += j < len(ga) and ga[j] <= y + d + 300
        return hits

    best = max((coarse + k * 10 for k in range(-40, 41)), key=score)
    return best, score(best)


def report_pair(a, b, args):
    gap = args.gap_us or 3 * a.period
    print(f"\n=== {a.label}  x  {b.label}")
    offset = 0.0
    if not (a.absolute and b.absolute):
        offset, matched = align_old(a, b, gap)
        if offset is None:
            print("  old-format traces with no gaps to align on; cannot pair")
            return
        print(f"  OLD FORMAT: aligned on shared gaps, {matched}/{len(b.gaps(gap))} matched;"
              " per-callback pairing is approximate")
    starts = a.start
    pairs = []
    for j, s in enumerate(b.start):
        x = s + offset
        k = bisect.bisect_left(starts, x)
        best = min((i for i in (k - 1, k) if 0 <= i < len(starts)),
                   key=lambda i: abs(starts[i] - x), default=None)
        if best is not None and abs(starts[best] - x) < a.period / 2:
            pairs.append((best, j))
    if not pairs:
        print("  no callbacks in common")
        return
    print(f"  paired callbacks: {len(pairs)} of {len(b.rows)}")
    delta = [b.start[j] + offset - a.start[i] for i, j in pairs]
    print(f"  start of B - start of A, us: p1 {pct(delta, 1):.1f}  p50 {pct(delta, 50):.1f}"
          f"  p95 {pct(delta, 95):.1f}  p99 {pct(delta, 99):.1f}")

    kinds = Counter()
    for i, j in pairs:
        sa, ea = a.start[i], a.end[i]
        sb, eb = b.start[j] + offset, b.end[j] + offset
        overlap = min(ea, eb) - max(sa, sb)
        if overlap <= 0:
            kinds["serial, A then B" if sb >= ea else "serial, B then A"] += 1
        elif abs(sb - sa) < 50:
            kinds["parallel (started within 50 us)"] += 1
        else:
            kinds["partial overlap"] += 1
    for k, v in kinds.most_common():
        print(f"    {k:34s} {100 * v / len(pairs):6.2f} %")
    if a.absolute and b.absolute:
        # A host that runs the two one after the other starts the second a few
        # microseconds after the first ends, whichever thread it uses.
        wait = sorted(
            (b.start[j] - a.end[i]) if b.start[j] >= a.start[i] else (a.start[i] - b.end[j])
            for i, j in pairs
        )
        print(f"  later start minus earlier end, us: p5 {pct(wait, 5):.1f}  p50 {pct(wait, 50):.1f}"
              f"  p95 {pct(wait, 95):.1f}  (a few us: the host serialises them)")
    first = ["A" if b.start[j] + offset >= a.start[i] else "B" for i, j in pairs]
    switches = sum(x != y for x, y in zip(first, first[1:]))
    print(f"  A starts first in {100 * first.count('A') / len(first):.1f} % of cycles;"
          f" order changes {switches} times")

    if a.absolute and b.absolute:
        same_tid = sum(a.rows[i]["tid"] == b.rows[j]["tid"] for i, j in pairs)
        cores = [(int(a.rows[i]["cpu_start"]), int(b.rows[j]["cpu_start"])) for i, j in pairs]
        same_cpu = sum(x == y for x, y in cores)
        smt = sum(x != y and core_of(x) == core_of(y) for x, y in cores)
        n = len(pairs)
        print(f"  same thread {100 * same_tid / n:.1f} %, same CPU {100 * same_cpu / n:.1f} %,"
              f" SMT siblings {100 * smt / n:.1f} %")
        # Of the wall time either was busy, how much were both busy?
        events = []
        for inst, shift in ((a, 0.0), (b, offset)):
            for s, e in zip(inst.start, inst.end):
                events += [(s + shift, 1), (e + shift, -1)]
        events.sort()
        busy = {1: 0.0, 2: 0.0}
        level, last = 0, events[0][0]
        for t, step in events:
            if level in busy:
                busy[level] += t - last
            level, last = level + step, t
        either = busy[1] + busy[2]
        print(f"  both busy {100 * busy[2] / max(either, 1e-9):.1f} % of the time either was")

    # Serial or parallel, by consequence: does a gap follow the sum or the max?
    for name, combine in (("sum", lambda x, y: x + y), ("max", max)):
        cells = defaultdict(lambda: [0, 0])
        for i, j in pairs:
            if j + 1 >= len(b.rows):
                continue
            v = combine(a.duration[i], b.duration[j])
            cell = cells[min(int(v // 100) * 100, 2400)]
            cell[0] += 1
            cell[1] += b.interval[j + 1] > gap
        print(f"  P(gap next | {name} of durations): "
              + " ".join(f"{k}:{c[1] / c[0]:.2f}({c[0]})" for k, c in sorted(cells.items())
                         if c[0] >= 20))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("traces", nargs="+")
    parser.add_argument("--gap-us", type=float, default=None,
                        help="interval treated as a gap (default: 3 host periods)")
    parser.add_argument("--short-us", type=float, default=930.0,
                        help="the short-cycle budget to count callbacks against")
    args = parser.parse_args()
    instances = [Instance(p) for p in args.traces]
    for inst in instances:
        report_instance(inst, args)
    for x in range(len(instances)):
        for y in range(x + 1, len(instances)):
            report_pair(instances[x], instances[y], args)


if __name__ == "__main__":
    sys.exit(main())
