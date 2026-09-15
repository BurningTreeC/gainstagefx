# GainStageFX model inventory

Authoritative checklist of every hardware-inspired (and generic) model found in the
repository, reconciled against the project targets. Built 2026-09-15 by searching
source *contents* of `src/`, `tests/`, `examples/`, `docs/`, root documentation,
`assets/`, `packaging/`, `installer/`, git history (82 commits) and the local,
git-ignored `docs/schematics/` folder. **The repository is the inventory; the target
lists in the task prompts are a minimum, not a boundary.**

Three names are kept separate for every model:

- **Internal ID**: the stable serialized value. For existing host parameters this is the
  nih-plug `#[id]` string; *never renamed*.
- **Hardware inspiration**: the precise engineering reference (manufacturer, model,
  revision) used in research and code documentation.
- **Display name**: the generic GainStageFX product label shown in the UI.

Status vocabulary: `IMPLEMENTED`, `PARTIALLY IMPLEMENTED`, `PLACEHOLDER`, `PLANNED`,
`REFERENCED ONLY`, `GENERIC` (a designed topology, no hardware claim).
Evidence vocabulary (per `MODELS.md`): CIRCUIT DERIVED, PHYSICS DERIVED,
PUBLISHED-PARAMETER DERIVED, EMPIRICALLY TUNED, APPROXIMATED.

## Serialization facts that constrain every change

1. `circuit` is an `EnumParam<Circuit>` with explicit ids
   `clean, crunch, highgain, overdrive, distortion, console, studio, ts808, bigmuff,
   markiic, evh5150, neve, twin`. **Order is also load-bearing**: legacy saved presets
   store *normalized* values (index / (len-1)). Appending a variant changes every
   old normalized value, so new models must **not** be appended to `Circuit` unless a
   versioned migration for legacy normalized values is added first
   (see `tests/modular_state.rs`).
2. `power_amp` (added this refactor) ids: `matched, bypass, power_mark_iic,
   power_twin_ab763, power_5150, power_2203_el34`. Saved presets now also store
   `model_ids`, so this list *can* grow (ids take precedence). Host state stores
   enum ids as strings.
3. `cabinet` ids `off, combo, stack` are the legacy baked cabinet filter. New physical
   speaker/cabinet/microphone selection must be **new parameters** with legacy
   defaults, never new variants of `cabinet`.
4. `diode` (`silicon, germanium, led`), `amplifier` (`valve, jfet, opamp`), `iron`
   (`off, nickel, steel, amorphous`), `tone` (`off, wide, scooping`), `oversampling`
   (`off, x2, x4, x8`) are unchanged.
5. Internal IDs requested by the naming policy (`pedal_ts808`, `amp_mark_iic_plus`,
   ...) are recorded below as **proposed engineering IDs**. They are used for new
   parameters/documents only; the existing serialized ids above stay authoritative.

## 1. Studio preamps / gain stages

| Internal ID | Current display | Hardware inspiration | Implementation | Status | Source status | Power-stage status | Cab/speaker dependency | Presets | Serialization concerns | Proposed display |
|---|---|---|---|---|---|---|---|---|---|---|
| `neve` (proposed `pre_diyre_73p_v11`) | British 73 | DIY Recording Equipment **73P v1.1** 500-series card (Neve-1073-style); *not* a literal 1073, not 73P2 (no 73P2 material in repo) | `src/circuits/neve.rs` (`build`, `output`), `voice.rs` (`line` sim) | IMPLEMENTED (input xfmr, PRE1, PRE2, OUTPUT BC109C/TIP3055/VTB1148) | Project-local DIYRE drawings (5 PNGs, untracked); traced node by node; no online source log in `docs/models` yet | Own line driver, independent of guitar power (kept under power Bypass) | None; presets use Cabinet Off | Neve Preamp, Neve Driven | Display changed (was "Neve 73P"); id unchanged | British 73 |
| — (proposed `pre_api_312`) | — | API **312** discrete op-amp preamp card (revision TBD: 2520 op-amp, input/output transformers) | none | PLANNED | Not researched | n/a | none | none | would need a non-`Circuit` selection or migration | American 312 |
| — (proposed `pre_ua_610`) | — | Universal Audio **610** tube preamp (610-B / 6176 channel, revision TBD) | none | PLANNED | Not researched | n/a | none | none | as above | Tube 610 |
| `console` | Console | GENERIC: step-up input transformer + discrete stage; amplifier row picks valve/JFET/op-amp | `studio.rs` (`CONSOLE`, `VALVE_CHANNEL`), `jfet.rs`, `valve.rs`, `iron.rs` | GENERIC | No hardware claim | none | none | 4 Studio presets | id/order fixed | Console (keep) |
| `studio` | Studio | GENERIC: op-amp gain on studio rails + output iron | `studio.rs` (`STUDIO`) | GENERIC | No hardware claim | none | none | 6 Studio presets | id/order fixed | Studio (keep) |

