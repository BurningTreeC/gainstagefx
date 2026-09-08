//! The DIY Recording Equipment 73P microphone preamplifier, from the drawings.
//!
//! TARGET: DIY Recording Equipment 73P (500-series Neve-style card)
//! REVISION: 1.1
//! SCHEMATIC: `docs/schematics/neve_73p_balanced_input.png`,
//!            `docs/schematics/neve_73p_gain.png`,
//!            `docs/schematics/neve_73p_preamp_1.png`,
//!            `docs/schematics/neve_73p_preamp_2.png`,
//!            `docs/schematics/neve_73p_output.png`
//! SOURCE: project-local drawings (primary), DIYRE assembly guide text on the
//!         same pages (secondary, used only for the stated stage gains)
//! CONFIDENCE: high for PRE 1 and PRE 2 topology and values, which were traced
//!             node by node off the drawings and then checked against the
//!             stage gains the drawings' own text states.
//! APPROXIMATION: the OUTPUT block is not modelled; the twelve-position gain
//!                switch is swept continuously between its own endpoints; the
//!                VR3 matching trimpot sits at mid travel. All three are set
//!                out below.
//!
//! # What the card is
//!
//! Three identical three-transistor amplifier blocks in a row. PRE 1 and PRE 2
//! make the gain; OUTPUT drives the line. Each block is a common-emitter input
//! stage, a second common-emitter voltage stage, and an emitter-follower
//! buffer, wrapped in series feedback from the follower's emitter back to the
//! *emitter* of the input transistor. That last detail is the whole circuit:
//!
//! ```text
//!             +-------------------- R12 2k2 --------------------+
//!             |                                                 |
//!   IN --C9-- b(T1)                                             |
//!             e ------------------------------------------------+
//!             |                                                 |
//!            R13 390R                                        e(T2) --> OUT
//!             |                                                 b
//!            C3 470u                                            |
//!             |                                              c(T3)
//!         R5 1k8 || R4 91R                            (T1 c -> T3 b)
//!             |
//!            gnd
//! ```
//!
//! With C3 a short at audio the leg from T1's emitter to ground is
//! `R13 || R5 || R4`, and the closed-loop gain is `1 + R12 / that`:
//!
//! - PRE 1: `R5 || R4` = 1800 || 91 = 86.6 R; with R13 that is 70.9 R;
//!   `1 + 2200/70.9` = 32.0, or **30.1 dB**.
//! - PRE 2 with nothing on its GAIN pin: 1800 || 390 = 320.9 R;
//!   `1 + 2200/320.9` = 7.86, or **17.9 dB**.
//! - PRE 2 with R61 330 R hung on the GAIN pin: **23.2 dB**.
//! - PRE 2 with R62 120 R: **28.4 dB**.
//!
//! The drawings' own text says 30 dB, 18 dB, 23 dB and 28 dB. Four independent
//! arithmetic checks against four stated figures is the evidence that the
//! traced topology is the drawn one, and the reason R4 belongs in PRE 1's
//! feedback leg rather than anywhere else.
//!
//! # The gain switch
//!
//! The real control is a twelve-position rotary switch doing three jobs at
//! once: a resistive divider on the input (0-35 dB), the PRE 2 feedback leg
//! above (R61 and R62 at the 45 and 50 dB positions), and whether PRE 1 is in
//! circuit at all (55 dB and above). This model is wired as the 70 dB
//! position -- both stages, no attenuation between them -- and the Drive knob
//! sweeps the PRE 2 feedback leg between 10 k, which is far enough above R71
//! to leave the stage at its open-circuit gain, and R62 at the top.
//!
//! That is an engineering approximation and it is exactly this much of one.
//! The top of the sweep is R62 off the drawing and the stage gain there is the
//! drawing's 28 dB. The bottom stands in for the open circuit and is 0.2 dB
//! short of it. The taper is sized so the middle of the knob lands at 331 R,
//! which is the switch's own middle resistor R61 to within one per cent, and
//! the sweep is therefore even in decibels across the whole travel. What is
//! missing is that the hardware arrives at those three points in detents
//! rather than smoothly. The input divider and the PRE 1 bypass are not
//! modelled at all: the plugin's own input level and make-up do that job, and
//! doing it twice would be worse than not doing it here.
//!
//! # What is not here
//!
//! The OUTPUT block -- T6/T7 BC109C, T8 TIP3055, and the VTB1148 output
//! transformer, together 18 dB of which 4 dB is the iron -- is not modelled.
//! The plugin's own output transformer stage is a separate control and stands
//! in for the iron; the 14 dB of transistor gain in front of it does not, and
//! its clipping behaviour is therefore missing from this voice.

