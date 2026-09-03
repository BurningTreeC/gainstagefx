# GainStageFx

Circuit modelled gain stages, from subtle preamplifier saturation to a scooped
high gain sound. CLAP and VST3, built with [nih-plug].

![The panel](docs/panel.png)

Nothing here is a filter shaped to sound like an amplifier. Every voice is a
netlist — resistors, capacitors, inductors, valves, diodes, op-amps — solved by
modified nodal analysis, with Newton–Raphson wherever a part is not linear. The
sound comes out of the topology, so a capacitor in the gain leg makes the
mid-hump because that is what it does in the hardware, not because someone drew
the curve.

## The panel

Six numbered bands in the order the signal goes through them, with an arrow at
the foot of each pointing into the next.

| | | |
|---|---|---|
| **1 Input** | Trim, and a meter | The meter reads against the level the circuits were voiced at. Its zero is where the rest of the panel means what it says. |
| **2 Circuit** | Topology, clipping | What does the work. The clipping row greys out on the valve circuits, which have no diodes in them. |
| **3 Drive** | Drive | All the way up is the sound the circuit is named for. Down from there only cleans up. |
| **4 Tone** | Stack, bass, mid, treble | A passive stack, so it only ever cuts. The scooping voicing has a resonant leg. |
| **5 Cabinet** | Off, combo, stack | A speaker is most of what a distorted amplifier sounds like. |
| **6 Output** | Mix, level | The dry path is delayed to match, so mixing is a mix and not a comb filter. |

## The circuits

| Voice | What it is | Measured at full drive |
|---|---|---|
| Clean | One valve stage barely working | 1.5 % distortion, almost all second harmonic |
| Crunch | Two stages, the second driven by the first | 8 % |
| High Gain | Three stages, all clipping on every note | 25 %, 11.7 % third harmonic |
| Overdrive | Diodes across the feedback resistor | 12 %, and it cleans up when hit harder |
| Distortion | Diodes across the signal to ground | 30 %, a ceiling the output stops at |

The two clipper families are not a matter of degree. In the loop, the diodes
lower the *gain*, so the output never stops following the input — which is why
an overdrive on a hot signal barely does anything. To ground, they are a
ceiling: past it the waveform is squared off and the harmonics fill out to the
top of the band.

Silicon, germanium and LED are selectable wherever there are diodes. They differ
by forward voltage, which is the whole reason anyone swaps them: measured,
germanium is already bending at 20 mV where an LED has not started, and the LED
overtakes it when driven hard.

## Presets

Twenty-six, in five groups, ordered quietest first so the list reads as a range.
Measured end to end the catalogue runs from 0.03 % distortion to 26 %.

Each one is a full set of panel positions — loading one and looking at the panel
tells you how the sound is made. **Scooped Metal** is three cascaded stages, the
scooping voicing with the mid control right down, and a stack behind it.

Save your own with the button beside the name; they go to
`~/.config/gainstagefx/presets` as readable JSON and appear under their own
**Saved** heading at the foot of the list. Only those can be deleted — saving
under a shipped preset's name writes a new file beside it rather than replacing
it, so the shipped one never becomes unreachable. A dot next to the name means
the panel has been moved since the preset was loaded; move the control back and
the dot goes, because that state is compared rather than remembered.

Loading a preset goes out as ordinary parameter gestures, so it lands in a DAW's
automation lanes and undoes in one step like any other edit.

## Building

```sh
cargo xtask bundle gainstagefx --release   # CLAP and VST3 in target/bundled
cargo test --release                        # the measurements
./install.sh                                # into the usual places
```

Every claim in this README that has a number in it is checked by a test that
measures it. `cargo test --release` runs seventy of them.

## How it is built

`src/dsp` is the engine and knows nothing about audio plugins:

- **`netlist`** — circuits as named nodes, validated before use. A dangling node
  or an unreachable output is a `Fault`, not a silent wrong answer.
- **`ac`** — stamps the matrix with complex admittances and solves per
  frequency. Exact, in microseconds, no audio involved. Refuses to run on a
  circuit containing a device, because a device has no single frequency
  response.
- **`time`** — trapezoidal companion models, Newton–Raphson with junction
  voltage limiting, and a separate DC matrix for the operating point with
  capacitors open and inductors shorted.
- **`device`** — the nonlinear parts: Koren triode, Shockley diode, and a
  two-state op-amp. Parts that impose a voltage rather than a conductance get a
  branch unknown of their own.
- **`measure`** — bin-aligned probe tones with phase continuous across the
  settle boundary.

The two solvers overlap deliberately. On a linear network they must agree, and
`tests/agreement.rs` is the only real check either of them has.

## What the measurements caught

The discipline here is that a number nothing can disagree with is not a
measurement. Several times the thing that turned out to be wrong was the
measurement, and several times it was me:

- A linear network read 2.6 % distortion because the probe tone's phase
  restarted between settling and capture. Nothing was wrong with the circuit.
- A gain control moved the Clean voice 6 dB end to end, on the reasoning that a
  gain control sets how much local feedback each stage keeps. It does not: a
  cathode bypass is bounded by the cathode resistor. It is a volume pot now, and
  the range is 80 dB.
- A linear rheostat cannot sweep evenly in decibels — for a span of 48 it put 18
  of its 30 dB into the first quarter of the travel and 2 into the last. The
  track has to be shaped for the span it covers.
- The oversampler handed the circuit four samples for every one the host sent
  while the circuit still solved with the host's timestep, putting every corner
  frequency four times too high. It cost the overdrive 11 dB and the valve
  voices almost nothing, so nothing but the clipper would have shown it.
- An operating point converged, satisfied its own tolerance, and was wrong,
  because the matrix was stale. Kirchhoff caught it; convergence could not.

[nih-plug]: https://github.com/robbert-vdh/nih-plug

## Licence

GPL-3.0-or-later.