## 2. Pedals

| Internal ID | Current display | Hardware inspiration | Implementation | Status | Source status | Power-stage status | Cab/speaker dependency | Presets | Serialization concerns | Proposed display |
|---|---|---|---|---|---|---|---|---|---|---|
| `ts808` (proposed `pedal_ts808`) | Green 808 | Ibanez **TS808** Tube Screamer (JRC4558D, 1N4148 pair, BC549 buffers) | `src/circuits/ts808.rs` | IMPLEMENTED (input buffer, clipper, tone, level, output buffer); **values corrected 2026-09-15** to the three-source TS-808 values (owner decision), catalogue voice and pedal slot `pedal_ts808` build the same circuit | [green_808.md](models/green_808.md): Cerutti and ElectroSmash drawings, Keen analysis; manufacturer sheet not obtained | none | Catalogue voice, or in front of any circuit via the pedal slot | Green Overdrive, Screamer Boost, Texas Storm '83 (slot) | Display changed (was "TS808"); stored `ts808` now means the corrected values | Green 808 |
| `pedal_ts9` (pedal slot only) | Green 9 | Ibanez **TS9** Tube Screamer (TS808 with 470 ohm / 100 k output resistors) | `ts808::TS9` via `ts808::build_with` | IMPLEMENTED (op-amp type variation not represented) | [green_9.md](models/green_9.md) | none | pedal slot | none | new `pedal` parameter id | Green 9 |
| — (proposed `pedal_rat`) | — | **ProCo RAT** (revision TBD: original/RAT2; LM308 with 30 pF compensation, 1N914 hard clip, Filter control) | none | PLANNED | Not researched. Solver gap: op-amp has rail clamp only, no GBW/slew, which the RAT's LM308 behavior depends on | none | needs pedal slot | none | new selection | Rodent |
| — (proposed `pedal_fuzz_face`) | — | Dallas Arbiter **Fuzz Face** (germanium PNP AC128/NKT275 vs silicon BC108/BC183 NPN: must pick one and document) | none | PLANNED | Not researched. Solver gap: no PNP device (a positive-ground PNP circuit can be written as its exact negative-ground NPN mirror, which must be documented) | none | needs pedal slot | none | new selection | Round Fuzz |
| `bigmuff` (proposed `pedal_bigmuff_ramshead`) | Big Muff | Electro-Harmonix **Big Muff Pi, 1973 Ram's Head** (4 transistors, 1N914) | `src/circuits/bigmuff.rs` (`RAMS_HEAD` selected); also `pedal_bigmuff_ramshead` in the pedal slot | IMPLEMENTED | Kit Rae's traced archive ([big_muff.md](models/big_muff.md)); value-by-value cross-check not yet done | none | same pedal-slot limitation | Sustain Fuzz | Not in the target list: **keep**. Display name is still a trademark and needs a decision | proposed "Ram Fuzz '73" (needs user approval) |
| (voicing) `bigmuff::TRIANGLE` | — | EHX Big Muff Pi **1971 Triangle** | `bigmuff.rs` const | PARTIALLY IMPLEMENTED (netlist voicing, tested in `tests/bigmuff.rs`, not selectable) | as above | none | — | none | would need selection | TBD |
| (voicing) `bigmuff::SUPA` | — | **Colorsound Supa Tonebender** (Muff topology, first stage without diodes) | `bigmuff.rs` const | PARTIALLY IMPLEMENTED (not selectable) | as above | none | — | none | would need selection | TBD |
| — | — | **Boss HM-2** Heavy Metal | only `docs/schematics/boss-hm2.png` (untracked) | REFERENCED ONLY | Local drawing present; no code, no log | — | — | — | — | not assigned |
| — | — | **Boss MT-2** Metal Zone | only `docs/schematics/boss-mt2.png` (untracked) | REFERENCED ONLY | Local drawing present; no code, no log | — | — | — | — | not assigned |
| `overdrive` | Overdrive | GENERIC op-amp with diodes in feedback (Si/Ge/LED) | `clipper.rs` (`OVERDRIVE`) | GENERIC | no hardware claim (MODELS.md: "not TS9/RAT") | none | none | 4 Overdrive presets | id/order fixed | Overdrive (keep) |
| `distortion` | Distortion | GENERIC diodes to ground (Si/Ge/LED) | `clipper.rs` (`DISTORTION`) | GENERIC | no hardware claim | none | none | 5 Distortion presets | id/order fixed | Distortion (keep) |