use crate::dsp::netlist::{BipolarSpec, Circuit, CoreSpec, Fault, Netlist, Taper};

/// The gain-setting resistance on PRE 2's GAIN pin, as a control.
pub const GAIN: usize = 0;
/// VR2, the 10 k output trim on the front panel.
pub const TRIM: usize = 1;

/// The BC109C on the card: a high-beta, low-noise small-signal NPN.
/// T8, the power device in the OUTPUT block. A TIP3055 is an epitaxial power
/// NPN: far more junction than a BC109C, so a much larger saturation current,
/// and a beta an order of magnitude lower. Both matter here -- it is the low
/// beta that makes T7 have to drive it, and the large junction that puts its
/// base a little further up than a small-signal part would sit.
const TIP3055: BipolarSpec = BipolarSpec {
    saturation: 3.0e-12,
    forward_beta: 45.0,
    reverse_beta: 3.0,
    early: 60.0,
};

const BC109C: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 500.0,
    reverse_beta: 4.0,
    early: 100.0,
};

/// The rail the card runs from: +16 V regulated to +8 V above a -16 V taken as
/// zero, so the blocks see about 24 V end to end.
pub const SUPPLY: f64 = 24.0;

/// Primary volts per secondary volt on the input transformer. Below one steps
/// up.
///
/// Not a turns ratio: the balanced-input drawing states what the X1 VTB9045
/// does in circuit, which is "amplifies the input signal 6dB (9dB with the Lo
/// Z switch in)". Six decibels is a voltage ratio of two, so 0.5. The Lo-Z
/// switch, which reconnects the primaries in parallel for another three
/// decibels, is not modelled.
pub const INPUT_RATIO: f64 = 0.5;

/// The two resistors the gain switch can hang on PRE 2's GAIN pin, and the
/// open circuit it presents everywhere else.
///
/// R61 and R62 off the gain drawing. Kept as named constants because they are
/// what the sweep below is sized against, and because the three stage gains in
/// the module docstring are arithmetic on them.
pub const R61: f64 = 330.0;
pub const R62: f64 = 120.0;

/// The variable part of the modelled GAIN leg, in series with R62.
///
/// The leg therefore runs from 10 k at the bottom of the control to R62 at the
/// top. Ten thousand rather than an open circuit because the ratio a control
/// can sweep evenly is limited: from a true open circuit the first two thirds
/// of the knob would move the stage by less than a decibel, and the last tenth
/// would be dead against the solver's clamp. Ten thousand costs 0.19 dB at the
/// quiet end and buys a control that moves all the way along.
const GAIN_SWEEP: f64 = 9_880.0;

/// The taper for that sweep.
///
/// A span below one is the log law running the other way: quickly at first and
/// slowly at the end, which is what a rheostat needs when the thing it is
/// fighting -- R71, at 1k8 -- is much smaller than the top of its own track.
/// Sized so the middle of the knob puts 331 R on the pin, which is the middle
/// of the range in decibels and, to within one per cent, R61 off the drawing.
const GAIN_SPAN: f64 = 4.761e-4;

/// Build the whole card as the plugin uses it: input transformer, PRE 1,
/// PRE 2, and the output trim.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out", true)
}

