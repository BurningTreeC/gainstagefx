# The catalogue's realtime audit — 2 October 2026

Every shipped preset, and the most expensive rig the panel lets a player dial in,
measured against the live deadline: 64 samples at 48 kHz, a 1,333 µs period, and on
the owner's REAPER rig a 1.0 ms USB cycle one callback in three, which drops out past
~930 µs (`docs/realtime-multi-instance.md`). The material is the fixture DI take
(`tests/fixtures/01-260912_1044.wav`, 19 s), at its recorded level and 6 dB hotter.

Machine: 16 cores, REAPER not running, nothing else busy; load average 2.4–5.5,
which is the measurement's own two threads and the build before it. Wall-clock
figures are this machine's; the solver counts beside them are not machine-dependent.

## How

- **Every preset** (`cargo run --release --example rt_scenario -- --all --plugin
  --table [--gain-db 6]`): played as the plugin plays a mono or dual-mono track --
  blocks through a `StageWorker`, pipelined or not by the plugin's own
  `PipelineGovernor` (factored out of `plugin.rs` for this, unchanged) -- back to
  back, deadline cutoff not armed, so every solve runs to the end and the counts
  are complete.
- **The worst rig** (`cargo run --release --example stress`): each category swept in
  turn, keeping its most expensive option by mean serial callback time; a second
  pass re-sweeps the circuit, power stage, pedal, cabinet and speaker. Everything a
  player can turn up is up: oversampling requested at 8x, drive and pedal drive at
  full, both microphones on. Then the rig is played over the whole take through the
  plugin path, back to back and paced like a host.

## Every preset

| | at the take's level | 6 dB hotter |
|---|---:|---:|
| median p99.9 | 453 µs | 482 µs |
| presets with any callback over 930 µs | 10 | 9 |
| presets with any callback over 1,333 µs | 4 | 2 |
| presets with unsettled samples | 1 | 1 |

90 of 100 presets never cross 930 µs at the take's level. Those that do, both
levels (callbacks over 930 µs / over the period; p99.9 and worst callback in µs):

| preset | 0 dB | +6 dB | what it is |
|---|---|---|---|
| American 800RB Clank | 143 / 41; 1,722, max 8,357; 62 unsettled, 3,736 fallbacks | **2,239 / 1,971; 40,136, max 47,224; 83,046 unsettled, 2.1 M fallbacks** | the 300 W transistor stage's solver collapsing under drive (below) |
| Twin, Modern Purple | 24 / 0; 949 | 67 / 1; 998 | a steady margin, not spikes: the Twin's whole netlist behind the six-op-amp pedal; where the time goes is still to be measured (`--stages`) |
| Swedish Death '90 | 4 / 0; 836 | 8 / 0; 905 | the same, smaller |
| Jazz Clean, Jazz Chorus | 2 / 0; 670 | 2 / 0; 777 | the JC-120's transistor power stage: 412–1,540 fallbacks |
| Brown '84, Thrash Rhythm, SVT Bass Driver DI, Plexi Orange Phase, Blizzard '80, Boutique Rhythm, Desert Deaf '02, The Great Wall '79 | 1–7 isolated | 0–1 isolated | not workload: they do not repeat between runs, and Brown '84 had its worst callbacks at the *quieter* level. Scheduling, not the solver |

## The most expensive rig

With the 800RB and its stage set aside (they dominate every comparison; below):

| category | most expensive | margin over the next |
|---|---|---|
| circuit | **High Gain** (a generic topology) | 6.0 vs 3.5 ms mean |
| power stage | **American 6550** (the SVT's) | 6.0 vs 3.3 ms |
| pedal | Treble Boost | ~10 % over Modern 33, Round Fuzz |
| wah | Off -- a wah in front *lowers* the cost, by thinning what reaches the fuzz | noise |
| cabinet / speaker / mic A / mic B | Closed 1x12 / Jazz 12 / FET Condenser 47 / Ribbon 38 | **within noise** (under 3 %) |

Played through the plugin path, oversampling 8x as it runs:

| | p50 | p99 | p99.9 | max | over 930 µs | over 1,333 µs | fallbacks |
|---|---:|---:|---:|---:|---:|---:|---:|
| 0 dB, back to back | 2,671 | 5,575 | 6,121 | 7,715 | 14,244 of 14,244 | 14,244 | 149,831 |
| +6 dB, back to back | 2,690 | 5,638 | 6,235 | 7,046 | all | all | 160,783 |
| 0 dB, paced | 2,681 | 5,567 | 6,169 | 6,849 | all | all | 149,857 |
| +6 dB, paced | 2,671 | 5,624 | 6,282 | 7,063 | all | all | 160,427 |

No unsettled samples: the rig is slow, not broken. **Every callback misses the
deadline.** What makes it so:

- **Oversampling.** `effective_oversampling` caps a *modelled* circuit at 2x; the
  topologies are not capped, and a power stage chosen from the override list runs
  inside the same oversampler. So High Gain with 8x requested and the American 6550
  behind it runs both at 8x.
- **The SVT's power stage driven hot**: 150,000 fallbacks in 19 s.
- **2x is not live-safe either, for a heavy chain.** In the first sweep, at 2x with
  full drive, the American Deluxe on its own stage averaged 1,117 µs serially, and
  with the SVT's stage and a fuzz in front 2,605 µs. Every shipped preset runs at 1x.
- **The acoustic side does not matter for the deadline**: every cabinet, speaker and
  microphone lands within the noise of the others.

## The 800RB at full

The first sweep picked the American 800RB, and it swamped everything after it: with
Volume and its BOOST (the panel's Master on the 800RB's own stage) both up and no
pedal, it averaged **8,739 µs a callback, p99.9 73,291 µs** -- six and a half times
slower than real time. The same 300 W stage chosen from the override list averaged
1,154 µs, because there the 800RB's BOOST goes back to its calibrated rest and the
stage is driven far less hard. With the shipped preset 6 dB hotter it does the same
(above). It is the transistor stage under hard clipping. The JC-120's transistor
stage also falls back (412–1,540 times over the take), with nothing unsettled;
whether the two share a cause is not yet established.

## Fixed: the junction-only node (GMIN), 3 October

The 800RB's collapse was one node. Q15's collector (`c15` in
`american_ss800`), the current limiter's, is joined to the rest of the circuit only
through Q15's collector junction and D8. `dsp::time::ss800_collapse` (an ignored
diagnostic) recorded the stage through the collapse and traced one solve pass by pass:
the residual was at round-off by the fourth pass, and every pass after it asked `c15`
to move -0.579 V -- because `c15` stood at **468,000 V**. The diode and transistor
stamps floored a reverse junction's slope at 1e-12 S in the Jacobian but not in the
current, so a node held only by off junctions had no answer within reach: Newton
walked toward one at `Is / 1e-12`, half a volt a pass, and every sample spent its 64
passes and fell back.

