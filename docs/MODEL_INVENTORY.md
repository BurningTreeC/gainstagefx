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
   markiic, evh5150, neve, twin, amp_jcm800_2203, pre_api_312, pre_ssl_4000e, pre_ua_610a`. **Order is also load-bearing**:
   legacy saved presets store *normalized* values (index / (len-1)).
   - `amp_jcm800_2203` was appended on 2026-09-15 together with that migration:
     `presets::migrate` re-expresses a saved preset without stable ids against
     `params::LEGACY_CIRCUIT_COUNT = 13`. Tested in `tests/modular_state.rs`.
   - Host state stores enum ids as strings, so sessions are unaffected.
   - Host automation lanes store normalized values; a lane recorded on the circuit
     parameter before the append will map differently. This is unavoidable when an enum
     grows.
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
| `neve` (proposed `pre_diyre_73p_v11`) | British 73 | DIY Recording Equipment **73P v1.1** 500-series card (Neve-1073-style); *not* a literal 1073, not 73P2 (no 73P2 material in repo) | `src/circuits/neve.rs` (`build`, `output`), `voice.rs` (`line` sim) | IMPLEMENTED (input xfmr, PRE1, PRE2, OUTPUT BC109C/TIP3055/VTB1148) | Project-local DIYRE drawings (5 PNGs, untracked); traced node by node; no online source log in `docs/models` yet | Own line driver, independent of guitar power (kept under power Bypass) | None; presets use Cabinet Off | British 73 Pre, British 73, Driven | Display changed (was "Neve 73P"); id unchanged | British 73 |
| `pre_api_312` | American 312 | API **312** mic preamp card (2622 input, 2520 op-amp, 2503 output) | `american312.rs` | IMPLEMENTED 2026-09-15 (card values; 2520 as ideal op-amp; transformer internals ESTIMATED to API data) | [american_312.md](models/american_312.md): API card drawing, transformer and 2520 data sheets | none | none | American Console Pre, American Console, Pushed | appended to `circuit` | American 312 |
| `pre_ua_610a` | Tube 610 | Universal Audio **610-A** modular console preamp (UA drawing C-10068, Hinson redraw) | `tube610.rs` | IMPLEMENTED 2026-09-15 (two feedback pairs, EQ at 0, Hi gain; B+ and transformer internals ESTIMATED) | [tube_610.md](models/tube_610.md): Hinson redraw via Internet Archive, UTC catalogue and terminal table, RCA 12AY7 | own output transformer | none | Valve Console Pre, Valve Console, Pushed | appended to `circuit` | Tube 610 |
| `pre_ssl_4000e` | British 4K E | Solid State Logic **SL 4000 E** channel amp 82E01 mic amplifier (Jensen JE-115K-E) | `console_e.rs` | IMPLEMENTED 2026-09-15 (two 5534s around the gain pot; Jensen core ESTIMATED to data sheet) | [british_4k_e.md](models/british_4k_e.md): SSL drawings T82001-71 rev 16 and 82E01-710-8208-12, Jensen data sheets | none | none | British Desk Pre, British Desk, Driven | appended to `circuit` | British 4K E |
| `console` | Console | GENERIC: step-up input transformer + discrete stage; amplifier row picks valve/JFET/op-amp | `studio.rs` (`CONSOLE`, `VALVE_CHANNEL`), `jfet.rs`, `valve.rs`, `iron.rs` | GENERIC | No hardware claim | none | none | 4 Studio presets | id/order fixed | Console (keep) |
| `studio` | Studio | GENERIC: op-amp gain on studio rails + output iron | `studio.rs` (`STUDIO`) | GENERIC | No hardware claim | none | none | 6 Studio presets | id/order fixed | Studio (keep) |

## 2. Pedals

| Internal ID | Current display | Hardware inspiration | Implementation | Status | Source status | Power-stage status | Cab/speaker dependency | Presets | Serialization concerns | Proposed display |
|---|---|---|---|---|---|---|---|---|---|---|
| `ts808` (proposed `pedal_ts808`) | Green 808 | Ibanez **TS808** Tube Screamer (JRC4558D, 1N4148 pair, BC549 buffers) | `src/circuits/ts808.rs` | IMPLEMENTED (input buffer, clipper, tone, level, output buffer); **values corrected 2026-09-15** to the three-source TS-808 values (owner decision), catalogue voice and pedal slot `pedal_ts808` build the same circuit | [green_808.md](models/green_808.md): Cerutti and ElectroSmash drawings, Keen analysis; manufacturer sheet not obtained | none | Catalogue voice, or in front of any circuit via the pedal slot | Green Overdrive, Screamer Boost, Texas Storm '83 (slot) | Display changed (was "TS808"); stored `ts808` now means the corrected values | Green 808 |
| `pedal_ts9` (pedal slot only) | Green 9 | Ibanez **TS9** Tube Screamer (TS808 with 470 ohm / 100 k output resistors) | `ts808::TS9` via `ts808::build_with` | IMPLEMENTED (op-amp type variation not represented) | [green_9.md](models/green_9.md) | none | pedal slot | none | new `pedal` parameter id | Green 9 |
| `pedal_rat` (pedal slot) | Rodent | **Pro Co RAT**, LM308 with 30 pF compensation, 1N914 to ground, 2N5458 follower | `rodent.rs` | IMPLEMENTED 2026-09-15 | ElectroSmash analysis + LM308 curves; op-amp as transconductor-integrator (GBW 1.1 MHz, slew 0.3 V/us). Log: `docs/models/rodent.md` | none | pedal slot | none | `pedal` id | Rodent |
| `pedal_mxr_dist_plus` (pedal slot) | Yellow Dist | **MXR Distortion+**, 741 with germanium diodes to ground | `distortion_plus.rs` | IMPLEMENTED 2026-09-16 | ElectroSmash's analysis, with its own arithmetic as the check (3.5-46.5 dB, 350 mV of clipping). Log: `docs/models/yellow_dist.md` | none | pedal slot | Blizzard '80 | `pedal` id | Yellow Dist |
| `pedal_fuzz_face` (pedal slot) | Round Fuzz | Arbiter **Fuzz Face**, germanium PNP (1966-68), AC128 models | `round_fuzz.rs` | IMPLEMENTED 2026-09-15 | ElectroSmash analysis + fuzzboxes.org survey (C1 2.5 uF / C2 20 uF conflict recorded); native PNP device. Log: `docs/models/round_fuzz.md` | none | pedal slot | none | `pedal` id | Round Fuzz |
| `bigmuff` (proposed `pedal_bigmuff_ramshead`) | Ram Fuzz | Electro-Harmonix **Big Muff Pi, 1973 Ram's Head** (4 transistors, 1N914) | `src/circuits/bigmuff.rs` (`RAMS_HEAD` selected); also `pedal_bigmuff_ramshead` in the pedal slot | IMPLEMENTED | Kit Rae's traced archive ([big_muff.md](models/big_muff.md)); value-by-value cross-check not yet done | none | same pedal-slot limitation | Sustain Fuzz | Not in the target list: **keep**. Display renamed to "Ram Fuzz" (owner request, 2026-09-15); ids unchanged | Ram Fuzz |
| (voicing) `bigmuff::TRIANGLE` | — | EHX Big Muff Pi **1971 Triangle** | `bigmuff.rs` const | PARTIALLY IMPLEMENTED (netlist voicing, tested in `tests/bigmuff.rs`, not selectable) | as above | none | — | none | would need selection | TBD |
| (voicing) `bigmuff::SUPA` | — | **Colorsound Supa Tonebender** (Muff topology, first stage without diodes) | `bigmuff.rs` const | PARTIALLY IMPLEMENTED (not selectable) | as above | none | — | none | would need selection | TBD |
| — | — | **Boss HM-2** Heavy Metal | only `docs/schematics/boss-hm2.png` (untracked) | REFERENCED ONLY | Local drawing present; no code, no log | — | — | — | — | not assigned |
| — | — | **Boss MT-2** Metal Zone | only `docs/schematics/boss-mt2.png` (untracked) | REFERENCED ONLY | Local drawing present; no code, no log | — | — | — | — | not assigned |
| `overdrive` | Overdrive | GENERIC op-amp with diodes in feedback (Si/Ge/LED) | `clipper.rs` (`OVERDRIVE`) | GENERIC | no hardware claim (MODELS.md: "not TS9/RAT") | none | none | 4 Overdrive presets | id/order fixed | Overdrive (keep) |
| `distortion` | Distortion | GENERIC diodes to ground (Si/Ge/LED) | `clipper.rs` (`DISTORTION`) | GENERIC | no hardware claim | none | none | 5 Distortion presets | id/order fixed | Distortion (keep) |

## 3. Guitar amplifiers (preamp sections)

| Internal ID | Current display | Hardware inspiration | Implementation | Status | Source status | Power-stage status | Cab/speaker dependency | Presets | Serialization concerns | Proposed display |
|---|---|---|---|---|---|---|---|---|---|---|
| `markiic` (proposed `amp_mark_iic_plus`) | Cali IIC+ | Mesa/Boogie **Mark IIC+** lead channel, stock RP10A C+ **60 W** interpretation | `markiic.rs` (preamp + 5-band graphic), `power.rs::MARKIIC` | IMPLEMENTED preamp **including lead return, V2B, Lead Master and V2A (corrected 2026-09-15)**; graphic via scalar recovery gain | [docs/models/cali_iic_plus.md](models/cali_iic_plus.md): archival preamp scan, community RP10/FINAL redraws (local copies); PI values discrepancy still recorded | Split at `TO EQ` -> graphic -> power (master pot stands in for MASTER). Matched = Cali 6L6. Panel Master = Lead Master | physical cabinets | Boutique Lead, Boutique Rhythm, Puppet Master '86 | display changed; legacy fixture blocks for this voice no longer compared | Cali IIC+ |
| `evh5150` (proposed `amp_5150`) | American 5150 | Peavey **EVH 5150** lead channel (code labels match the **5150 II** factory drawing's ULTRA controls; revision unresolved) | `evh5150.rs`, `power.rs::EVH5150` | PARTIALLY IMPLEMENTED: six triodes built; **tone stack not modelled** (plugin tone section substitutes *after* power) | [docs/models/american_5150.md](models/american_5150.md); local `peavey-5150.jpeg` | Split at `To Tone Stack` (33 k load). Matched = American 6L6 High-Gain; no resonance control | legacy filter | Ultra Lead, Ultra Rhythm | display changed | American 5150 |
| `twin` (proposed `amp_twin_reverb_ab763`) | American Twin | Fender **Twin Reverb AB763** (blackface) Vibrato channel | `twin.rs`, `power.rs::TWIN`, `dsp/spring.rs`, `dsp/tremolo.rs` | IMPLEMENTED channel; reverb mix network and 22 k PI tail / 450 V PI rail discrepancies documented; effects applied after power (legacy placement) | [docs/models/american_twin.md](models/american_twin.md): original schematic and layout read | Split at channel output -> synthetic open master -> power. Matched = American 6L6 Clean | legacy filter | Blackface Clean, Blackface Throb | display changed (was "Twin Reverb") | American Twin |
| `amp_jcm800_2203` | Brit 800 | Marshall **JCM800 2203** (100 W) master-volume preamp, 1981 drawing | `brit800.rs`, `power.rs::BRIT_EL34` | IMPLEMENTED 2026-09-15 (four triodes, cathode follower, stack, droppers) | [docs/models/brit_800.md](models/brit_800.md): 1981 Marshall drawing with voltage table, 1988 Marshall drawing cross-check, local redraw; middle 22k / treble 220k / bass log resolved from 1988 | Split at the treble wiper into the master pot. Matched = Brit EL34 | physical cabinets | Brit Crunch, Brit Lead, Screamer Boost | appended to `circuit`; saved presets without ids migrated (`LEGACY_CIRCUIT_COUNT`) | Brit 800 |
| `amp_dual_rectifier` | Cali Rectifier | Mesa/Boogie **Dual Rectifier**, two-channel Rev F (RF-1F board), red channel modern | `rectifier.rs`, `power.rs::RECTO_6L6` | IMPLEMENTED 2026-09-16 (five triodes incl. the 39 k cold stage, follower, stack, master) | [docs/models/cali_rectifier.md](models/cali_rectifier.md): Mesa's own RF-1F preamp and power-amp sheets, with the Rectifier Guide for revisions | Split at the red master. Matched = Recto 6L6. **The valve rectifier is not modelled**: the supply is its silicon setting | physical cabinets | Recto Rhythm, Recto Lead | appended to `circuit` | Cali Rectifier |
| `amp_marshall_1959` | Brit Plexi | Marshall **1959 Super Lead** (100 W, EL34), bright channel, Unicord drawing 70-6-11 July 1970 | `plexi.rs`, `power.rs::PLEXI_EL34` | IMPLEMENTED 2026-09-15 (three triodes, cathode follower, stack, 20k/10k/10k droppers) | [docs/models/brit_plexi.md](models/brit_plexi.md): Unicord 1970, Marshall c.1967 (voltages), Marshall 1988 1959 STD (pot laws, later changes), Robinette cross-check | Split at the treble wiper into the inverter's coupling cap (no master). Matched = Brit Plexi EL34 | physical cabinets | Plexi Crunch, Plexi Cranked, Blackout '80, Experienced '67 | appended to `circuit` | Brit Plexi |
| `amp_hiwatt_dr103` | Brit DR103 | **Hiwatt DR103** Custom 100, brilliant channel, Issue 4 factory sheets | `dr103.rs`, `power.rs::DR103_EL34` | IMPLEMENTED 2026-09-16 (five triodes, master volume, direct-coupled inverter) | [docs/models/brit_dr103.md](models/brit_dr103.md): Hiwatt's own 1994-95 sheets, Circuit Codex redraw of a late-60s amp, Ampbooks' inverter analysis | Split at the driver's cathode, with its direct voltage carried across. Matched = DR103 EL34 | physical cabinets | Hi-Headroom Clean/Pushed, The Great Wall '79 | appended to `circuit` | Brit DR103 |
| `amp_vox_ac30_tb` | Brit AC30 | **Vox AC30/6 Top Boost**, brilliant channel | `ac30.rs`, `power.rs::AC30_EL84` | IMPLEMENTED 2026-09-16 (three triodes, two-knob stack, cathode-biased EL84s, no loop) | [docs/models/brit_ac30.md](models/brit_ac30.md): Dallas 1974 and VSL 1971 factory sheets, voxac30.org.uk archive, Ampbooks | Split at the treble wiper into the 220 k mixers. Matched = AC30 EL84 | physical cabinets | Chime Clean, Chime Edge | appended to `circuit` | Brit AC30 |
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
| `power_2203_el34` | Brit EL34 | JCM800 2203 1981: ECC83 LTP 82k/100k, 4x EL34 -42 V, 100 k NFB from 4 ohm, 22k/.1uF presence, Hammond 1750U data | `power.rs::BRIT_EL34` | IMPLEMENTED (supply impedances/core estimated) | brit_el34.md (1981 originals + 1988 cross-check) | 4 ohm resistor, or a speaker load on the physical path | Brit Crunch, Brit Lead, Screamer Boost (matched to Brit 800) | appended id |
| `power_1959_el34` | Brit Plexi EL34 | 1959 Super Lead 1970: 2203-family inverter and iron, no master, 47 k NFB from 16 ohm (23.5 k at 4), 5 k presence, -37 V (estimated) | `power.rs::PLEXI_EL34` | IMPLEMENTED 2026-09-15 (supply voltages from the 1967 drawing, bias and iron estimated) | brit_plexi.md | 4 ohm resistor or speaker load | matched to Brit Plexi | appended id |
| `power_dr103_el34` | DR103 EL34 | Hiwatt DR103: 82k/91k inverter direct-coupled to the driver, 22k/2k2 tail, 22 k stoppers, 4x EL34 -38 V, 10 k feedback from 16 ohm | `power.rs::DR103_EL34` | IMPLEMENTED 2026-09-16 (supplies and iron estimated; presence omitted) | brit_dr103.md | 8 ohm resistor or a speaker load | matched to Brit DR103 | appended id |
| `power_ac30_el84` | AC30 EL84 | AC30 Top Boost: cathode-biased 4x EL84 on 50 ohm/250 uF, no feedback, 250 k cut control, 4 k a-a | `power.rs::AC30_EL84` | IMPLEMENTED 2026-09-16 (supplies and iron estimated; valve rectifier as a resistance) | brit_ac30.md | 8 ohm resistor or a speaker load | matched to Brit AC30 | appended id |
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
| Semiconductors | BC549, BC109C, TIP3055, FS36999, 2N5457, J201, J113, 2SC3378, 1N914/1N4148, Si/Ge/LED | various | Ebers-Moll NPN and PNP (PNP added 2026-09-15), Shockley diodes, square-law JFET, AC128 |
| Op-amp | JRC4558 (TS808) and generic; LM308 (Rodent) | `netlist` `OpAmp`, `Transconductor`; `rodent::lm308` | ideal with rail limit; LM308 built from a saturating transconductor into an integrator for GBW and slew |

## 7. Gaps between repository and targets

- **Implemented and already renamed:** Green 808, Cali IIC+, American 5150,
  American Twin, British 73. Also the power stages Cali 6L6, American 6L6 Clean,
  American 6L6 High-Gain and Brit EL34.
- **Existing models that are not on the target list and must be preserved:**
  Big Muff Ram's Head (selectable), the Triangle and Supa Tonebender voicings
  (not selectable), the seven generic topologies, the Iron materials, the Twin
  spring and tremolo, and the legacy Combo/Stack cabinets.
- **Referenced only:** Boss HM-2 and MT-2 (local schematics, no code).
- **Missing preamps:** none. Every model on the target list is implemented.
- **Missing power stages:** none.
- **Chain stages:** pedal slot, speaker load, cabinets and microphones are implemented.
- **Research logs still owed for implemented models:** Big Muff (value-by-value
  cross-check), 73P (online DIYRE source record).
- **Solver capability gaps:** none of the ones this document listed remain.
  - control-rate adjustable R/L/C, 2026-09-15, for speaker loads (`tests/speaker_load.rs`);
  - PNP bipolars and op-amp gain-bandwidth/slew, 2026-09-15, for the Round Fuzz and Rodent;
  - cathode bias, no feedback loop and a cut control, 2026-09-16, for the AC30;
  - a direct-coupled inverter and an input carrying a direct voltage, 2026-09-16, for the
    DR103;
  - **a rectifier valve** (`Part::Rectifier`, Child's law), 2026-09-16, for the Rectifier's
    valve setting and the AC30's GZ34 (`tests/devices.rs`);
  - **the mains** (`Simulation::set_supply_scale` and the `mains` parameter), 2026-09-16,
    which is a variac and what the Brown presets are wired to (`tests/mains.rs`).
- **Naming:** Big Muff is displayed as "Ram Fuzz" (2026-09-15).

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
7. DONE: **Brit 800 preamp (2203)**: analyze the located 1981 preamp drawing; Matched = Brit EL34.
8. DONE: **Rodent (ProCo RAT)**: LM308 revision; `Part::Transconductor` for GBW/slew.
9. DONE: **Round Fuzz (Fuzz Face)**: germanium revision; native PNP device.
10. DONE: **Brit AC30 + EL84 Class-A**: cathode bias, no feedback loop and a cut control
    added to `power.rs`; the valve rectifier is a series resistance.
11. DONE: **Brit DR103**: direct-coupled inverter, which needed a biased input in the
    netlist so one block can hand the next its direct voltage.
12. DONE: **Cali Rectifier + its power stage** (two-channel Rev F, from Mesa's own
    sheets; the switchable valve rectifier is not modelled).
13. DONE: **American 312, Tube 610** (plus British 4K E).
14. **Versioned corrections** of inherited approximations: the 5150 tone stack,
    Twin PI tail/rail, Mark IIC+ recovery/master order and PI values. Each needs its
    own regression checkpoint and must never silently alter Matched legacy output.
15. **Factory era presets** once their components exist, with rig research logs that
    mark each claim DOCUMENTED / WIDELY REPORTED / PLAUSIBLE / APPROXIMATED.