/// The same card brought out at a chosen node, and optionally without its
/// input transformer, so one block can be measured at a time.
pub fn tap(source: f64, load: f64, at: &str, with_iron: bool) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Neve 73P");
    net.supply("n", 100.0, SUPPLY).capacitor("n", "gnd", 470e-6);

    // --- balanced input: X1 VTB9045 ---------------------------------------
    let input = if with_iron {
        net.input("mic", source)
            .resistor("mic", "pri", 100.0)
            .core("pri", "gnd", CoreSpec::NICKEL)
            .transformer("pri", "gnd", "sec_xfmr", "gnd", INPUT_RATIO)
            // The secondary is referred to ground, and the card's input node
            // is not, so a blocking capacitor keeps the transformer from
            // deciding the input node's DC. Large enough that its corner is
            // far below anything audible.
            .capacitor("sec_xfmr", "in1", 10e-6)
            .capacitor("in1", "gnd", 180e-12); // C39
        "in1"
    } else {
        net.input("in1", source);
        "in1"
    };

    block(&mut net, Block::PRE1, input, "pre1_out");
    // At the 70 dB position the gain switch is a wire between the two cards,
    // so PRE 1's output capacitor C10 and PRE 2's input capacitor C32 meet at
    // one node with R3 and R64 on it.
    block(&mut net, Block::PRE2, "pre1_out", "pre2_out");

    // --- output trim: VR3 then VR2 ----------------------------------------
    // R67 is a bleeder to ground at the P2 test point, not a series resistor:
    // the drawing takes the trim off the same node. VR3 is a 5 k trimpot wired
    // as a rheostat for matching one card to another; there is no stated
    // setting for it, so it sits at mid travel and is marked as an estimate.
    net.resistor("pre2_out", "gnd", 51_000.0) // R67
        .resistor("pre2_out", "trim_top", 2_500.0) // VR3 at mid travel
        .pot("trim_top", "out", "gnd", 10_000.0, Taper::Audio, TRIM)
        .resistor("out", "gnd", load);

    net.build(at)
}

/// Which of the two amplifier blocks is being built. They are the same drawing
/// with different designators; only the feedback resistor and the gain leg
/// differ, and both of those are named here rather than duplicated below.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    /// R1 15 k around it and R4 91 R across R5, for 30 dB.
    PRE1,
    /// R63 18 k around it and the gain switch on its leg, for 18 to 28 dB.
    PRE2,
}

