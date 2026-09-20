# Metal Zone (Boss MT-2) research log

Engineering identity: **Boss MT-2 Metal Zone** (1991-). Stable id `pedal_boss_mt2`
(pedal slot). Display: **Metal Zone**.

Status: **IMPLEMENTED 2026-09-16** (`src/circuits/metal_zone.rs`), tested in
`tests/metal_zone.rs`, and in the pedal slot as **Metal Zone** (`pedal_boss_mt2`) with all
six controls: Dist, Low, Middle, Mid Freq, High and Level.

## Eight-question checkpoint

1. **Revision.** The MT-2 as Boss drew it on `VR BOARD (PCB 22930117RT 2/2)` with the
   `LED BOARD 752755400`: six controls (Level, Dist, High, Mid Freq, Middle, Low). Not the
   2019 MT-2W Waza.
2. **Original schematic found?** **Yes** -- Boss's own drawing, read directly
   (project-local `docs/schematics/boss-mt2.png`, git-ignored). It names every part
   *including* the semiconductors, which the HM-2 scan does not.
3. **Best source.** That drawing.
4. **Cross-check.** [Electric Druid's MT-2 analysis](https://electricdruid.net/boss-mt-2-metal-zone-pedal-analysis/)
   (Tom Wiltshire), which works the filters out from the same designators and agrees with
   the sheet part for part. Its figures are what the model is measured against:
   - pre-distortion "mid hump" peaking near **1 kHz** at about **36 dB** of gain;
   - gain stage **x2 to x252** (VR01 250k with R051 1k), rolled off above **13 kHz** by
     C028 47p;
   - post-distortion gyrators at **4,898 Hz Q 2.2** and **105 Hz Q 14.6** -- the double
     peak that is the scoop;
   - Low control a gyrator at **105 Hz Q 3.1**, **±20 dB**; High a shelf, **±20 dB**;
   - Middle a Wien-bridge semi-parametric, **±15 dB**, sweeping **240 Hz to 4.7 kHz**
     boosting and to 6.3 kHz cutting, at constant Q.
   - Its conclusion is the design brief: "It's really all about frequency response ...
     those frequency responses determine the sound of the pedal, much more than diode
     choice ever would."
   - A second reading of the same sheet:
     [Matthieu Brucher's analysis](https://blog.audio-tk.com/2019/10/22/analysis-of-the-boss-mt2-metal-zone-pedal-1/).
5. **Signal path and values** (Boss's own designators).

```text
INPUT -- R059 10k -- C042 .047 -- Q011 2SK184GR buffer, R058 1M, R060 10k
pre-distortion shaping, op-amp 3b (1/2 M5218AL):
     C033 .015 + R043 100k (106 Hz), loop R044 220k with C032 100p (6 dB/oct above 7.2 kHz)
     gyrator on Q010 2SC3378GR: R046 2.2k, C034 .027, R053 47k, C035 .01, R054 10k
     Q009 2SK118Y with R052 100k, R047 1M, C039 1uF, D005 1SS133
  -- R045 10k / R042 10k divider, C029 .033, C031 .047, R040 100k
gain, op-amp 3a: C027 10uF, R051 1k, VR01 250kA (Dist), C028 47p, R041 1k, C030 10uF
clipping: D004 and D003 1SS133 to ground; R033 2.2k; R032 10k / R031 4.7k / C023 .015
post-distortion, two gyrators on Q008 and Q007 2SC3378GR:
     R034 1k, C024 .015, R036 47k, C025 .0015, R037 10k
     R027 470, C020 .22/35, R025 470k, C017 .047, R024 10k
  -- op-amp 4b (R030 3.3k, C022 47p, R029 100k) -- R028 22k -- op-amp 4a (R026 22k,
     C018 10p) -- R014 22k
tone, on the VR board:
     HIGH   VR03b 100kG with R061 2.2k, C044 .01, C010 10p, R015 22k -- op-amp 1a
     LOW    VR03a 100kG with R012 2.2k, C008 .22/35, R013 100k, C009 .047 -- op-amp 1b
     MIDDLE VR02b 100kG, R038 47k, R035 47k, C026 100p -- op-amp 2a
     MID FREQ VR02a 50kCx2 (dual gang) with C036 .022, C043 .0082, R048 2.2k, R062 2.2k
              -- op-amp 2b as the Wien-bridge buffer
     C011 1uF, C037 1uF, C038 .1, R050 330, R049 330, R039 1M
  -- VR04 50kA (Level) -- C005 10uF -- Q002 2SK118Y -- Q001/Q003/Q004 output buffer chain
     (2SC3378GR, 2SK118Y, 2SC3378GR) with R001 1k, R002 10k, R003 100k, R004 100k,
     C001 10uF, C002 10uF, C003 1uF
supply: 9 V through D006 S5500G, R056/R057 10k make 4.5 V, C040/C019 100uF
bypass: Q005/Q006 2SC2458GR flip-flop -- switching, not audio
```

6. **To be modeled exactly.** The whole audio path: the JFET buffer, the pre-distortion
   gyrator hump, the gain stage and its 1SS133 clipping pair, both post-distortion
   gyrators, and all four tone controls including the **sweepable mid** -- which is a
   control the plugin has nowhere else and the reason this pedal is worth building.
7. **To be approximated, and why.**
   - **M5218AL** op-amps: a 10 MHz, 5 V/us dual. The existing `Transconductor`
     construction (as used for the Rodent's LM308 and the Yellow Dist's 741) carries those
     two numbers; input offset and noise are not modelled. PUBLISHED-PARAMETER DERIVED.
   - **1SS133** diodes: small-signal silicon switching diodes. No SPICE model located, so
     they take 1N4148-class constants. APPROXIMATED.
   - **2SK184GR / 2SK118Y** JFETs and **2SC3378GR** NPNs: the catalogue's JFET and
     Ebers-Moll NPN with data-sheet-class parameters; the 2SC3378 is already in the
     netlist catalogue ("high beta, low noise"). APPROXIMATED where a curve was not found.
   - The Q005/Q006 bypass flip-flop is switching and is not built.
8. **Why.** No manufacturer device models for the Japanese semiconductors; everything else
   is on the drawing.

## What it is for

Two things the plugin does not have: a **semi-parametric mid** with a swept centre, and a
distortion whose character is made almost entirely of filters rather than of diodes. It is
also the other half of the pair the repository has been carrying schematics for without
code.

## What the build found

**The band arithmetic confirms the reading.** Every gyrator was worked out from the
drawing's values as `L = R1 x R2 x C` and paired with its series capacitor:

| | from the drawing | L | with | resonance | published |
|---|---|---|---|---|---|
| the hump | 47k, 2.2k, .01 | 1.034 H | .027 uF | **953 Hz** | "around 1 kHz" |
| post high | 47k, 1k, .015 | 0.705 H | .0015 uF | **4894 Hz** | 4,898 Hz |
| post low | 470k, 470, .22 | 48.6 H | .047 uF | **105 Hz** | 105 Hz |
| Low control | 100k, 2.2k, .22 | 48.4 H | .047 uF | **106 Hz** | 105 Hz |

Four numbers derived from the sheet landing on an independent publication is the check this
model rests on.

**R030 belongs in series with the post-distortion legs, not across them.** Across, it is a
gain leg of its own and the stage lifts everything by thirty-one times, resonance or not,
which is a gain stage rather than a scoop. In series, the legs are a high impedance away
from their resonances and the stage sits at unity everywhere but the two ends of the band.

**The Dist rheostat runs backwards on a forward taper.** Its wiper is tied to `b`, so what
is in circuit is the a-to-wiper leg, `R x (1 - f)`; with `Taper::Audio` the knob turned the
gain down. Measured before the fix: 76 dB with the control shut and 34 dB with it open.

### The Mid Freq control

The original sweeps its middle with a **Wien bridge** on a dual-gang pot. What makes that a
*parametric* mid rather than merely a moving one is two properties together: the centre
goes as `1/R`, and the Q does not move with it.

Two arrangements were built and measured before the one that ships:

1. **A gyrator with one swept resistance.** `L = R1 x R2 x C` gives a centre going as
   `1/sqrt(R)` -- the drawing's own 50 k gang then covers about a fifth of the published
   240 Hz - 4.7 kHz span -- and a Q of `2 pi f0 R C`, so sweeping the centre sweeps the
   sharpness. Measured, it was a broad low-mid lift moving from 560 Hz to 380 Hz across the
   whole knob, which is not a parametric mid by any reading.
2. **The Wien network with its buffer bootstrapping the leg.** Faithful to the drawing, and
   the centre did move correctly; but the depth goes as `1 / (1 - G/3)` and the loop
   oscillates at `G = 3`, so a usable margin left a band a decibel deep on a rising shelf.

What ships is **both gang sections sweeping one gyrator**, which is what the dual gang is
for:

- `L = R^2 C_gyr`, so `f0 = 1 / (2 pi R sqrt(C_gyr Cs))` -- proportional to `1/R`, the Wien
  bridge's own law;
- the leg's series resistance moves with it, so `Q = sqrt(C_gyr / Cs)` -- **a constant, set
  by the two capacitors and nothing else.**

With the drawing's C036 .022 against a .01 series capacitor that is a Q of 1.48 held across
the sweep, and 2.2 k to 52.2 k of gang gives **4877 Hz down to 206 Hz**, against a published
4.7 kHz to 240 Hz.

**What it does not hold is the depth.** The leg's series resistance is the swept one, so the
band is deep at the top of the sweep (+14 dB, against a published +-15) and shallow at the
bottom (about 5 dB at noon), where the original's is even. Holding both the law and the
depth needs the Middle to be its own stage with the Wien network in its feedback, rather
than a leg on the shared equaliser track; that is the next piece of work on this pedal.

## Measured (`tests/metal_zone.rs`)

- It solves, and silence stays silent.
- The pre-clipper hump peaks at 950 Hz, more than 10 dB above 100 Hz, 300 Hz and 3 kHz --
  which is what decides *which* part of the guitar gets distorted.
- The Dist control spans more than 20 dB at the gain stage's own output.
- Each tone control owns its band: low, middle and high each have more than 10 dB of range
  where they live, and each does more there than the others do.
- The post-distortion stage is double-peaked, lifting 105 Hz and 4.9 kHz more than 6 dB
  above 700 Hz: the scoop.
- The Mid Freq control moves the band: turned up it lifts 4 kHz far more than 250 Hz, and
  turned down that difference collapses.
- Unity through the pedal is at **level 0.45**, measured broadband. For this pedal that is
  eight decibels away from what a single 220 Hz tone would have said -- it is shaped hard
  enough that one frequency says very little about how loud it is -- and it puts it 1.6 dB
  above no pedal at all, in the middle of the list.

## Original middle EQ completion checkpoint (2026-09-20)

The production pedal still uses the approximate gyrator described above. Codex has
now implemented the bounded **isolated U2a/U2b mid-EQ netlist** as
`metal_zone::mid_eq_reference`, plus boost/cut, center-flat and multi-rate
small-signal AC/time tests. Those tests still need to be executed on the development
machine; headroom/state/loading validation and production integration remain next.

1. **Revision:** original MT-2, Boss Service Notes **April 1991**, MT board
   75275252000 / PCB 22930117RT 1/2 and VR board 22930117RT 2/2. Not MT-2W.
2. **Original schematic:** found and visually read on page 3 of the
   [four-page Boss service notes](https://guitar-gear.ru/forum/index.php?app=core&attach_id=50406&module=attach&section=attach).
   This adds a reproducible online provenance to the pre-existing local PNG.
3. **Best source:** that factory drawing. Its scan has no extractable text;
   render page 3 with Poppler and enlarge the lower-right U2a/U2b network.
4. **Cross-check:** the published analysis already cited above reproduces the
   same designators. The service appendix (page 4) also supplies a future complete
   pedal validation fixture: 200 Hz, 5 mV peak-to-peak square-wave input and six
   control settings with oscilloscope traces. These traces have not yet been
   digitized and are not a claimed validation result.
5. **Values/topology, DOCUMENTED:** EQ output feeds C011 1 uF, then R038 47k to
   U2a minus. R035 47k and C026 100 pF connect U2a output to minus. R050 330 ohm
   connects the post-C011 input to VR02b pin 1. U2a output goes through C037 1 uF
   and R049 330 ohm to VR02b pin 3. VR02b is 100kG; its wiper feeds U2b plus.
   U2b is a follower: its output feeds C036 22 nF, R048 2.2k and the first 50kC
   gang in series to the bridge node. The second 50kC gang and R062 2.2k return
   that node to ground; C043 8.2 nF also connects it to ground. C038 100 nF
   connects the bridge node to U2a plus, with R039 1M to the 4.5 V reference.
   Both gang wipers are tied to pin 1. Level is downstream of U2a.
6. **Exact mapping:** retain the separate Middle stage and every listed R/C,
   both gang sections and both amplifiers. Measure at U2a's output, before Level.
   Model low/high-EQ drive as a specified source resistance and the Level network
   as a specified load. These ports permit subsequent integration without
   pretending that arbitrary source/load values are factory components.
7. **Approximations:** use the catalogue's rail-aware ideal op-amps, +/-4 V
   around the ground-referenced bias, as in the current pedal; M5218AL GBW/slew,
   input currents and noise remain absent. The drawing says C and G taper but
   supplies no measured taper curves. For this isolated engineering fixture,
   Mid Freq is explicitly **linear in electrical gang resistance**, and Middle
   retains the existing estimated symmetric law. Endpoint and center tests do
   not authenticate mechanical knob tracking. An eventual panel integration
   must reconcile the C-taper law and legacy session behavior explicitly.
8. **Why:** the current mid gyrator matches an approximate center-frequency law
   but loses boost/cut depth at the low end. The factory network supplies that
   missing physical feedback structure. This is circuit completion, not a solver
   optimization and not grounds for replacing nonlinear amplifiers with linear
   devices in the shipping path. AC tests may linearize the two ideal amplifiers
   around zero solely to measure their small-signal response.
