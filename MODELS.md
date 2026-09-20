# Model inventory and evidence policy

Current audit: **2026-09-20**, reconciled with `src/circuits`, `src/acoustics`,
`src/params.rs`, model research logs and tests. The checkout has 27 selectable
`Circuit` entries, eight pedal models, nine explicit power stages (plus Matched
and Bypass), seven speakers, ten physical cabinets and eleven microphone profiles.
An implemented selection is not a claim of complete hardware fidelity or a
48 kHz / 64-sample realtime qualification.

The detailed [inventory](docs/MODEL_INVENTORY.md#9-current-completion-and-validation-audit-2026-09-20)
records implementation, reference availability, validation gaps, tests and performance.
[ROADMAP.md](docs/ROADMAP.md) retains the requested modern, bass and album-rig work.

| Family | Present | Important unfinished work |
| --- | --- | --- |
| Guitar amplifiers | Cali IIC+, American 5150, American Twin, Brit 800, Brit Plexi, Brit AC30, Brit DR103, Cali Rectifier | 5150 revision/stack/resonance; DR103 presence loop; IIC+ power values; measured transformer data |
| Studio preamps | British 73 (DIYRE 73P v1.1, not a literal 1073), American 312, British 4K E, Tube 610 | 73P online provenance; 2520 transistor/dynamic behavior; 610 EQ/gain switching; Studio Preamp has no legible verified circuit yet |
| Pedals | Green 808, Green 9, Ram Fuzz, Rodent, Round Fuzz, Yellow Dist, Heavy Metal, Metal Zone | MT-2 mid EQ is a swept approximation with incorrect depth; HM-2/MT-2 active gyrators and device fits need further validation; DS-1 researched, not implemented |
| Power stages | IIC+, Twin, 5150, 2203, Plexi, AC30, DR103, Rectifier silicon and valve settings | Hardware revision/transformer uncertainties; harder loads and complete chains still need realtime qualification |
| Speakers/cabinets/mics | Reactive speaker load in the power circuit; physical cabinet/array path; placement and dual microphones | Estimated Celestion parameters; nonlinear/thermal speakers; construction data; C414 revision and several mic response charts |
| Modern high gain | Existing Rectifier and 5150 families | Revv G3, Fortin 33, Generator and its power section remain not started |
| Bass | Existing RAT can be reused as a bass preset | SVT, GK 800RB, Microtubes 900, their power stages, B3K, SansAmp Bass Driver, Bass Big Muff, bass drivers/cabinets and DI blend remain not started |

The AC30 GZ34, Rectifier valve option (`Part::Rectifier`) and mains scaling are
already implemented. MT-2 Mid Freq also exists; the gap is its circuit topology
and boost/cut behavior, not a missing knob. Twin's AB763 electrical effects path,
PI tail and shared power supply were corrected on 2026-09-17. The IIC+ recovery,
lead master and graphic corrections are also present. Earlier audit text that
calls these absent is historical.

Status vocabulary for the current checklist: **not started**, **researching**,
**schematic acquired**, **partial**, **implemented**, **validated**,
**performance-ready**. These are evidence milestones, not synonyms. Validation
must name its scope; an AC control test does not validate nonlinear hardware
behavior, and a mean-CPU measurement does not establish callback deadlines.
No blanket performance-ready claim is made for the nonlinear catalogue.

Work order after stabilizing the solver baseline:

1. Validate the now-implemented isolated April 1991 MT-2 U2a/U2b Wien middle
   stage, add headroom/state/loading coverage, then integrate it into the production
   pedal with the real low/high-stage source and Level load. Recalibrate and compare
   presets/realtime cost; do not silently replace legacy sessions without a policy.
2. Resolve the American 5150's exact revision and reconstruct its loaded tone
   stack before the power stage; add its resonance network from that same drawing.
   Preserve an explicit legacy path or document/version the resulting sound change.
3. Complete the DR103's feedback into the preamp driver with an exact coupled
   representation, then reconcile IIC+ PI/power values and existing device fits.
4. Complete the DS-1 research checkpoint, choosing TA7136AP or BA728N explicitly.
   Continue the Studio Preamp drawing search without borrowing another product's circuit.
5. Work through the modern/bass roadmap with original schematics first; expand
   array capacity and obtain bass driver parameters before claiming 8x10 support.

Every hardware change needs a per-device log in `docs/models` **before code**:
revision; original schematic found?; best source; independent cross-check; actual
component/device/supply/transformer/pot/switch values; exact DSP mapping; unresolved
evidence; and approximation rationale. Read the drawing itself. Never silently
resolve a conflicting value or use a near relative as an unavailable model.

Label claims CIRCUIT DERIVED, PHYSICS DERIVED, PUBLISHED-PARAMETER DERIVED,
EMPIRICALLY TUNED or APPROXIMATED, and distinguish DOCUMENTED from ESTIMATED values.
Display names remain generic; serialized IDs and enum order remain append-only.
Keep generic circuits, legacy filters and trusted reference paths. Source comments
describe implementation, not independent proof of hardware fidelity.