/// One three-transistor block, from `input` to `output`.
///
/// PRE 1 designators are in the comments; PRE 2's are the same parts under
/// different numbers (R1/R63, R2/R64, R3/R67, R5/R71 and so on).
fn block(net: &mut Netlist, which: Block, input: &str, output: &str) {
    let p = match which {
        Block::PRE1 => "a",
        Block::PRE2 => "b",
    };
    let node = |name: &str| format!("{p}_{name}");
    let (t1b, t1c, t1e) = (node("t1b"), node("t1c"), node("t1e"));
    let (t3c, t3e) = (node("t3c"), node("t3e"));
    let t2e = node("t2e");
    let (mid, emitter, shunt, leg) = (node("mid"), node("emitter"), node("shunt"), node("leg"));

    // The input node: R2 to ground, C9 into the base, and R1 back from the
    // shunt-feedback node. R1 lands here, on the pin, so a stiff source
    // absorbs its current and the block's voltage gain is the feedback
    // arithmetic above.
    let r1 = match which {
        Block::PRE1 => 15_000.0, // R1
        Block::PRE2 => 18_000.0, // R63
    };
    net.resistor(input, "gnd", 120_000.0) // R2
        .capacitor(input, &t1b, 22e-6) // C9
        .resistor(input, &shunt, r1) // R1
        .resistor(&shunt, "gnd", 10_000.0) // R8
        .capacitor(&shunt, &t3e, 22e-6); // C6

    // T1, the input transistor. R10 biases its base from the emitter network,
    // C5 is base to collector and C4 base to emitter.
    net.resistor(&t1b, &emitter, 68_000.0) // R10
        .capacitor(&t1b, &t1c, 100e-12) // C5
        .capacitor(&t1b, &t1e, 1_500e-12) // C4
        .bipolar(&t1c, &t1b, &t1e, BC109C);

    // T1's collector load is R14 and R11 in series from the rail, with C2
    // holding their junction at signal ground -- so the load is 47 k at audio
    // and 80 k at DC. C7 rolls the top off the same node.
    net.resistor("n", &mid, 33_000.0) // R14
        .capacitor(&mid, "gnd", 22e-6) // C2
        .resistor(&mid, &t1c, 47_000.0) // R11
        .capacitor(&t1c, "gnd", 680e-12); // C7

    // T3, the voltage stage, driven straight off T1's collector. Its emitter
    // sits on R6 above the R7/C1 network, which is also where T1's base bias
    // comes from.
    net.resistor("n", &t3c, 5_100.0) // R9
        .bipolar(&t3c, &t1c, &t3e, BC109C)
        .resistor(&t3e, &emitter, 470.0) // R6
        .resistor(&emitter, "gnd", 1_500.0) // R7
        .capacitor(&emitter, "gnd", 150e-6); // C1

    // T2, the output follower. Collector on the rail, base off T3's collector.
    net.bipolar("n", &t3c, &t2e, BC109C);

    // The feedback network, which is what sets the gain: R12 from the
    // follower's emitter to T1's emitter, R13 from there to ground, and C3
    // into the gain leg.
    net.resistor(&t2e, &t1e, 2_200.0) // R12
        .resistor(&t1e, "gnd", 390.0) // R13
        .capacitor(&t1e, &leg, 470e-6); // C3

    match which {
        // R5 with R4 across it, which is the whole of PRE 1's extra 12 dB.
        Block::PRE1 => {
            net.resistor(&leg, "gnd", 1_800.0) // R5
                .resistor(&leg, "gnd", 91.0); // R4
        }
        // R71, and the gain switch in parallel with it. See the module
        // docstring for what the sweep stands in for.
        Block::PRE2 => {
            net.resistor(&leg, "gnd", 1_800.0) // R71
                .pot(
                    &leg,
                    &leg,
                    "gain_sw",
                    GAIN_SWEEP,
                    Taper::ReverseLog { span: GAIN_SPAN },
                    GAIN,
                )
                .resistor("gain_sw", "gnd", R62); // R62
        }
    }

    // Out through C8 and C10, with R3 bleeding the coupled node to ground.
    net.capacitor(&t2e, "gnd", 1_000e-12) // C8
        .capacitor(&t2e, output, 22e-6) // C10
        .resistor(output, "gnd", 51_000.0); // R3

    // Where the DC solver should start looking. Hand-worked from the loop
    // above: T3 draws about 1.9 mA, which puts T1's emitter near 2.2 V and
    // the follower's emitter near 13.6 V on a 24 V rail.
    net.initial_voltage(&mid, 15.9);
    net.initial_voltage(&t1c, 4.4);
    net.initial_voltage(&t1e, 2.2);
    net.initial_voltage(&emitter, 2.9);
    net.initial_voltage(&t3e, 3.8);
    net.initial_voltage(&t3c, 14.3);
    net.initial_voltage(&t2e, 13.6);
}

// ---------------------------------------------------------------------------
// 7. OUTPUT
// ---------------------------------------------------------------------------

/// VR1, the bias trimpot. Not a front-panel control: it is set once, on the
/// bench, against the card's stated bias.
pub const BIAS_TRIM: usize = 0;

/// Where VR1 rests.
///
/// The assembly guide's calibration is 2.5 V across B1/B2 once the card has
/// been warm for a quarter of an hour, which is a measurement on a resistor
/// this model does not have. What it does have is the thing that measurement
/// is *for*: the trimpot sets T8's standing current, and the position here is
/// the one that puts the collector near the middle of its swing, which is what
/// "lowest distortion and highest headroom" means.
const BIAS_REST: f64 = 0.5;

