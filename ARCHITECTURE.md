# GainStageFX architecture audit

For the current implementation and continuation checklist, see
[IMPLEMENTATION_RESUME.md](IMPLEMENTATION_RESUME.md). The audit below records the
original architecture; the following paragraph summarizes changes since then.

The chain now selects complete power circuits independently, including a new
1981 2203-inspired Brit EL34 stage. Matched retains the old routing. The 73P line
driver has separate storage and remains part of the studio preamp. Host/saved-state
migration defaults missing power selection to Matched; saved presets carry stable
model IDs. Physical speaker/cabinet/microphone processing is still pending.

Audit completed 2026-09-14, before implementation changes. Working tree was clean.
Version: 0.10.0. Rust/nih-plug pinned to f36931f7; CLAP and VST3 exported from
plugin.rs; Vizia GUI with vendored baseview. No applicable AGENTS.md found.

## Inspected surfaces

src/voice.rs, plugin.rs, params.rs, presets.rs, calibration.rs, stereo_worker.rs;
src/editor/{mod,panel,style,widgets,session}.rs; all circuit module inventories and
builders, especially markiic, evh5150, twin, power, neve, ts808, cabinet;
src/dsp/{netlist,time,device,partition,ac,complex,oversample,spring,tremolo,measure};
Cargo.toml, README, .gitignore, tests and examples inventories, test support,
local performance/attack/duplicated-mono reports, local schematic inventory.
Source comments referring to CLAUDE.md point to a file absent from this checkout.
README model/test/preset counts and panel dimensions are stale.

## Actual signal flow before refactor

Input trim -> calibration volts -> oversampler [selected gain netlist -> Mark
five-band graphic (Mark only) -> associated output netlist -> optional iron ->
makeup] -> Twin tremolo and reverb sum -> latency padding -> plugin tone ->
legacy cabinet -> switch fade -> wet/dry mix and output trim.

The Twin effects are actually applied AFTER power processing, despite comments
claiming a preamp placement. The tank is fed from scaled input, not a dynamically
solved driver tap. The 5150 uses the plugin tone section AFTER the power amplifier
as a substitute for its unreadable stack. Preserve these legacy behaviors until a
versioned topology correction has its own regression and research checkpoint.

## Existing amplifier boundaries (internally separated, not user modular)

| Family | Preamp boundary in implementation | Following processing |
| --- | --- | --- |
| Mark IIC+ | RP10A stock lead interpretation: input -> V1A -> Fender stack -> Volume 1 (rest .8) -> V1B -> Lead Drive -> V3B -> V4A -> C30/R31/C31/R32/C32 -> lead_out, 1 Mohm load | Separate graphic RLC netlist with scalar driver recovery -> Lead Master -> ECC83 LTP -> pair 6L6GC -> transformer -> 8 ohms; secondary NFB/presence in same power solve |
| 5150 | Six triodes V1A/V1B/V2A cold clipper/V2B/V5B/V5A -> C58 -> R89 -> stack, 33 kohm shunt | Ultra Post -> ECC83 LTP -> four 6L6GC -> transformer -> 8 ohms; NFB/presence, no resonance control; substitute tone follows power |
| Twin | AB763-inspired Vibrato channel: 68k input -> 7025 -> stack -> Volume/always-on bright cap -> recovery 7025 -> .1uF -> out, 1 Mohm | Synthetic open master -> ECC81 LTP -> four 6L6GC -> transformer -> 4 ohms; 820 ohm NFB, practically absent presence; effects approximation as above |
| JCM800 2203 | Not implemented (local schematic PDF exists) | None |
| Dual Rectifier | Not implemented | None |
| DR103 | Not implemented | None |
| AC30 | Not implemented | None |

PowerSpec separates immutable values from Simulation. power::build composes
master, phase inverter, bias, grid coupling, pentodes, RC plate/screen supplies,
centre-tapped ideal transformer halves, nonlinear core, leakage, load, and feedback
in ONE netlist. Its load is only a resistor. No feedback from cabinet to amp exists.
The preamp/power split is one-way voltage transfer with fixed impedances, not a
joint circuit solve. No last-waveshaper shortcut is necessary or appropriate.

Neve is different: Gain::Neve routes its 73P input/PRE1/PRE2 netlist into a separate
BC109C/TIP3055 output/VTB1148 line-driver netlist. This must remain part of the
studio preamp even when guitar power amplification is bypassed.

## Runtime and host integration

Chain preconstructs 21 gain variants, matching optional output simulations, three
iron models, two tones, two cabinets, Mark graphic, Twin tank/recovery/tremolo.
All selection uses existing storage. DC settling is deferred until selected;
operating points are shared between identically configured channels. Full runtime
state is independently owned. Exact duplicated mono uses one chain until the first
differing frame; then histories copy into the dormant chain and stereo stays latched
until reset. Persistent right worker uses atomics and job stealing, but waits with
an UNBOUNDED spin if the worker has started: existing realtime-policy violation.

No plugin FIFO. Host frames process directly; stereo trim/mix scratch is sized in
initialize. FIR oversampling supports 1/2/4/8; all hardware models are pinned to
1x by existing CPU policy, including TS808/Big Muff. Generic nonlinear voices use
requested quality. Gain/output/graphic/iron share effective rate; tone/cabinet and
Twin effects run at host rate. initialize rebuilds chains; Chain::set_rate also
retunes all sections and reconstructs the spring tank outside the callback.

Latency is fixed 66 host samples at every quality (padding plus FIR delay), with
66-sample dry alignment. Host bypass returns input bits unchanged while hidden DSP
advances: existing documented departure from latency-preserving bypass.

Controls have 20ms nih-plug smoothers. Circuit controls use one next_step(block)
update per callback; trim/mix ramp samplewise. Makeup uses fixed .02/sample glide
(not rate invariant). Selection uses 256-sample recursive last-output fade (see
actual FADE_LEN in voice.rs), not parallel old/new processing.

GUI selectors and knobs bind parameter pointers through Vizia RawParamEvent.
Model IDs are explicit nih Enum ids; their order must also remain stable because
saved JSON stores NORMALIZED numeric values. Host state uses nih serialization;
no custom state migration exists. Saved presets are name + map, no schema version.
Missing values currently leave prior controls untouched. This is unsafe for new
routing controls and requires explicit legacy defaults.

## Validation before implementation

Existing tests cover AC/time agreement, nonlinear devices, stage levels, tone,
master, controls, calibration, allocations, reset/state copy, stereo wake-up,
plugin callbacks and presets. Examples provide offline rendering, alias metrics,
partition/solver/power/preset cost and performance audits. Guitar DI exists at
tests/fixtures/01-260912_1044.wav. Full release baseline launched; results tracked
in IMPLEMENTATION_PROGRESS.md. Historical local reports already record rare power
solver failures and stereo deadline misses; do not claim these are newly solved.
