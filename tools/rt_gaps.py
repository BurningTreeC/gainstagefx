#!/usr/bin/env python3
"""Which callbacks the dropouts in a REAPER capture follow.

    python3 tools/rt_gaps.py /tmp/gainstagefx-trace/gainstagefx-rt-<pid>-instance-*.csv

A dropout shows up as a gap of more than 4 ms between one instance's callbacks,
and a device-level one shows up in every instance at once, so gaps within 3 ms
of each other are counted as one. For each dropout this prints every instance's
last callback before it: its duration, the slot it started in, its power-stage
passes and whether the pipeline ran it.
"""
import sys
import csv, bisect
files = sorted(sys.argv[1:])
if not files:
    sys.exit(__doc__)
inst = {}
for f in files:
    rows = []
    with open(f) as fh:
        for r in csv.DictReader(fh):
            rows.append((int(r['start_ns']), int(r['end_ns']), float(r['process_us']), r['preset'] if 'preset' in r else r['voice'], int(r['power_passes'] or 0), r['pipeline']))
    rows.sort()
    inst[f.split('instance-')[1].split('.')[0]] = rows
names = {k: max(set(r[3] for r in v), key=[r[3] for r in v].count) for k, v in inst.items()}
print("instances:", names)
# gaps per instance: next start - this start > 4 ms
events = []
for k, rows in inst.items():
    for a, b in zip(rows, rows[1:]):
        if b[0] - a[0] > 4_000_000:
            events.append((a[0], k))
events.sort()
# cluster gaps within 3 ms across instances = same device event
clusters = []
for t, k in events:
    if clusters and t - clusters[-1][0] < 3_000_000:
        clusters[-1][1].add(k)
    else:
        clusters.append([t, {k}])
t0 = min(r[0][0] for r in inst.values())
print(f"{len(events)} instance gaps -> {len(clusters)} distinct dropouts")
print(" time s   instances   | for each instance: last callback before the gap (us, slot us since previous start, power passes, pipeline)")
long_cause = 0
for t, ks in clusters:
    parts = []
    worst = 0
    for k, rows in sorted(inst.items()):
        starts = [r[0] for r in rows]
        i = bisect.bisect_right(starts, t + 1_000) - 1
        if i < 1:
            continue
        r = rows[i]; prev = rows[i-1]
        slot = (r[0] - prev[0]) / 1000
        worst = max(worst, r[2])
        parts.append(f"{names[k][:12]:12s} {r[2]:6.0f}us slot{slot:5.0f} pp{r[4]:4d} {r[5][:6]}")
    if worst > 900: long_cause += 1
    print(f"{(t-t0)/1e9:7.1f}  {','.join(sorted(ks)):9s} | " + " | ".join(parts))
print(f"dropouts where some instance's last callback exceeded 900 us: {long_cause} of {len(clusters)}")