/// The 73P's OUTPUT block, from `docs/schematics/neve_73p_output.png`.
///
/// The card's own words for it: "The OUTPUT section provides more voltage gain
/// and the output current needed to drive the next device in the signal chain.
/// The output is another three-transistor amplifier. T6 and T7 provide voltage
/// gain, while the big power transistor T8 provides the current. T8 and X2 form
/// a transformer-coupled amplifier. The VR1 trimpot sets the bias of T3 for the
/// lowest distortion and highest headroom. R39/C24 set the gain of the
/// amplifier. The OUTPUT block provides 18 dB of gain, with 4 dB of this
/// provided by the output transformer."
///
/// Three things about it are worth saying before the parts.
///
/// **The gain is set by an emitter, not by a collector.** R33 3.3 k carries the
/// output back to T6's emitter through C19, and what T6's emitter sits on is
/// R38 1.2 k for direct current with R39 1.5 k and C24 across it for signal.
/// So the closed-loop gain is `(R33 + Re) / Re` with `Re` the two in parallel,
/// 667 ohms -- about six times, or fifteen decibels, against the fourteen the
/// guide implies once the transformer's four are taken off eighteen.
///
/// **The bias is a servo.** T6's base has no divider on it at all: R34 56 k
/// runs from the base to VR1's wiper, and VR1 sits in T8's emitter. If T8
/// draws more, the wiper rises, T6's base rises, T6 pulls its collector down,
/// T7 and T8 follow it down and T8 draws less. One loop sets the operating
/// point of all three devices, which is why there is a trimpot rather than a
/// resistor.
///
/// **T8's collector load is the transformer.** There is no collector resistor:
/// the primary goes from the collector to the rail, so at direct current the
/// collector sits at the rail and at signal it works into the reflected load.
/// That is where the headroom comes from, and it is why this block can drive a
/// line where the PRE blocks cannot.
///
/// What is not modelled: the polarity relay (U2 and SW1, which reverse the
/// output and are not in the audio path when they are not switching), and any
/// nonlinearity in X2. The guide gives the transformer's gain and nothing
/// about where its iron runs out, and a knee invented here would be an
/// invented sound. The plugin's own output transformer section is a separate
/// control and stands for that.
pub fn output(source: f64, load: f64) -> Result<Circuit, Fault> {
    output_tap(source, load, "spk")
}

