# Cali IIC+ / existing Cali 6L6 checkpoint

Target: existing stock RP10A C+ 60W interpretation, not ++ or Simul-Class.
Research 2026-09-14, before routing refactor. No component values changed.

Sources searched/read:
- [Archival manufacturer-labelled preamp schematic](https://schematicheaven.net/boogieamps/boogie_mkiic_plus.pdf)
- [RP10 redraw, same document available locally](https://www.scribd.com/document/917436296/RP10-IIC-Schematic)
- [Alternate FINAL redraw](https://cdn.imagearchive.com/boogieforum/data/attach/6/6870-Mark-IIC-Schematic-FINAL-1-.pdf)
- [Manufacturer manual index](https://legacy.mesaboogie.com/support/user-manuals.html)
- [RCA 6L6GC data](https://frank.pocnet.net/sheets/049/6/6L6GC.pdf)

The local RP10/FINAL are community redraws, NOT original factory schematics.
RP10 explicitly states its reference numbers are unofficial. Online FINAL fetch
failed; local existing copy is available. Modern reissue manuals do not establish
1983 circuit values. No fully authenticated original 60W power drawing verified.

Cross-check exposed substantive gaps: RP10 shows lead return/recovery/master
before EQ, while current code connects lead_out directly to the graphic network.
RP10 power drawing shows 82.5k/90.9k PI plates, 100k/150k grid leaks, 22k tail,
250k presence network and -47V 60W bias. Current PowerSpec uses 82k/100k plates,
1M/1M leaks, 10k tail, 5k presence and -44V bias. Its master is 1M resting .30.
The RCA data does not justify code's claim that 3.6k is its stated 450V pair load.
Do NOT claim the existing power spec exactly reproduces this drawing.

Preamp modeled values include 150k/100k/82k/274k plate loads, 1.5k/1.5k/1.5k/
3.3k cathodes, early 250pF/.1uF/.047uF stack, 680k lead resistor, 270k/68k divider,
410V assumed supply. RP10 R9=91k versus RP11A 100k is retained.

DSP: existing nonlinear netlists and RLC graphic preserved bit-for-bit for Matched.
Selection boundary is the existing lead_out -> graphic -> complete power netlist.
This is an implementation boundary; omitted recovery stages mean it is not the
complete physical preamp. Fixed load, transformer/supply constants and scalar EQ
recovery remain APPROXIMATED / EMPIRICALLY TUNED pending a versioned correction.

Eight-question checkpoint: device identified; original preamp located, full exact
power unverified; best source archival scan + revision-specific redraw; cross-check
performed and conflicts recorded; values above; exact preservation of old equations;
legacy approximations retained; necessary to separate routing from sound changes.
Tests: frozen multi-signal/rate/level fixture in tests/legacy_baseline.rs.

## Correction 2026-09-15: lead return, V2B, Lead Master, V2A

**Trigger.** Owner report: the Puppet Master '86 preset (Mark IIC+) "doesn't sound like
Metallica at all, there's way too less distortion".

**Finding.** Both local drawings (`RP10 IIC+ Schematic.pdf`, `Mark IIC+ Schematic FINAL.pdf`)
return `LEAD OUTPUT` through LDR3A to the V2B grid. From there the path is:

```text
LEAD OUTPUT -- (LDR3A lit) -- V2B grid; R10 3.3M || C10 10pF from V1B output (after C7);
               R11 680k || C11 47pF to ground
V2B: R19 (RP10) / R13 (FINAL) 100k plate; R16 1.5k cathode, unbypassed (15 uF there is "++ MOD")
plate -- R105 47k -- C9 .047 uF -- node: LEAD MASTER 250k rheostat to ground via LDR4A
node -- R102 150k -- send node: R101 4.7k to ground; RETURN normalled; R103 47k at the return
V2A: grid at the return; R104 1k cathode with C16 .47 uF; C15 22 uF via R20 68k (GAIN BOOST
     switch shorts R20; stock position open); R13 (RP10) / R19 (FINAL) 120k plate
plate -- C12 .047 uF -- TO EQ (MASTER 1M rheostat to ground)
```

Omitted, per the drawings' own notes:
- R12 2.2k in the send ("later edition factory mod");
- R106 15k ("added due to dodgy CTS pots in later models");
- the 120 pF on V2A ("removed after August 1984", "don't think present on RP10A");
- the ++ mods.

The previous netlist stopped at `LEAD OUTPUT` and fed the graphic directly. That left out
two triodes, one of which (V2B) clips at a guitar's level.

**Measured (0.122 V at 220 Hz, Volume 1 at 0.8, power stage at rest).**

| Lead Drive | Old preamp | New preamp |
|---|---|---|
| 0.7 | 37.2 % | 58.5 % |
| 1.0 | 38.6 % | 60.4 % |

Calibration figure, with the power stage: **39.1 % → 61.1 %** (the 5150 is 57.2 %).

**Mapping and approximations.**
- The panel's Master knob is now the **Lead Master** (`markiic::LEAD_MASTER`, rest 0.5 —
  a stated playing position).
- The amplifier's MASTER is still represented by the power stage's master pot at rest
  0.30. The graphic between them is linear, so the order changes only loading.
- LDRs are modelled as shorts (lit), and the reverb return loading is omitted (reverb
  off).

**Consequences.**
- The Mark IIC+ blocks of the frozen legacy fixture are no longer compared
  (`tests/legacy_baseline.rs`). The 5150/Twin/73P blocks still are.
- The calibration and power-trim tables were regenerated.
- Presets were re-voiced.
- Tests: `tests/markiic.rs::the_recovery_stages_add_the_saturation_the_lead_channel_is_known_for`
  and `the_lead_master_turns_the_lead_channel_up`.

## Correction 2026-09-15: the graphic equaliser topology

**Finding.** Both drawings show a feedback equaliser:
- Q1 emitter follower (C41 .1 uF, R41 1 M, R42/R43 470 k bias);
- R46 3.32 k into Q2's base, then a Q2/Q3 long-tailed pair on R47 22.1 k, with Q4
  following;
- R48 3.32 k with C47 10 pF from the output back to Q3's base;
- every slider's track between Q2's base (bottom ends) and Q3's base (top ends), wiper to
  R-L-C to the LDR5A ground.

The model had each track from the signal to ground plus a fixed 16.64 dB "driver make-up".
It could cut 12 dB but boost only 1.8-2.7 dB.

**Now.** The Q stages are an ideal op-amp follower and an ideal differential amplifier,
with a rail they never meet (see the note in `markiic::graphic`). Make-up removed.

**Slider value and law.**
- 50 k: Mesa replacement part for Mark I-IV EQ sliders (Tube Amp Doctor Z-MB-S50K).
- Law ESTIMATED: dual slope, span 150. A linear track put only ±1 dB in the middle half
  of the travel.

**Measured.**

| Band | Full cut / full boost | Quarter travel |
|---|---|---|
| 60 Hz | ±13.9 dB | ±6.1 dB |
| 240 Hz | ±10.0 dB | ±5.4 dB |
| 750 Hz | ±15.1 dB | ±5.5 dB |
| 2.2 kHz | ±9.8 dB | ±4.8 dB |
| 6.6 kHz | ±9.8 dB | ±5.1 dB |

Centred, it is flat within 0.2 dB (−0.45 dB insertion). Tests: `tests/graphic.rs`.

## Schematic survey and Bass taper correction (2026-09-15)

**Trigger.** Owner report: the Puppet Master '86 preset has far too much low end. The
owner also asked for a thorough search for correct Mark IIC+ schematics.

**Schematics found.** None is a Mesa factory service drawing; Mesa does not publish them.
1. *MESA/BOOGIE MARK IIC+ Guitar Preamplifier*, the Mesa-labelled drawing with the patent
   notice ([schematicheaven](https://schematicheaven.net/boogieamps/boogie_mkiic_plus.pdf)).
   - The tracer of (2) calls the original Mesa schematic one that "deliberately contained
     errors" (Boogie Amp Forum, "Mark IIC+ Schematic (correct!)").
   - Tube labels are scrambled (V2A/V2B), but the values match (2) and (3).
   - It gives operating voltages: D 392 V, C 397 V, V1A cathode 1.6 V, V1B cathode 1.3 V.
   - The EQ sliders are marked **50K**, which confirms `SLIDER`.
   - No tapers are marked.
2. *Mark IIC+ Schematic FINAL* by jrb32 (2019), traced from an RP10A upgraded by Mike B and
   cross-checked against RP11A/SP11A photos. This is `docs/schematics/Mark IIC+ Schematic
   FINAL.pdf` (byte-identical to the forum's Google Drive copy) and the basis of the model.
3. *Mesa MK2C+ SLOCLONE v2* by Dan_O / The_Haunted, rev 2.0, 15 April 2020, "Verified By
   'Joey' and 'the_haunted' at sloclone forums". Archived at
   `web.archive.org/.../sites.google.com/site/danoampstuff/mesa/Mesa_MK2C+_SLOCLONE_v2.pdf`.
   This is the **only** one that marks pot tapers:
   - Treble 250k B, **Bass 250k A**, Middle 10k B;
   - Volume 1 1M A, Lead Drive 1M A, Lead Master 250k, Master 100K A (R17 22k "to even
     out audio taper on later models").
4. Parts cross-check: Tube Amp Doctor sells Mesa pots, A250K LOG for "MK II/III Bass,
   Treble, Master" and A1M LOG for "MK II/III Volume, Lead Drive"
   ([listing](https://www.tubeampdoctor.com/en/parts-for-amplifiers/passive-components/pots-knobs/potentiometer/mesa-boogie-pots/)).

**Agreement.** (1), (2) and (3) have the same topology and, apart from the items below,
the same values in every stage the model builds: V1A, the stack, Volume 1, V1B, the lead
drive, V3B, V4A with its .22 uF + 6.8 k series leg, the lead output network, the V2B return,
Lead Master, V2A and the graphic.

**Differences recorded, model unchanged.**
- V1B coupling: 0.1 uF + 91 k (2, RP10A) vs .047 uF + 100 k (1, 3). The model follows
  the RP10A.
- V2A cathode: 1 k ("also seen 680 ohm", 2; 1K, 1) vs 1k5 (3). The model uses 1 k.
- GAIN BOOST / PULL DEEP resistor: 68 k ("also seen 15K", 2) vs 15 k (3). The switch is
  out in the model either way.
- V4A R29: 6.8 k ("15K also seen", 2) vs 15 k (3).
- Supply: 410 V assumed in the model against 392/397 V drawn on (1).

**Correction: Bass is an audio taper.**
- `markiic::tap` built the Bass rheostat `ReverseLinear`. It is now `ReverseAudio`, per
  (3) and (4).
- The treble stays linear, following (3), which marks B against the A of the parts
  listing (CONFLICTING; recorded). The middle stays linear (3).
- Small signal at the stack output (`examples/mark_bass.rs`, 82 Hz), in dB of bass
  above the knob at 0:

  | Bass | 0.15 | 0.30 | 0.65 | 1.0 |
  |---|---|---|---|---|
  | linear (before) | +11.5 | +13.9 | +15.1 | +15.4 |
  | audio (now) | +4.6 | +7.4 | +14.3 | +15.4 |

  On the linear track the knob did almost everything in its first 15 %. That made Mesa's
  guidance ("BASS set at 3.0 or well below" with a high Volume 1) meaningless.
- At lead-channel gain the Bass knob is in front of four clipping stages. It moves the
  output's octave balance by well under a decibel; the 80/240 Hz graphic bands set the
  low end, as the manual says. The preset's excess came mainly from its 80 Hz slider at
  0.85. See PRESETS.md.

**Operating point against the Mesa-labelled drawing** (`examples/mark_op.rs`, model supply 410 V):

| Node | Drawing (1) | Model |
|---|---|---|
| Supply D / C | 392 V / 397 V | 410 V |
| V1A cathode | 1.6 V | 1.73 V |
| V1B cathode | 1.3 V | 2.03 V |
| V4A cathode | 2 V | 2.07 V |

- V1A and V4A agree within the supply difference.
- V1B does not. With 100 k and 1.5 k, a 12AX7 at 1.3 V of bias would draw about 2 mA and
  leave its plate near 190 V. It cannot sit at 1.3 V on a 392 V rail with the values that
  every drawing gives, so the figure is not adopted (unexplained; recorded).