## 3. Guitar amplifiers (preamp sections)

| Internal ID | Current display | Hardware inspiration | Implementation | Status | Source status | Power-stage status | Cab/speaker dependency | Presets | Serialization concerns | Proposed display |
|---|---|---|---|---|---|---|---|---|---|---|
| `markiic` (proposed `amp_mark_iic_plus`) | Cali IIC+ | Mesa/Boogie **Mark IIC+** lead channel, stock RP10A C+ **60 W** interpretation | `markiic.rs` (preamp + 5-band graphic), `power.rs::MARKIIC` | IMPLEMENTED preamp; graphic via scalar recovery gain | [docs/models/cali_iic_plus.md](models/cali_iic_plus.md): archival preamp scan, community RP10/FINAL redraws (local copies), discrepancies recorded (recovery/master order, PI values) | Split at `lead_out` -> graphic -> power. Matched = Cali 6L6 | legacy Combo/Stack filter only | Boutique Lead, Boutique Rhythm | display changed | Cali IIC+ |
| `evh5150` (proposed `amp_5150`) | American 5150 | Peavey **EVH 5150** lead channel (code labels match the **5150 II** factory drawing's ULTRA controls; revision unresolved) | `evh5150.rs`, `power.rs::EVH5150` | PARTIALLY IMPLEMENTED: six triodes built; **tone stack not modelled** (plugin tone section substitutes *after* power) | [docs/models/american_5150.md](models/american_5150.md); local `peavey-5150.jpeg` | Split at `To Tone Stack` (33 k load). Matched = American 6L6 High-Gain; no resonance control | legacy filter | Ultra Lead, Ultra Rhythm | display changed | American 5150 |
| `twin` (proposed `amp_twin_reverb_ab763`) | American Twin | Fender **Twin Reverb AB763** (blackface) Vibrato channel | `twin.rs`, `power.rs::TWIN`, `dsp/spring.rs`, `dsp/tremolo.rs` | IMPLEMENTED channel; reverb mix network and 22 k PI tail / 450 V PI rail discrepancies documented; effects applied after power (legacy placement) | [docs/models/american_twin.md](models/american_twin.md): original schematic and layout read | Split at channel output -> synthetic open master -> power. Matched = American 6L6 Clean | legacy filter | Blackface Clean, Blackface Throb | display changed (was "Twin Reverb") | American Twin |
| — (proposed `amp_jcm800_2203`) | — | Marshall **JCM800 2203** (100 W, 1981 export drawings) preamp | none (power only: `power.rs::BRIT_EL34`) | PLANNED (preamp); power IMPLEMENTED | [docs/models/brit_el34.md](models/brit_el34.md) covers PI/power/PSU; preamp drawing located (drtube `jcm800pr.gif`, local `jcm_800_2203.pdf`) but preamp not yet analyzed | Brit EL34 exists and is selectable | legacy filter | none | new selection (not `Circuit`) | Brit 800 |
| — (proposed `amp_dual_rectifier`) | — | Mesa/Boogie **Dual Rectifier** (revision TBD: Rev. C/F/G, 2-channel vs 3-channel) | none | PLANNED | Not researched | needs its own 6L6 + tube-rectifier power | — | none | new selection | Cali Rectifier |
| — (proposed `amp_hiwatt_dr103`) | — | **Hiwatt DR103** Custom 100 (revision TBD) | none | PLANNED | Not researched | needs Brit EL34 Hi-Headroom power | — | none | new selection | Brit DR103 |
| — (proposed `amp_vox_ac30_tb`) | — | **Vox AC30** Top Boost (generation TBD: JMI AC30/6 TB vs later) | none | PLANNED | Not researched; `PentodeSpec::EL84` exists but unused | needs cathode-biased EL84, no-NFB power: `power.rs` builder has fixed-bias only | — | none | new selection | Brit AC30 |
| `clean` / `crunch` / `highgain` | Clean / Crunch / High Gain | GENERIC 1/2/3 cascaded ECC83 stages | `preamp.rs`, `valve.rs` | GENERIC | no hardware claim | none (can use any power override) | legacy filter | Preamp/Crunch/High Gain presets (15) | id/order fixed | keep |

## 4. Power amplifiers

| Internal ID | Display | Hardware inspiration | Implementation | Status | Source status | Load | Presets | Serialization |
|---|---|---|---|---|---|---|---|---|
| `matched` | Matched | resolves to the selected amp's own power stage (None for pedals/studio/generic) | `voice::PowerAmp::resolved` | IMPLEMENTED | n/a | as resolved | all (default) | legacy default; `filter_state` + preset migration insert it |
| `bypass` | Bypass | no power stage (PI, tubes, NFB, OT all skipped; 73P line driver kept) | `voice.rs` | IMPLEMENTED | n/a | n/a | none | appended id |
| `power_mark_iic` | Cali 6L6 | Mark IIC+ 60 W: ECC83 LTP, 2x 6L6GC, NFB/presence | `power.rs::MARKIIC` | IMPLEMENTED (values not fully reconciled with RP10 drawing: see research log) | cali_iic_plus.md | **8 ohm resistor** | Boutique presets (matched) | appended id |
| `power_twin_ab763` | American 6L6 Clean | Twin Reverb AB763: ECC81 LTP, 4x 6L6GC, 820 ohm NFB | `power.rs::TWIN` | IMPLEMENTED (22 k tail / 450 V PI rail discrepancy documented) | american_twin.md | **4 ohm resistor** | Blackface presets | appended id |
| `power_5150` | American 6L6 High-Gain | 5150: ECC83 LTP, 4x 6L6GC, 39 k NFB, presence, no resonance | `power.rs::EVH5150` | IMPLEMENTED (resonance/depth missing) | american_5150.md | **8 ohm resistor** | Ultra presets | appended id |
| `power_2203_el34` | Brit EL34 | JCM800 2203 1981: ECC83 LTP 82k/100k, 4x EL34 -42 V, 100 k NFB from 4 ohm, 22k/.1uF presence, Hammond 1750U data | `power.rs::BRIT_EL34` | IMPLEMENTED (supply impedances/core estimated) | brit_el34.md (1981 originals + 1988 cross-check) | **4 ohm resistor** | none yet | appended id |
| — (proposed `power_dr103_el34`) | Brit EL34 Hi-Headroom | Hiwatt DR103 power section | none | PLANNED | not researched | — | — | new id |
| — (proposed `power_ac30_el84`) | Brit EL84 Class-A | Vox AC30 cathode-biased 4x EL84, no NFB | none | PLANNED (needs cathode-bias builder support) | not researched | — | — | new id |
| — (proposed `power_rectifier_6l6`) | (to name) | Dual Rectifier power, tube rectifier sag | none | PLANNED | not researched | — | — | new id |

## 5. Speakers, cabinets, microphones

Physical speakers, cabinets and microphones are **IMPLEMENTED** (2026-09-15) as new
parameters `cab_model`, `speaker`, `mic_a`, `mic_b` (+ placement), whose first entries
keep the legacy behaviour. With a physical cabinet the power stage drives a reactive
Thiele-Small speaker load inside its own netlist (`acoustics::speaker`,
`power::build_with_speaker`); otherwise it still terminates in a resistor and the
legacy baked filter below applies. See SPEAKER_MODEL.md, CABINET_MODEL.md,
MICROPHONE_MODEL.md.

| Internal ID | Display | What it is | Implementation | Status | Evidence | Presets | Serialization |
|---|---|---|---|---|---|---|---|
| `cabinet=combo` | Combo | GENERIC loaded RLC: 110 Hz/Q .9 HP, 8.5 kHz/Q .72 LP, 14 kHz pole; peak-normalized | `circuits/cabinet.rs::COMBO` | IMPLEMENTED (legacy) | EMPIRICALLY TUNED response, no geometry, no mic | 8 presets | must remain for old sessions |
| `cabinet=stack` | Stack | GENERIC: 85 Hz/Q 1.35, 7 kHz/Q .8, 11 kHz pole | `cabinet.rs::STACK` | IMPLEMENTED (legacy) | EMPIRICALLY TUNED | 15 presets | must remain |

Speakers (research: `docs/models/speakers.md`, fits in `tools/speaker_fit/`):

| Proposed ID | Display | Hardware inspiration | Data found | Status |
|---|---|---|---|---|
| `spk_celestion_v30` | Brit V30 | Celestion Vintage 30, 8 ohm | Mfr: Fs 75, Re 7.3, 100 dB, 44 mm VC, 1.42 kg ceramic, response plot. **No Qms/Qes/Bl/Mms/Le published** | IMPLEMENTED (remaining T/S ESTIMATED from priors) |
| `spk_celestion_g12m25` | Brit Green 25 | Celestion G12M-25 Greenback, 8 ohm | Mfr: Fs 75, Re 6.7, 98 dB, response plot; same gaps | IMPLEMENTED |
| `spk_celestion_g12t75` | Brit T75 | Celestion G12T-75, 8 ohm | Mfr: Fs 85, Re 6.77, 97 dB, response plot; same gaps | IMPLEMENTED |
| `spk_jensen_p12r` | American Vintage 12 | Jensen P12R (reissue) 8 ohm | Mfr full T/S + numeric SPL and impedance curves | IMPLEMENTED |
| `spk_jensen_p10r` | American Vintage 10 | Jensen P10R 8 ohm | Mfr full T/S + curves (impedance peak conflicts with T/S: 82 vs 61 ohm) | IMPLEMENTED |
| `spk_jensen_c12n` | American Ceramic | Jensen C12N 8 ohm | Mfr full T/S + curves | IMPLEMENTED |
| `spk_jensen_p12n` | American Alnico | Jensen P12N 8 ohm | Mfr full T/S + curves | IMPLEMENTED |

Cabinets (values marked DOCUMENTED / DERIVED / ESTIMATED / TUNED in docs/models/cabinets.md):

| Proposed ID | Display | Inspiration | Documented so far | Status |
|---|---|---|---|---|
| `cab_marshall_1960a` | Brit 1960 4x12 | Marshall 1960A (angled), G12T-75 factory | 770 W x 755 H x 365 D mm (marshall.com); 16/4 ohm mono | IMPLEMENTED |
| `cab_marshall_1960b` | Brit Closed 4x12 | Marshall 1960B (straight), G12T-75 | same outer dims (marshall.com) | IMPLEMENTED |
| `cab_marshall_1960ax` | Brit Green 4x12 | Marshall 1960AX, G12M-25 16 ohm | 770 x 755 x 365 mm | IMPLEMENTED |
| `cab_marshall_1960av` | Brit V30 4x12 | Marshall 1960AV (V30-loaded 1960) | geometry as 1960A; model page not yet fetched | IMPLEMENTED |
| `cab_mesa_recto_standard` | Cali Oversized 4x12 | Mesa/Boogie Rectifier Standard 4x12, V30 | 32.9 H x 30.1 W x 14.25 D in (retail/Mesa text); ply thickness unverified | IMPLEMENTED |
| `cab_generic_oversized_412` | Oversized 4x12 | GENERIC oversized closed 4x12 | no hardware claim; geometry ESTIMATED | IMPLEMENTED |
| `cab_fender_twin_open_212` | American Open 2x12 | Fender Twin Reverb combo (AB763-style) open back | 20 H x 26-1/8 W x 10-1/2 D in (reproduction cabinet maker); openness ESTIMATED | IMPLEMENTED |
| `cab_fender_deluxe_open_112` | American Open 1x12 | Fender '65 Deluxe Reverb open back | 17.5 H x 24.5 W x 9.5 D in (retail spec) | IMPLEMENTED |
| `cab_marshall_1912` | Closed 1x12 | Marshall 1912 closed 1x12 | ~500 W x 470 H x 290 D mm (retail) | IMPLEMENTED |
| `cab_marshall_1936` | Closed 2x12 | Marshall 1936 closed 2x12 | 750 W x 600 H x 310 D mm (retail) | IMPLEMENTED |

Microphones (research: `docs/models/microphones.md`):

| Proposed ID | Display | Inspiration | Primary data found | Status |
|---|---|---|---|---|
| `mic_shure_sm57` | Dynamic 57 | Shure SM57 | Shure user guide v3.6: response chart, polar 125 Hz-8 kHz, -56 dBV/Pa, proximity statement | IMPLEMENTED |
| `mic_sennheiser_md421` | Dynamic 421 | Sennheiser MD 421 II | product sheet: 30-17k, cardioid, 2 mV/Pa, small response/polar plot | IMPLEMENTED |
| `mic_sennheiser_e906` | Dynamic 906 | Sennheiser e 906 | 40-18k, supercardioid, 2.2 mV/Pa, presence 4.2 kHz; response shape **secondary source** (manual PDF blocked) | IMPLEMENTED |
| `mic_sennheiser_md409` | Dynamic 409 | Sennheiser MD 409 U3 | 1986 data sheet: 50-15k, 1.18 mV/Pa, 5 cm and 100 cm curves; datasheet says *cardioid*, later marketing says supercardioid (conflict recorded) | IMPLEMENTED |
| `mic_royer_r121` | Ribbon 121 | Royer R-121 | cut sheet: 30-15k +-3 dB, fig-8, -50 dBV/Pa, response + polar | IMPLEMENTED |
| `mic_beyer_m160` | Ribbon 160 | beyerdynamic M 160 | data sheet: 40-18k, hypercardioid (>25 dB at 110 deg), 1.0 mV/Pa, 10 cm and 1 m curves | IMPLEMENTED |
| `mic_coles_4038` | Ribbon 38 | Coles 4038 | data sheet: 30-15k flat, fig-8, -65 dB re 1V/Pa, response chart | IMPLEMENTED |
| `mic_neumann_u87ai` | Condenser 87 | Neumann U 87 Ai (cardioid) | operating manual: 20-20k, 20/28/22 mV/Pa, curves and polars for 3 patterns | IMPLEMENTED |
| `mic_akg_c414` | Condenser 414 | AKG C414 (XLS vs XLII to be chosen) | range/sensitivity only; chart not retrieved | IMPLEMENTED (APPROXIMATED: no chart found) |
| `mic_neumann_u67` | Tube Condenser 67 | Neumann U 67 (reissue = 1960-71 design) | range, 15/24/16 mV/Pa; chart not retrieved | IMPLEMENTED (APPROXIMATED: no chart found) |
| `mic_neumann_u47fet` | FET Condenser 47 | Neumann U 47 fet | 40-16k, 8 mV/Pa; chart not retrieved | IMPLEMENTED (APPROXIMATED: no chart found) |

## 6. Transformers, tone stacks, effects and device fits (building blocks)

| Item | Inspiration | Where | Status / evidence |
|---|---|---|---|
| Iron: Nickel / Steel / Amorphous | GENERIC core materials | `iron.rs`, `netlist::CoreSpec` | IMPLEMENTED; PHYSICS DERIVED + EMPIRICALLY TUNED |
| 73P input transformer, VTB1148 output | DIYRE 73P / Carnhill VTB1148 | `neve.rs` | IMPLEMENTED; values from drawings |
| Mark IIC+ OT, Twin 125A29A, 5150 70500207 | as named | `power.rs` specs | IMPLEMENTED; ratios/inductances largely ESTIMATED (see research logs) |
| 2203 OT | Hammond 1750U replacement data (not original Dagnall) | `power.rs::BRIT_EL34` | PUBLISHED-PARAMETER DERIVED (replacement part) |
| Tone: Wide / Scooping | GENERIC designed passive network, *explicitly not* a Fender/Marshall stack | `tone.rs` | GENERIC |
| Mark IIC+ Fender-style stack + 5-band graphic | Mark IIC+ drawing | `markiic.rs` | IMPLEMENTED |
| Twin AB763 stack, bright cap | AB763 | `twin.rs` | IMPLEMENTED |
| 5150 tone stack | 5150 | — | **MISSING** (illegible on local drawing; plugin tone section substitutes post-power) |
| Spring reverb | Accutronics 4AB3C1B-style long tank | `dsp/spring.rs` | IMPLEMENTED; dispersive allpass approximation |
| Optical tremolo | AB763 neon/LDR vibrato | `dsp/tremolo.rs` | IMPLEMENTED; behavioral LDR model |
| Tubes | ECC83, ECC82, ECC81, 6L6GC, EL34, **EL84 (unused)** | `netlist.rs` | Koren-style fits |
| Semiconductors | BC549, BC109C, TIP3055, FS36999, 2N5457, J201, J113, 2SC3378, 1N914/1N4148, Si/Ge/LED | various | Ebers-Moll NPN only (**no PNP**), Shockley diodes, square-law JFET |
| Op-amp | JRC4558 (TS808) and generic | `netlist` `OpAmp` | ideal with rail limit; **no GBW/slew** |

## 7. Gaps between repository and targets

- **Implemented and already renamed:** Green 808, Cali IIC+, American 5150,
  American Twin, British 73. Also the power stages Cali 6L6, American 6L6 Clean,
  American 6L6 High-Gain and Brit EL34.
- **Existing models that are not on the target list and must be preserved:**
  Big Muff Ram's Head (selectable), the Triangle and Supa Tonebender voicings
  (not selectable), the seven generic topologies, the Iron materials, the Twin
  spring and tremolo, and the legacy Combo/Stack cabinets.
- **Referenced only:** Boss HM-2 and MT-2 (local schematics, no code).
- **Missing preamps and pedals:** American 312, Tube 610, Rodent, Round Fuzz,
  Brit 800 preamp (research checkpoint in `docs/models/brit_800.md`), Cali Rectifier,
  Brit DR103, Brit AC30.
- **Missing power stages:** EL34 Hi-Headroom, EL84 Class-A, Rectifier power.
- **Chain stages:** pedal slot, speaker load, cabinets and microphones are implemented.
- **Research logs still owed for implemented models:** Big Muff (value-by-value
  cross-check), 73P (online DIYRE source record).
- **Solver capability gaps:** PNP devices (Fuzz Face), op-amp GBW/slew (RAT LM308),
  cathode-biased no-NFB power stage (AC30), tube rectifier sag (Rectifier/AC30),
  control-rate adjustable R/L/C was added 2026-09-15 for speaker loads (`tests/speaker_load.rs`).
- **Naming decision needed from the user:** a generic display name for Big Muff.

## 8. Prioritized research / implementation queue

Each item runs research, documentation, baseline, implementation, tests, benchmark
and documented results before the next one starts. Hardware circuits require the
schematic-first checkpoint in `docs/models/<model>.md` *before* code.

1. DONE: **Speaker electrical and mechanical model + reactive load coupling** (research done:
   `docs/models/speakers.md`). Validate the adjustable-part solver extension,
   stamp the load into each power netlist, keep the legacy resistive path bit-identical.
2. DONE: **Physical cabinets** (finish geometry research: 1960AV page, Mesa ply, Twin/Deluxe
   baffle layout), then sealed/open/array processing.
3. DONE: **Microphones + placement + dual mic** (finish C414/U67/U47 fet chart search).
4. DONE: **Parameters, GUI, legacy-default migration** for speaker/cabinet/mic; level policy
   for power overrides (make-up for non-matched combinations).
5. DONE: **Independent pedal slot** reusing the existing TS808 and Big Muff netlists; write
   the TS808 and Big Muff research logs (schematic re-verification) before routing.
6. DONE: **Green 9 (TS9)**: schematic research on TS808 vs TS9 differences; implement as a
   revision of the TS808 netlist only where the drawings agree.
7. **Brit 800 preamp (2203)**: analyze the located 1981 preamp drawing; Matched = Brit EL34.
8. **Rodent (ProCo RAT)**: revision research plus an op-amp GBW/slew device extension.
9. **Round Fuzz (Fuzz Face)**: choose germanium or silicon revision; PNP mirror documented.
10. **Brit AC30 + EL84 Class-A** (cathode-bias/no-NFB and rectifier support in `power.rs`).
11. **Brit DR103 + EL34 Hi-Headroom.**
12. **Cali Rectifier + its power stage** (exact revision).
13. **American 312, Tube 610.**
14. **Versioned corrections** of inherited approximations: the 5150 tone stack,
    Twin PI tail/rail, Mark IIC+ recovery/master order and PI values. Each needs its
    own regression checkpoint and must never silently alter Matched legacy output.
15. **Factory era presets** once their components exist, with rig research logs that
    mark each claim DOCUMENTED / WIDELY REPORTED / PLAUSIBLE / APPROXIMATED.