`dsp::device::GMIN`: SPICE's teraohm across every junction, in the current and the
slope alike. The node now has an answer where the two leaks balance, and a reverse
region is linear, so one pass finds it.

| American 800RB Clank, power stage | before | GMIN |
|---|---:|---:|
| 0 dB, 8 s: passes a solve / unsettled / fallbacks | 3.55 / 19 / 1,169 | 3.51 / 0 / 734 |
| +6 dB, 8 s | 7.57 / 23,456 / 736,662 | 3.82 / 0 / 2,932 |
| +6 dB: callback p99.9 / max | 47.7 / 48.4 ms | 2.5 / 3.0 ms |
| Volume and BOOST up, 2x: passes a solve / fallbacks | 17.26 / 544,843 | 4.15 / 10,641 |

Across the catalogue at +6 dB, unsettled samples 83,046 -> **0** and fallbacks 2.25 M
-> 143 k. Over 4 s of every preset, 65 of 100 are bit-identical and the rest move by
-98 to -137 dB re peak (Twin, Blue Chorus -87.5 dB: the CE-2's oscillator, its LFO
phase sliding by a hair). Both frozen baselines and every transistor and diode
circuit's tests pass. Timed interleaved against the previous build on a quiet machine,
the presets without the defect are unchanged. The JC-120's fallbacks rise 14 % (503 ->
574 at the take's level) with its passes a solve unchanged (3.248 -> 3.249).
`tests/american_800rb.rs::the_power_stage_does_not_collapse_driven_hot` holds it: 3.17
passes a solve and nothing unsettled now, 12.37 and 21,885 unsettled before.

What is left on the 800RB driven hot is clipping, the output going rail to rail, and a
tail on the same node: once it hangs half a volt under Q15's base through a junction
carrying nanoamperes, it follows that base through 80 V a sample by Newton's
one-thermal-voltage-a-pass descent on an exponential. Only a looser tolerance would end
that, and the tolerance is not the solver's to loosen.

The worst rig, searched again with GMIN in and nothing excluded (3 October): the
800RB is the most expensive circuit again, but at 1,274 µs a callback serially where it
was 8,739 -- a Round Fuzz in front of it, its own 300 W stage, the Legacy cabinet (the
resistor-loaded stage costs more than the speaker-loaded one driven this hard), two
ribbons, 2x as it runs. Through the plugin path: p50 1,573 µs, p99.9 5,365 µs, 12,463 of
14,244 callbacks over 930 µs, 11 samples unsettled (39 at +6 dB) and 112,133
fallbacks. Still unplayable live, now by load: a fuzz at full into the 800RB at full,
at 2x.

## The SVT's stage driven hot (looked at, not changed)

Behind the High Gain topology at full drive the American 6550 stage is fed 12 V where
its drawing gives 0.257 V for full power, 33 dB over: 3.2 % of solves are hard (10 or
more passes) and 3.2 % fall back, 4.36 passes a solve. `dsp::time::power_hard` (an
ignored diagnostic for any rig) shows why: on each edge the plates, the output grids,
the drivers and the followers move hundreds of volts in one sample (a pass's correction
of +800 V on a plate, -93 V on an output grid) -- honest work. One pattern looked like
the solver's: a driver grid crossing the grid-current kink at 0 V back and forth while
the line search crawled at a tenth of a step. **Tried and rejected**: landing a grid's
step on the kink instead of across it. Hard solves 12,239 -> 11,952, but fallbacks
12,388 -> 14,857 and passes a solve 4.355 -> 4.361. Reverted. What is left is how hard
the stage is driven and at what oversampling: the owner's call, not the solver's.

## What follows

In order of what a player can hit:

1. ~~The transistor power stages' convergence under clipping~~: the 800RB's collapse
   was the junction-only node, fixed above.
2. ~~The power stage's fallbacks when driven hot~~: honest work, looked at above.
3. **What the oversampling options promise.** The cap keys on `is_modelled`; the
   cost comes from whatever stage is in the oversampled path. Whether a chain with a
   power stage in it should be capped as a modelled circuit is, and whether 2x
   belongs on a live chain at all, is the owner's call: it is an option's range, not
   a solver fix.
4. **Twin, Modern Purple** at a steady margin: where its callbacks' time goes, by
   stage, before anything is changed.