/// The same block brought out at a chosen node, for measuring one stage at a
/// time.
pub fn output_tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Neve 73P output");
    // Ten ohms, not the hundred the PRE blocks feed through. This block draws
    // nearly forty milliamps where they draw a fraction of one, and a hundred
    // ohms in front of it costs four volts of the rail -- which the card does
    // not, because the OUTPUT block sits next to the reservoir rather than
    // behind the decoupling the small-signal stages get.
    net.supply("n", 10.0, SUPPLY).capacitor("n", "gnd", 470e-6);
    net.rest(BIAS_TRIM, BIAS_REST);

    // --- T6, the voltage stage -------------------------------------------
    // C18 couples in, R32 is the base stopper, and R34 brings the bias back
    // from VR1's wiper. C22 is base to collector and C23 base to emitter,
    // which is where this stage's top end is decided.
    net.input("in", source)
        .capacitor("in", "c18", 22e-6) // C18
        .resistor("c18", "b6", 2_200.0) // R32
        .resistor("b6", "wiper", 56_000.0) // R34
        .capacitor("b6", "e6", 5_000e-12) // C23
        .capacitor("b6", "c6", 220e-12) // C22
        .supply("c6", 68_000.0, SUPPLY) // R40
        .bipolar("c6", "b6", "e6", BC109C);

    // The emitter network, which is the gain. R38 alone at direct current;
    // R39 in parallel with it once C24 is a short, which is everywhere in the
    // audio band.
    net.resistor("e6", "gnd", 1_200.0) // R38
        .resistor("e6", "r39", 1_500.0) // R39
        .capacitor("r39", "gnd", 100e-6); // C24

    // The feedback leg: R33 from the output back to that emitter, with C21
    // across it taking the top off the loop, and C19 blocking the direct
    // voltage the output sits at.
    net.resistor("fb", "e6", 3_300.0) // R33
        .capacitor("fb", "e6", 330e-12) // C21
        .capacitor("fb", "out", 100e-6); // C19

    // --- T7, driving T8 ---------------------------------------------------
    net.resistor("e7", "gnd", 18_000.0) // R36
        .bipolar("out", "c6", "e7", BC109C);

    // --- T8, the current ---------------------------------------------------
    // Its collector is the output node -- the transformer's primary is its
    // load -- and its emitter carries the bias chain that feeds R34.
    net.resistor("e8", "gnd", 47.0) // R41
        .capacitor("n", "e8", 100e-6) // C25
        .pot("e8", "wiper", "vr1", 5_000.0, Taper::Linear, BIAS_TRIM) // VR1
        .resistor("vr1", "gnd", 1_200.0) // R35
        .bipolar("out", "e7", "e8", TIP3055);

    // C20 and R37 hang off the output node: a hundred microfarads in series
    // with 33 k, which at audio is 33 k of bleed to ground and at direct
    // voltage is nothing.
    net.capacitor("out", "c20", 100e-6) // C20
        .resistor("c20", "gnd", 33_000.0); // R37

    // --- X2, the VTB1148 ----------------------------------------------------
    // The primary runs from the collector to the rail, which is signal ground,
    // so it is the collector load.
    //
    // The ratio sits across the winding and the copper in series with it,
    // which is the way round it has to be for a single-ended stage. The other
    // way -- copper outside, ratio spanning the collector and the rail -- puts
    // the copper's *direct* voltage drop through the ideal element, and an
    // ideal transformer passes direct voltage where a real one passes none.
    // Measured, that put 1.7 V of offset on the secondary, because T8's
    // thirty-seven milliamps through thirty ohms of primary is a volt.
    //
    // The push-pull stages in `power.rs` get away with the other arrangement
    // because their two halves' direct currents oppose and the drop cancels.
    // Nothing cancels here.
    //
    // What that costs is the constraint the guitar amplifiers had to avoid:
    // the inductor and the ideal element now span the same pair of nodes and
    // state the same thing at rest. It survives because the inductor's
    // direct-current stamp is a tenth of a milliohm rather than a millionth of
    // one -- see `time.rs` -- so the two constraints are merely close rather
    // than identical, and the offset it leaves on the secondary is six
    // microvolts.
    net.resistor("out", "w", PRIMARY_COPPER)
        .inductor("w", "n", PRIMARY_HENRY)
        .transformer("w", "n", "sec", "gnd", OUTPUT_RATIO)
        .inductor("sec", "spk", LEAKAGE)
        .resistor("spk", "gnd", 1_500.0) // R43
        .capacitor("spk", "gnd", 0.01e-6) // C26
        .resistor("spk", "gnd", load);

    net.build(at)
}

/// Primary volts per secondary volt.
///
/// The guide states the transformer's contribution as four decibels of the
/// block's eighteen, and four decibels of voltage gain is a step-up of 1.585 --
/// so the primary carries `1 / 1.585` of a secondary volt. Stated from the
/// gain because that is what the guide gives; the winding count is not
/// published.
const OUTPUT_RATIO: f64 = 0.6310;

/// The primary's copper, its magnetising inductance and the leakage in the
/// secondary. None of the three is published, so they are set from what they
/// do: the inductance puts the low corner around five hertz against the
/// reflected load, and the leakage puts the high one around forty kilohertz.
/// An output transformer for a line stage is built to be flat across the band
/// and these are what "flat across the band" costs.
const PRIMARY_COPPER: f64 = 30.0;
const PRIMARY_HENRY: f64 = 8.0;
const LEAKAGE: f64 = 1.0e-3;
