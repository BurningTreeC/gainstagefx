# GainStageFX model inventory

Authoritative checklist of every hardware-inspired (and generic) model found in the
repository, reconciled against the project targets. Originally built 2026-09-15 by searching
source *contents* of `src/`, `tests/`, `examples/`, `docs/`, root documentation,
`assets/`, `packaging/`, `installer/`, git history (82 commits) and the local,
git-ignored `docs/schematics/` folder. **The repository is the inventory; the target
lists in the task prompts are a minimum, not a boundary.**

**Current reconciliation: 2026-09-20.** Section 9 records the current implementation,
validation and performance gaps; older implementation dates identify historical work,
not the date of a complete hardware or realtime qualification.

The optional Noise Reduction input expander is shared digital processing, not a
new hardware model. It is disabled by default and does not replace or simplify
any circuit. Its controls and validation are documented in README.md and DSP.md.

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
   markiic, evh5150, neve, twin, amp_jcm800_2203, pre_api_312, pre_ssl_4000e, pre_ua_610a, amp_marshall_1959, amp_vox_ac30_tb,
   amp_hiwatt_dr103, amp_dual_rectifier, pedal_ts9_circuit, pedal_rat_circuit,
   pedal_fuzz_face_circuit, pedal_mxr_dist_plus_circuit, pedal_boss_hm2_circuit,
   pedal_boss_mt2_circuit, amp_fender_deluxe_ab763, amp_roland_jc120,
   amp_fender_deluxe_ab763_normal, amp_jcm800_2205`. **Order is also load-bearing**:
   legacy saved presets store *normalized* values (index / (len-1)).
   - `amp_jcm800_2203` was appended on 2026-09-15 together with that migration:
     `presets::migrate` re-expresses a saved preset without stable ids against
     `params::LEGACY_CIRCUIT_COUNT = 13`. Tested in `tests/modular_state.rs`.
   - Host state stores enum ids as strings, so sessions are unaffected.
   - Host automation lanes store normalized values; a lane recorded on the circuit
     parameter before the append will map differently. This is unavoidable when an enum
     grows.
2. `power_amp` (added this refactor) ids: `matched, bypass, power_mark_iic,
   power_twin_ab763, power_5150, power_2203_el34, power_1959_el34, power_ac30_el84,
   power_dr103_el34, power_recto_6l6, power_recto_6l6_tube, power_deluxe_6v6,
   power_2205_el34`. Saved presets now also store
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
| `pedal_boss_hm2` (pedal slot) | Heavy Metal | **Boss HM-2** Heavy Metal, the Japanese original | `heavy_metal.rs` | IMPLEMENTED 2026-09-16 | Boss's own service-notes drawing (`docs/schematics/boss-hm2.png`, untracked), read part by part; gyrator centres derived from it (87/958/1278 Hz) match the published analyses' 80 Hz and 900 Hz-1.3 kHz. Log: `docs/models/heavy_metal.md` | none | pedal slot | none | `pedal` id, appended | Heavy Metal |
| `pedal_boss_mt2` (pedal slot) | Metal Zone | **Boss MT-2** Metal Zone | `metal_zone.rs` | PARTIALLY IMPLEMENTED: all six controls exist; Mid Freq uses a dual-gang gyrator approximation whose boost/cut depth varies incorrectly across the sweep; isolated factory Wien stage under validation | Boss's own drawing (`docs/schematics/boss-mt2.png`, untracked) plus Electric Druid's analysis, which agree part for part; post-distortion gyrators derived from the sheet (4894/105 Hz) match its published 4,898/105 Hz. Log: `docs/models/metal_zone.md` | none | pedal slot | none | new `pedal` id | Metal Zone |
| (proposed `pedal_boss_ds1`) | Orange Dist | **Boss DS-1**, the 1978 TA7136AP version | none yet | RESEARCHED 2026-09-16 | ElectroSmash's value-by-value analysis. Open question: no TA7136AP data sheet located, so either that part is modelled from one or the later BA728N revision is built instead. Log: `docs/models/orange_dist.md` | none | pedal slot | none | new `pedal` id | Orange Dist |
| (proposed `pre_boogie_studio`) | Cali Studio Pre | **Mesa/Boogie Studio Preamp** (rackmount; *not* the Studio .22 combo) | none yet | RESEARCHED 2026-09-16, **NOT cleared**: no legible drawing | Mesa's own manual gives the control set; the only Studio Preamp sheet located is hand-drawn and cannot be read value-by-value, and the legible sheet in the same file is a different product (Studio Caliber). Log: `docs/models/studio_pre.md` | its own; the documented rig is a solid-state power amp behind it | physical cabinets | blocks *Never Mind '91*, *Seattle Ten '91* | new `circuit` id | Cali Studio Pre |
| `overdrive` | Overdrive | GENERIC op-amp with diodes in feedback (Si/Ge/LED) | `clipper.rs` (`OVERDRIVE`) | GENERIC | no hardware claim (MODELS.md: "not TS9/RAT") | none | none | 4 Overdrive presets | id/order fixed | Overdrive (keep) |
| `distortion` | Distortion | GENERIC diodes to ground (Si/Ge/LED) | `clipper.rs` (`DISTORTION`) | GENERIC | no hardware claim | none | none | 5 Distortion presets | id/order fixed | Distortion (keep) |

**Every pedal is also a `circuit`.** Appended to that enum on 2026-09-16 with ids
`pedal_ts9_circuit`, `pedal_rat_circuit`, `pedal_fuzz_face_circuit`,
`pedal_mxr_dist_plus_circuit`, `pedal_boss_hm2_circuit` and `pedal_boss_mt2_circuit` --
appended, because a saved automation lane stores a normalised value against the list's
order. The Green 808 and the Ram Fuzz were already there, from before the pedal slot
existed. `CALIBRATION` was regenerated for the six new voices; measured at a guitar's level
with the drive up, the RAT makes 43.8 % distortion, the MT-2 61.5 % and the HM-2 **88.0 %**.

## 3. Guitar amplifiers (preamp sections)

| Internal ID | Current display | Hardware inspiration | Implementation | Status | Source status | Power-stage status | Cab/speaker dependency | Presets | Serialization concerns | Proposed display |
|---|---|---|---|---|---|---|---|---|---|---|
| `markiic` (proposed `amp_mark_iic_plus`) | Cali IIC+ | Mesa/Boogie **Mark IIC+** lead channel, stock RP10A C+ **60 W** interpretation | `markiic.rs` (preamp + 5-band graphic), `power.rs::MARKIIC` | IMPLEMENTED preamp **including lead return, V2B, Lead Master and V2A (corrected 2026-09-15)**; graphic via differential feedback network (earlier scalar make-up removed) | [docs/models/cali_iic_plus.md](models/cali_iic_plus.md): archival preamp scan, community RP10/FINAL redraws (local copies); PI values discrepancy still recorded | Split at `TO EQ` -> graphic -> power (master pot stands in for MASTER). Matched = Cali 6L6. Panel Master = Lead Master | physical cabinets | Boutique Lead, Boutique Rhythm, Puppet Master '86 | display changed; legacy fixture blocks for this voice no longer compared | Cali IIC+ |
| `evh5150` (proposed `amp_5150`) | American 5150 | Peavey **EVH 5150** lead channel (code labels match the **5150 II** factory drawing's ULTRA controls; revision unresolved) | `evh5150.rs`, `power.rs::EVH5150` | PARTIALLY IMPLEMENTED: six triodes built; **tone stack not modelled** (plugin tone section substitutes *after* power) | [docs/models/american_5150.md](models/american_5150.md); local `peavey-5150.jpeg` | Split at `To Tone Stack` (33 k load). Matched = American 6L6 High-Gain; no resonance control | legacy filter | Ultra Lead, Ultra Rhythm | display changed | American 5150 |
| `twin` (proposed `amp_twin_reverb_ab763`) | American Twin | Fender **Twin Reverb AB763** (blackface) Vibrato channel | `twin.rs`, `power.rs::TWIN`, `dsp/spring.rs`, `dsp/tremolo.rs` | IMPLEMENTED coupled Vibrato/reverb/tremolo electrical path; spring mechanics external; stock 22 k PI tail / 450 V PI rail and shared A/B/C power rail corrected | [docs/models/american_twin.md](models/american_twin.md): original schematic and layout read | Split at channel output -> synthetic open master -> power. Matched = American 6L6 Clean | legacy filter | Blackface Clean, Blackface Throb | display changed (was "Twin Reverb") | American Twin |
| `amp_jcm800_2203` | Brit 800 | Marshall **JCM800 2203** (100 W) master-volume preamp, 1981 drawing | `brit800.rs`, `power.rs::BRIT_EL34` | IMPLEMENTED 2026-09-15 (four triodes, cathode follower, stack, droppers) | [docs/models/brit_800.md](models/brit_800.md): 1981 Marshall drawing with voltage table, 1988 Marshall drawing cross-check, local redraw; middle 22k / treble 220k / bass log resolved from 1988 | Split at the treble wiper into the master pot. Matched = Brit EL34 | physical cabinets | Brit Crunch, Brit Lead, Screamer Boost | appended to `circuit`; saved presets without ids migrated (`LEGACY_CIRCUIT_COUNT`) | Brit 800 |
| `amp_dual_rectifier` | Cali Rectifier | Mesa/Boogie **Dual Rectifier**, two-channel Rev F (RF-1F board), red channel modern | `rectifier.rs`, `power.rs::RECTO_6L6` | IMPLEMENTED 2026-09-16 (five triodes incl. the 39 k cold stage, follower, stack, master) | [docs/models/cali_rectifier.md](models/cali_rectifier.md): Mesa's own RF-1F preamp and power-amp sheets, with the Rectifier Guide for revisions | Split at the red master. Matched = Recto 6L6. silicon and separate valve-rectifier settings exist; Matched selects silicon | physical cabinets | Recto Rhythm, Recto Lead | appended to `circuit` | Cali Rectifier |
| `amp_marshall_1959` | Brit Plexi | Marshall **1959 Super Lead** (100 W, EL34), bright channel, Unicord drawing 70-6-11 July 1970 | `plexi.rs`, `power.rs::PLEXI_EL34` | IMPLEMENTED 2026-09-15 (three triodes, cathode follower, stack, 20k/10k/10k droppers) | [docs/models/brit_plexi.md](models/brit_plexi.md): Unicord 1970, Marshall c.1967 (voltages), Marshall 1988 1959 STD (pot laws, later changes), Robinette cross-check | Split at the treble wiper into the inverter's coupling cap (no master). Matched = Brit Plexi EL34 | physical cabinets | Plexi Crunch, Plexi Cranked, Blackout '80, Experienced '67 | appended to `circuit` | Brit Plexi |
| `amp_hiwatt_dr103` | Brit DR103 | **Hiwatt DR103** Custom 100, brilliant channel, Issue 4 factory sheets | `dr103.rs`, `power.rs::DR103_EL34` | IMPLEMENTED 2026-09-16 (five triodes, master volume, direct-coupled inverter) | [docs/models/brit_dr103.md](models/brit_dr103.md): Hiwatt's own 1994-95 sheets, Circuit Codex redraw of a late-60s amp, Ampbooks' inverter analysis | Split at the driver's cathode, with its direct voltage carried across. Matched = DR103 EL34 | physical cabinets | Hi-Headroom Clean/Pushed, The Great Wall '79 | appended to `circuit` | Brit DR103 |
| `amp_vox_ac30_tb` | Brit AC30 | **Vox AC30/6 Top Boost**, brilliant channel | `ac30.rs`, `power.rs::AC30_EL84` | IMPLEMENTED 2026-09-16 (three triodes, two-knob stack, cathode-biased EL84s, no loop) | [docs/models/brit_ac30.md](models/brit_ac30.md): Dallas 1974 and VSL 1971 factory sheets, voxac30.org.uk archive, Ampbooks | Split at the treble wiper into the 220 k mixers. Matched = AC30 EL84 | physical cabinets | Chime Clean, Chime Edge | appended to `circuit` | Brit AC30 |
| `amp_fender_deluxe_ab763` | American Deluxe | Fender **Deluxe Reverb AB763** (blackface) Vibrato channel | `deluxe.rs`, `power.rs::DELUXE_6V6`, `dsp/spring.rs`, `dsp/tremolo.rs` | IMPLEMENTED 2026-09-22 (four triodes, shared cathodes, reverb send/return, optical tremolo coupled ahead of the channel mixer) | [docs/models/american_deluxe.md](models/american_deluxe.md): original Fender AB763 drawing with its voltage table | Split at the channel output into the inverter's 1 M + 1 M grid-leak chain. Matched = American 6V6 | legacy filter | Blackface Deluxe, Deluxe Breakup | appended to `circuit` | American Deluxe |
| `amp_fender_deluxe_ab763_normal` | American Deluxe Normal | Fender **Deluxe Reverb AB763**, **Normal** channel | `deluxe.rs` (`Channel::Normal`), `power.rs::DELUXE_6V6` | IMPLEMENTED 2026-09-22: both channels are now built by one function and either can be the one played; the other stays in the network with its jacks open. Adding the Normal channel's missing first stage pulled every plate 4 % nearer the drawing | [docs/models/american_deluxe.md](models/american_deluxe.md) | Split at the channel output, as the Vibrato channel. Matched = American Deluxe 6V6 | American Open 1x12 on the C12N | Blackface Normal | appended to `circuit` | American Deluxe Normal |
| `amp_roland_jc120` | Jazz 120 | Roland **JC-120 Jazz Chorus**, CH-1, JC-120UT/JT Sep. 2000 | `jazz120.rs` | PARTIALLY IMPLEMENTED 2026-09-23: CH-1 complete, jacks to CN3 (two 2SK184 common-source stages, the passive 250k/250k/10k stack with its two slope caps, 1 M Volume and bright network, 2SC1815 follower); both power amplifiers and the MN3007 chorus built. CH-2 and the reverb outstanding | [docs/models/jazz_120.md](models/jazz_120.md): Roland's own 1334 ppi service notes, plus three earlier editions kept because **CH-1 differs in every generation** | `jc120_power.rs`: one of the two 60 W complementary transistor amplifiers, IMPLEMENTED 2026-09-23. Reproduces the sheet's printed 26 dB and its 61.8 Vpp at the printed 3.10 Vpp input. Not a `PowerSpec` -- no inverter, no grid leaks, no output transformer -- so `PowerModel::spec` became an `Option` and `PowerModel::Jazz120SS` is the first non-valve entry, speaker-loaded like the rest. `dsp/bbd.rs`: the MN3007/MN3101 chorus, IMPLEMENTED 2026-09-23 -- 1024 stages and the printed 2.5-12 us clock, so 1.28-6.14 ms swept by the sheet's 1.2 s triangle, with IC2A/IC2B's band either side and no companding because the board has none. APPROXIMATED as a modulated fractional delay rather than solved stage by stage; the split is taken at the end of the chain rather than ahead of the two power amplifiers, which commutes to within 0.81 %. **2026-09-23:** the stage gained `RAIL_WIRING_OHMS` (the reservoir's ESR and the wiring to the TR board, absent and the reason saturation was unconditioned) and D11 became a fixed drop, taking the nonlinear boundary from 20 of 31 unknowns to 18. **2026-09-23 evening:** R79 3.9 k, in parallel with R76, was missing from the bias spreader -- it held 1.48 V where a Darlington output stage needs 2.4, so the PNP half idled at -0.34 V of base-emitter bias, reverse biased and completely off. The amplifier was single-ended with a dead zone across every zero crossing. Found by measuring with a real DI take rather than a synthetic pluck; four of the six sheet-derived tests passed throughout, because none of them looks at crossover | **Jazz Open 2x12** with its **Jazz 12** (Roland 30-103D), built 2026-09-24: Roland's documented 750 x 540 x 270 mm open-back 2x12, the driver voiced to a published JC-120 measurement (1.16 dB rms); the amplifier's own output stage loaded by it | Jazz Clean, Jazz Chorus (both at Drive 0.16, where the amplifier is clean -- 1.6 % distortion and effectively all of it second harmonic; it shipped at 0.55 and 29 % while the make-up was cancelling VR1) | appended to `circuit`; `Gain::ALL` appended behind it, so `CALIBRATION` and `POWER_TRIM_DB` gained one row each | Jazz 120 |
| `amp_jcm800_2205` | Brit 2205 | Marshall **JCM800 2205** 50 W split-channel head, **boost channel**, later (1985-89) circuit, drawings 2205 Preamp issue 2 18-5-88 and 2205 STD Output Stage & PSU issue 2 4-5-88 | `brit2205.rs`, `power.rs::BRIT_2205_EL34` | IMPLEMENTED 2026-09-24: V1A (shared), the diode-biased V1B, both gangs of the Gain pot, the W005 bridge + D6 clipper (folded to one diode each way, exact for identical junctions and tested against the bridge), V3A, the plate-driven stack, the master rheostat, the passive mix and V4A; the muted Normal channel as the load it is | [docs/models/brit_2205.md](models/brit_2205.md): both Marshall CAD drawings, the split-channel owner's manual (panel, sensitivity, tone swings), the 1983 hand-drawn sheet for the earlier circuit; tone swings within 2 dB of the manual's, V1B within 5 % of an owner's reading | Split at the inverter's input (V4A plate, C25 in the power stage). Matched = Brit 2205 EL34 | physical cabinets | Machine Rage '92 | appended to `circuit`; `Gain::ALL`, `CALIBRATION` and `POWER_TRIM_DB` gained a row, `POWER_TRIM_DB` a column | Brit 2205 |
| `clean` / `crunch` / `highgain` | Clean / Crunch / High Gain | GENERIC 1/2/3 cascaded ECC83 stages | `preamp.rs`, `valve.rs` | GENERIC | no hardware claim | none (can use any power override) | legacy filter | Preamp/Crunch/High Gain presets (15) | id/order fixed | keep |

## 4. Power amplifiers

| Internal ID | Display | Hardware inspiration | Implementation | Status | Source status | Load | Presets | Serialization |
|---|---|---|---|---|---|---|---|---|
| `matched` | Matched | resolves to the selected amp's own power stage (None for pedals/studio/generic) | `voice::PowerAmp::resolved` | IMPLEMENTED | n/a | as resolved | all (default) | legacy default; `filter_state` + preset migration insert it |
| `bypass` | Bypass | no power stage (PI, tubes, NFB, OT all skipped; 73P line driver kept) | `voice.rs` | IMPLEMENTED | n/a | n/a | none | appended id |
| `power_mark_iic` | Cali 6L6 | Mark IIC+ 60 W: ECC83 LTP, 2x 6L6GC, NFB/presence | `power.rs::MARKIIC` | IMPLEMENTED (values not fully reconciled with RP10 drawing: see research log) | cali_iic_plus.md | **8 ohm resistor** | Boutique presets (matched) | appended id |
| `power_twin_ab763` | American 6L6 Clean | Twin Reverb AB763: ECC81 LTP, 4x 6L6GC, 820 ohm NFB | `power.rs::TWIN` | IMPLEMENTED; 22 k tail, 450 V PI C node and shared reservoir/choke/screen supply corrected | american_twin.md | **4 ohm resistor** | Blackface presets | appended id |
| `power_5150` | American 6L6 High-Gain | 5150: ECC83 LTP, 4x 6L6GC, 39 k NFB, presence, no resonance | `power.rs::EVH5150` | IMPLEMENTED (resonance/depth missing) | american_5150.md | **8 ohm resistor** | Ultra presets | appended id |
| `power_2203_el34` | Brit EL34 | JCM800 2203 1981: ECC83 LTP 82k/100k, 4x EL34 -42 V, 100 k NFB from 4 ohm, 22k/.1uF presence, Hammond 1750U data | `power.rs::BRIT_EL34` | IMPLEMENTED (supply impedances/core estimated) | brit_el34.md (1981 originals + 1988 cross-check) | 4 ohm resistor, or a speaker load on the physical path | Brit Crunch, Brit Lead, Screamer Boost (matched to Brit 800) | appended id |
| `power_1959_el34` | Brit Plexi EL34 | 1959 Super Lead 1970: 2203-family inverter and iron, no master, 47 k NFB from 16 ohm (23.5 k at 4), 5 k presence, -37 V (estimated) | `power.rs::PLEXI_EL34` | IMPLEMENTED 2026-09-15 (supply voltages from the 1967 drawing, bias and iron estimated) | brit_plexi.md | 4 ohm resistor or speaker load | matched to Brit Plexi | appended id |
| `power_dr103_el34` | DR103 EL34 | Hiwatt DR103: 82k/91k inverter direct-coupled to the driver, 22k/2k2 tail, 22 k stoppers, 4x EL34 -38 V, 10 k feedback from 16 ohm | `power.rs::DR103_EL34` | IMPLEMENTED 2026-09-16 (supplies and iron estimated; presence omitted) | brit_dr103.md | 8 ohm resistor or a speaker load | matched to Brit DR103 | appended id |
| `power_ac30_el84` | AC30 EL84 | AC30 Top Boost: cathode-biased 4x EL84 on 50 ohm/250 uF, no feedback, 250 k cut control, 4 k a-a | `power.rs::AC30_EL84` | IMPLEMENTED 2026-09-16 (supplies and iron estimated; GZ34 Child-law rectifier implemented) | brit_ac30.md | 8 ohm resistor or a speaker load | matched to Brit AC30 | appended id |
| `power_deluxe_6v6` | American Deluxe 6V6 | Deluxe Reverb AB763: 12AT7 LTP, 2x 6V6GT, GZ34 valve rectifier, 325 V PI supply, 47 ohm tail leg | `power.rs::DELUXE_6V6` | IMPLEMENTED 2026-09-22 (`PentodeSpec::T6V6GT` fitted from the datasheet curves by `tools/tube_fit/fit_6v6gt.py`; supplies and iron estimated) | american_deluxe.md | 8 ohm resistor or a speaker load | Blackface Deluxe, Deluxe Breakup (matched) | appended id |
| `power_2205_el34` | Brit 2205 EL34 | JCM800 2205 1988: ECC83 LTP 82k/100k with 330k/120k grid leaks and a 22k + 4k7 tail, 2x EL34, 100 k NFB from 4 ohm, 22 k presence, no master (it is in the preamp) | `power.rs::BRIT_2205_EL34` | IMPLEMENTED 2026-09-24. Supplies WIDELY REPORTED (460/450 V, measured 50 W JCM800s; the 1981 sheet's 365 V made 31 W against a 70 W rating and is recorded, not used); bias ESTIMATED for 38 mA; iron Hammond 1750N replacement data | brit_2205.md | 4 ohm resistor or a speaker load | Machine Rage '92 (matched) | appended id |
| `power_recto_6l6` | Recto 6L6 | Dual Rectifier Rev F silicon supply | `power.rs::RECTO_6L6` | IMPLEMENTED; transformer/supply assumptions remain | cali_rectifier.md | resistor or reactive speaker | Recto matched | appended id |
| `power_recto_6l6_tube` | Recto 6L6 Tube | Dual Rectifier Rev F valve supply | `power.rs::RECTO_6L6_TUBE` | IMPLEMENTED; two 5U4GB Child-law model | cali_rectifier.md | resistor or reactive speaker | explicit override | appended id |

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
| `spk_roland_30_103d` | Jazz 12 | Roland 30-103D 8 ohm (JC-120) | Roland service notes: size and impedance only; no T/S, sensitivity or plot published. Voicing FITTED to a published near-field JC-120 measurement, reproduced in the model (1.16 dB rms, 90 Hz-12 kHz); Fs/Bl/Re ESTIMATED from the catalogue's medians | IMPLEMENTED 2026-09-24 |

Cabinets (values marked DOCUMENTED / DERIVED / ESTIMATED / TUNED in docs/models/cabinets.md):

| Proposed ID | Display | Inspiration | Documented so far | Status |
|---|---|---|---|---|
| `cab_marshall_1960a` | Brit 1960 4x12 | Marshall 1960A (angled), G12T-75 factory | 770 W x 755 H x 365 D mm (marshall.com); 16/4 ohm mono | IMPLEMENTED |
| `cab_marshall_1960b` | Brit Closed 4x12 | Marshall 1960B (straight), G12T-75 | same outer dims (marshall.com) | IMPLEMENTED |
| `cab_marshall_1960ax` | Brit Green 4x12 | Marshall 1960AX, G12M-25 16 ohm | 770 x 755 x 365 mm | IMPLEMENTED |
| `cab_marshall_1960av` | Brit V30 4x12 | Marshall 1960AV (V30-loaded 1960) | geometry as 1960A; manufacturer page recorded in cabinets.md; 70 W G12 Vintage differs from the 60 W V30 profile | IMPLEMENTED |
| `cab_mesa_recto_standard` | Cali Oversized 4x12 | Mesa/Boogie Rectifier Standard 4x12, V30 | 32.9 H x 30.1 W x 14.25 D in (retail/Mesa text); ply thickness unverified | IMPLEMENTED |
| `cab_generic_oversized_412` | Oversized 4x12 | GENERIC oversized closed 4x12 | no hardware claim; geometry ESTIMATED | IMPLEMENTED |
| `cab_fender_twin_open_212` | American Open 2x12 | Fender Twin Reverb combo (AB763-style) open back | 20 H x 26-1/8 W x 10-1/2 D in (reproduction cabinet maker); openness ESTIMATED | IMPLEMENTED |
| `cab_fender_deluxe_open_112` | American Open 1x12 | Fender '65 Deluxe Reverb open back | 17.5 H x 24.5 W x 9.5 D in (retail spec) | IMPLEMENTED |
| `cab_marshall_1912` | Closed 1x12 | Marshall 1912 closed 1x12 | ~500 W x 470 H x 290 D mm (retail) | IMPLEMENTED |
| `cab_marshall_1936` | Closed 2x12 | Marshall 1936 closed 2x12 | 750 W x 600 H x 310 D mm (retail) | IMPLEMENTED |
| `cab_roland_jc120_212` | Jazz Open 2x12 | Roland JC-120 combo, open back | 750 W x 540 H x 270 D mm without casters, 2 x 30-103D (Roland service notes); open back WIDELY REPORTED, open fraction ESTIMATED | IMPLEMENTED 2026-09-24 |

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
| Tubes | ECC83, ECC82, ECC81, 6L6GC, EL34, EL84 (AC30) | `netlist.rs` | Koren-style fits |
| Semiconductors | BC549, BC109C, TIP3055, FS36999, 2N5457, J201, J113, 2SC3378, 1N914/1N4148, Si/Ge/LED | various | Ebers-Moll NPN and PNP (PNP added 2026-09-15), Shockley diodes, square-law JFET, AC128 |
| Op-amp | JRC4558 (TS808) and generic; LM308 (Rodent) | `netlist` `OpAmp`, `Transconductor`; `rodent::lm308` | ideal with rail limit; LM308 built from a saturating transconductor into an integrator for GBW and slew |

## 6b. Planned: the modern high-gain chain and bass

Requested 2026-09-16 as two addenda. **None is implemented**, and none may be until its
schematic-first checkpoint is done; the requirements, the research each one needs and the
cross-combination tests are written up in [ROADMAP.md](ROADMAP.md).

Modern high-gain chain:

| Proposed ID | Display | Hardware inspiration | Status |
|---|---|---|---|
| `pedal_revv_g3` | Modern Purple | **Revv G3 / G3 V2** (revision to be chosen) | PLANNED, not researched. A distortion/preamp, not a boost; the Aggression switch must be established from the circuit |
| `pedal_fortin_33` | Modern 33 | **Fortin 33** | PLANNED, not researched. A frequency-shaping boost for an already-distorted amplifier -- a different device from Modern Purple, not a re-EQ of it |
| `amp_revv_generator` | Modern Generator | **Revv Generator 120** (MK3 / current revision) | PLANNED, not researched. Purple channel primarily, Red channel researched before deciding on multiple channels. Its own topology, not the Cali IIC+ with new constants |
| `power_revv_generator_6l6` | Modern 6L6 | the Generator-family power stage | PLANNED, not researched. Phase inverter, 6L6 configuration, bias, feedback, Presence, **Depth in the feedback loop where the circuit puts it**, transformer, supply, load interaction |
| `cab_modern_oversized_4x12` | Modern Oversized 4x12 | modern oversized sealed high-gain 4x12 designs | PLANNED, not researched. A GainStageFx design informed by documented real geometry; **not** the Cali Oversized profile renamed. Pairs with the existing `spk_celestion_v30`, reused |

Bass -- amplifiers, their power stages, and drives:

| Proposed ID | Display | Hardware inspiration | Status |
|---|---|---|---|
| `amp_ampeg_svt` | American SVT | **Ampeg SVT** (revision to be chosen; the name covers several amplifiers) | PLANNED, not researched |
| `power_svt_6550` | American 6550 | the SVT-family power stage | PLANNED. A large part of the model, not an afterthought |
| `amp_gk_800rb` | American 800RB | **Gallien-Krueger 800RB** | PLANNED. A different architecture, not a cleaner SVT |
| `power_gk_800rb` | American SS 800 | the 800RB solid-state power stage | PLANNED. Solid-state; no invented tube sag |
| `amp_darkglass_microtubes_900` | Modern Micro 900 | **Darkglass Microtubes 900** (generation to be chosen) | PLANNED. Model the electronics, not valve equations |
| `power_modern_bass_900` | Modern Bass Power | the Microtubes 900 power stage | PLANNED. If Class-D, a justified audio-band reduction rather than switching simulation |
| `pedal_sansamp_bass_driver` | Bass Driver | **Tech 21 SansAmp Bass Driver DI** (generation to be chosen) | PLANNED. Check Blend's real topology for phase; its speaker emulation stacks with the plugin's cabinet |
| `pedal_darkglass_b3k` | Modern Micro B3 | **Darkglass Microtubes B3K** | PLANNED. Derive the parallel architecture from the circuit rather than assuming it |
| `pedal_bass_big_muff` | Bass Fuzz Pi | **EHX Bass Big Muff Pi** | PLANNED. Researched separately from the Ram Fuzz; reuse `circuits::bigmuff` where topology is genuinely shared |
| (none) | Bass Rodent | ProCo RAT, already modelled | **Reuse, do not duplicate**: a bass-oriented configuration of the existing `pedal_rat`, unless research shows the hardware differs |

Bass cabinets (8x10 sealed, 4x10, 1x15) and the DI tap are architectural work rather than
a single model; see the roadmap. `spk_celestion_v30` and the existing microphones are
reused throughout rather than duplicated.

## 7. Gaps between repository and targets

- **Implemented and already renamed:** Green 808, Cali IIC+, American 5150,
  American Twin, British 73. Also the power stages Cali 6L6, American 6L6 Clean,
  American 6L6 High-Gain and Brit EL34.
- **Existing models that are not on the target list and must be preserved:**
  Big Muff Ram's Head (selectable), the Triangle and Supa Tonebender voicings
  (not selectable), the seven generic topologies, the Iron materials, the Twin
  spring and tremolo, and the legacy Combo/Stack cabinets.
- **Existing partial circuits:** MT-2 middle EQ, 5150 stack/resonance and revision,
  DR103 presence, IIC+ power values; see section 9. HM-2 and MT-2 both run today.
- **Missing preamps/amps:** Studio Preamp, modern and bass models from ROADMAP.md.
- **Missing power stages:** the modern Generator and bass families from ROADMAP.md.
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
    added to `power.rs`; the GZ34 rectifier device is also present.
11. DONE: **Brit DR103**: direct-coupled inverter, which needed a biased input in the
    netlist so one block can hand the next its direct voltage.
12. DONE: **Cali Rectifier + its power stage** (two-channel Rev F, from Mesa's own
    sheets; silicon and valve supply selections are present).
13. DONE: **American 312, Tube 610** (plus British 4K E).
14. **Versioned corrections** of inherited approximations: the 5150 tone stack,
    DR103 presence and Mark IIC+ PI values. Twin PI/supply and Mark recovery/master/graphic corrections are already present. Each needs its
    own regression checkpoint and must never silently alter Matched legacy output.
15. **Factory era presets** once their components exist, with rig research logs that
    mark each claim DOCUMENTED / WIDELY REPORTED / PLAUSIBLE / APPROXIMATED.

## 9. Current completion and validation audit (2026-09-20)

Use **not started / researching / schematic acquired / partial / implemented /
validated / performance-ready** as evidence milestones. Here `implemented` means
executable code with the stated scope, not a complete replica. `Tests` names
existing coverage, not a claim that the current checkout has passed it; the
current suite and realtime results belong in IMPLEMENTATION_PROGRESS.md.
**No nonlinear family is blanket-qualified performance-ready at 48 kHz / block 64.**
Performance must include p99, max, misses and unsettled counts for the complete
chain with profiling disabled. `Unqualified` below means no current per-device
hard-deadline qualification has been established by this audit.

| Device | Category | Status | Reference schematic found? | Reference documentation found? | Implementation status | Validation status | Known issues | Tests | Performance |
|---|---|---|---|---|---|---|---|---|---|---|
| DIYRE 73P v1.1 / British 73 | Preamp | implemented | Local manufacturer drawings | Local drawings; online log missing | Input, PRE1/PRE2, output/iron | Circuit tests | Not original 1073; winding/device fits | `neve`, `preamp` | Unqualified |
| API 312 / American 312 | Preamp | partial | Factory card | API 2520/transformer sheets | Card netlist, ideal rail-limited 2520 | Gain/transformer tests | 2520 transistor dynamics absent | `american312` | Unqualified |
| SSL 82E01 / British 4K E | Preamp | implemented | Factory revisions logged | Jensen sheets | Two amplifiers and input iron | Gain/response tests | Estimated core | `console_e` | Unqualified |
| UA 610-A / Tube 610 | Preamp | partial | C-10068 redraw | UTC, RCA | Two feedback pairs; EQ fixed flat, gain Hi | Response/DC tests | EQ/gain switches not exposed; rail/iron estimates | `tube610` | Unqualified |
| TS808 / Green 808 | Pedal | implemented | Traced drawings, not factory | Analysis and revision log | Buffers, feedback clipping, tone, level | Circuit/slot tests | Ideal op-amp and approximate device fits | `ts808`, `pedal_slot` | Unqualified |
| TS9 / Green 9 | Pedal | implemented | Traced revision | Revision log | Shared TS808 builder with output values | Circuit/slot tests | Op-amp variations absent | `pedal_slot` | Unqualified |
| Ram's Head / Ram Fuzz | Pedal | implemented | Traced archive | Version archive | Four BJTs, two clippers, passive tone | Clipping/control tests | Exact traced-unit value reconciliation outstanding | `bigmuff` | Unqualified |
| Triangle / Supa Tonebender | Pedal variants | partial | Traced archive | Version archive | Netlist constants, not selectable | Builder coverage | No host choice/calibration | `bigmuff` | Unqualified |
| LM308 RAT / Rodent | Pedal | implemented | Traced drawing | LM308 data | Dynamic amplifier, diodes, JFET | Clipping/control tests | Device fits | `rodent` | Unqualified |
| Germanium Fuzz Face / Round Fuzz | Pedal | implemented | Traced drawing | Revision survey | Native PNP pair | Bias/control tests | AC128 parameters approximate; cap conflict logged | `round_fuzz` | Unqualified |
| MXR Distortion+ / Yellow Dist | Pedal | implemented | Traced drawing | 741/analysis | Dynamic 741, germanium pair | Gain/clipping tests | Semiconductor fit | `distortion_plus` | Unqualified |
| Boss HM-2 / Heavy Metal | Pedal | partial | Boss drawing | Research log | Audio chain and controls | Gyrator/control/reference tests | Equivalent inductors omit active-gyrator limits; device fits | `heavy_metal`, internal partition tests | Tail/stability qualification pending |
| Boss MT-2 / Metal Zone | Pedal | partial | Boss April 1991 service notes | Factory appendix and analysis | Shipping swept gyrator; isolated original Wien EQ in development | Existing band tests; new isolated EQ tests | Shipping mid depth wrong; ideal op-amps, equivalent gyrators | `metal_zone`, internal partition tests | Tail/stability qualification pending |
| Boss DS-1 / Orange Dist | Pedal | researching | Traced drawing; factory sheet not logged | Analysis; TA7136AP data unresolved | No netlist | None | Choose 1978 TA7136AP or 1994 BA728N; do not hybridize | None | Not measured |
| Mark IIC+ / Cali IIC+ | Amp and power | partial | Archival and RP10A traced drawings | Revision logs | Recovery/master/graphic now present; separate 60 W power | Stage/gain/graphic tests | PI/presence values not reconciled; ideal graphic devices | `markiic`, `graphic`, `pentode` | Unqualified |
| 5150 / American 5150 | Amp and power | partial | Factory 5150 and 5150 II acquired | Factory manual | Six-stage preamp and 6L6 power | Gain/clip/legacy tests | Revision unresolved; 33k dummy stack and post-power generic EQ; resonance absent | `evh5150`, `legacy_baseline` | Ultra Lead chain is active stress fixture |
| AB763 / American Twin | Amp and power | implemented | Factory schematic/layout | Tube data | Coupled effects electrical path, stock PI/shared supply | DC, loading, solver/reference tests | Spring mechanics behavioral; iron estimates | `twin`, `pentode`, Twin trace/reference tests | Hard-deadline target still active |
| 1981 JCM800 2203 / Brit 800 | Amp and power | implemented | Marshall 1981/1988 | Voltage table; Hammond | Preamp/stack and Brit EL34 | DC/gain/controls/rates | Fixed handoff; supply/iron estimates | `brit800`, `modular_power` | Unqualified |
| 1970 1959 / Brit Plexi | Amp and power | implemented | Unicord factory | Marshall cross-check | Bright channel and separate power | DC/controls | No jumper/modified 1959T claim; estimates | `plexi` | Unqualified |
| AC30/6 Top Boost / Brit AC30 | Amp and power | implemented | 1971/1974 factory | Tube/OT data | Top Boost, cathode bias, cut, GZ34 | Bias/controls/sag | Documented mixed rectifier/passive reference; iron estimates | `ac30`, `devices` | Unqualified |
| DR103 Issue 4 / Brit DR103 | Amp and power | partial | Hiwatt factory sheets | Revision/PI analysis | Brilliant channel, driver, direct-coupled PI | DC/controls | Presence feeds preamp driver across present split; omitted | `dr103` | Unqualified |
| Dual Rectifier Rev F / Cali Rectifier | Amp and power | implemented | Mesa RF-1F and power | Revision/manual | Red Modern, silicon and 5U4GB supplies | DC/clipping/sag/rates | Other modes omitted; OT estimates | `rectifier`, `mains` | Unqualified |
| Mesa Studio Preamp | Preamp | researching | Available drawing insufficiently legible | Factory owner manual | No netlist | None | Studio .22 and Studio Caliber are different products | None | Not measured |
| Celestion V30, G12M-25, G12T-75 | Speakers | partial | Not applicable | Manufacturer Fs/Re/SPL | Reactive load and breakup fit | Analytic impedance/chain tests | Most T/S estimated; no thermal/excursion model | `speaker_load`, `acoustic_chain` | Host-rate path; full-chain qualification pending |
| Jensen P12R, P10R, C12N, P12N | Speakers | implemented | Not applicable | Manufacturer T/S and curves | Reactive load and breakup fit | Manufacturer curve comparison | P10R evidence conflict; no thermal/excursion model | `speaker_load` | Full-chain qualification pending |
| Ten physical guitar cabinets | Cabinets | implemented | Construction drawings unavailable | Manufacturer/retail dimensions | Compliance, geometry, arrays, rear radiation | Geometric/response tests | Four-driver position capacity; estimated layout/panels/slant | `acoustics`, `acoustic_chain` | Host-rate; qualified only by measured chain |
| SM57, MD421 II, MD409 U3, R121, M160, 4038, U87 Ai | Microphones | implemented | Not needed for profile scope | Manufacturer charts | Fitted response/placement/polar/dual mic | Placement/cancellation/rates tests | Estimated proximity/off-axis/nonlinearity; no circuit claim | `acoustics` | Host-rate, allocation tests exist |
| e906, C414, U67, U47 fet | Microphones | partial | Not needed for profile scope | Incomplete charts/specifications | Response profiles | Structural/placement tests | e906 secondary curve; three approximate curves; C414 revision unresolved | `acoustics` | Host-rate, allocation tests exist |
| Revv G3, Fortin 33, Generator 120, Generator power | Modern chain | not started | Not yet acquired in repository | Research needed | No netlists | None | Revisions, switching and Depth must be established | None | Not measured |
| Modern Oversized 4x12 | Cabinet | not started | Not applicable | Geometry research needed | No distinct profile | None | Do not rename existing oversized profile | None | Not measured |
| Ampeg SVT/6550, GK800RB/SS, Microtubes900/power | Bass amps/power | not started | Not yet acquired in repository | Research needed | No netlists | None | Revision and solid-state/Class-D model scope | None | Not measured |
| SansAmp Bass Driver, B3K, Bass Big Muff | Bass pedals | not started | Not yet acquired in repository | Research needed | No netlists | None | Blend topology/revision; reuse Muff only where justified | None | Not measured |
| 8x10, 4x10, 1x15; bass DI blend | Bass acoustics/routing | not started | Not applicable | Driver/construction/tap research needed | Four-driver array capacity currently blocks 8x10 | None | Add eight positions and up to sixteen front/rear paths per mic; select DI tap explicitly | None | Not measured |
| Album-rig blockers from ROADMAP section C | Amps/effects/routing | researching | Per-device status varies | PRESETS rig research | No substitute aliases | None | Modified 1959T, Fish/VHT/Ecstasy/Rockman parallel rig, Sunn Model T, Super Bass, Major, Ampeg VT, Laney/treble boost | None | Not measured |

The seven generic gain choices, nickel/steel/amorphous iron, Wide/Scooping tone,
legacy Combo/Stack filters and the Twin behavioral mechanical effects remain
intentional designed/approximate models. They are not undocumented hardware stubs.

### Immediate completion order

1. Stabilize current solver experiments and repair failing tests before changing
   shipping circuit behavior. Preserve the reduced/full reference and callback evidence.
2. **MT-2 original mid EQ:** the isolated U2a/U2b Wien stage from the
   [April 1991 factory sheet](https://guitar-gear.ru/forum/index.php?app=core&attach_id=50406&module=attach&section=attach)
   is now implemented as `metal_zone::mid_eq_reference`, with boost/cut, center-flat
   and multi-rate small-signal AC/time tests added but not yet executed at this
   handoff. Run those tests, add headroom/state/loading coverage, then integrate with
   the low/high stage while preserving its actual loading, remeasure calibration and
   compare full-pedal/preset sound and callback cost. Mechanical C/G pot laws remain
   a separately documented uncertainty.
3. **5150 revision and stack:** factory
   [5150 II service set](https://www.thetubestore.com/lib/thetubestore/schematics/Peavey/Peavey-5150-II-Schematic.pdf)
   and [original EVH drawing](https://el34world.com/charts/Schematics/files/Peavey/5150_evh.pdf)
   were reopened during this audit. The former is legible when rendered at high
   resolution. Reconcile its complete preamp/stack with the current six-stage
   code before choosing a revision or replacing the 33k surrogate. This changes
   gain/loading and needs an explicit legacy/version strategy, not a speed tweak.
4. Complete DR103 cross-boundary presence and IIC+ PI/power reconciliation.
   Improve existing active-device/gyrator fits with measured operating envelopes.
5. Finish DS-1 device evidence and Studio Preamp schematic acquisition; then the
   modern and bass families in ROADMAP.md. Bass cabinet work must expand the actual
   `CabinetProfile::positions` and acoustic path capacities, not just add a profile.
