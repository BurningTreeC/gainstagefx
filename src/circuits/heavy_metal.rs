//! The Heavy Metal: two transistor stages, a hard clipper, a coring gate and
//! three gyrators.
//!
//! Engineering reference (developer documentation, not panel text): Boss HM-2
//! Heavy Metal, the Japanese original, built from Boss's own service-notes
//! drawing. See `docs/models/heavy_metal.md`.
//!
//! Three things here exist nowhere else in the plugin.
//!
//! The first is the **coring gate**: D6 and D7 sit *in series with the signal*
//! rather than across it, back to back, so nothing gets through until the
//! signal is bigger than a germanium diode's forward voltage. Small signals are
//! held back entirely and the pedal stops rather than fades. That is the gated,
//! chainsaw decay it is known for, and it is a primitive noise gate made of two
//! diodes.
//!
//! The second is the **gyrator stack**. A gyrator is an active circuit that
//! behaves like an inductor: an op-amp (here a transistor stage) with a
//! capacitor arranged so the input current lags, which gives a resonant
//! peak without winding anything. Three of them, at roughly 80 Hz, 900 Hz and
//! 2 kHz, are boosted or cut by the two Colour Mix controls -- so this is a
//! *boost and cut* tone section rather than the passive losses every other
//! pedal here has.
//!
//! The third is that the gain stage clips **asymmetrically**: D3 alone one way
//! against the D4/D5 pair the other, so the two halves of the wave are squared
//! off at different heights and the result is full of even harmonics.

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, JfetSpec, Netlist, Taper};

/// VR4, the Dist control, 250 k (250KD on the drawing), across the second gain
/// stage into the clipping amplifier.
pub const DIST: usize = 0;
/// VR2, Colour Mix Low, 10 k.
pub const LOW: usize = 1;
/// VR3, Colour Mix High, 10 k.
pub const HIGH: usize = 2;
/// VR1, the Level control, 10 k audio.
pub const LEVEL: usize = 3;

/// Where the level control rests: **unity through the pedal**, which is what
/// the panel's Level knob means at noon. See `rodent::VOLUME_REST`.
///
/// Measured **broadband** (`examples/pedallevel.rs`) rather than on one tone.
/// For this pedal the two are two decibels apart -- its Colour Mix shapes hard
/// enough that its level at 220 Hz is not how loud it is -- and the same is true
/// of the Metal Zone. At this position it sits 2 dB above no pedal at all.
pub const LEVEL_REST: f64 = 0.60;

/// The germanium pair of the coring gate, D6 and D7.
///
/// ESTIMATED to the **0.3 V** forward drop the published analysis measures on
/// these two parts, rather than the catalogue's generic germanium, which is
/// softer than that. Where the gate opens *is* the sound of this pedal, so it
/// carries its own constants, exactly as the Yellow Dist's 1N270s do.
const CORING: DiodeSpec = DiodeSpec {
    saturation: 3.3e-9,
    emission: 1.2,
};

/// The JFET input buffer, Q1. APPROXIMATED: the scan carries no part numbers,
/// and the Boss standard of the period is a 2SK30A-class small-signal JFET.
const BUFFER: JfetSpec = JfetSpec::J201;

/// The NPNs. APPROXIMATED the same way, to the high-beta Japanese small-signal
/// part the catalogue already carries.
const NPN: BipolarSpec = BipolarSpec::NPN_2SC3378;

/// The op-amp rail, either side of the 4.5 V bias.
const SWING: f64 = 3.6;

/// The three gyrators, as the inductance each one stands for.
///
/// A gyrator is an op-amp wired so that the current into it lags the voltage:
/// it *is* an inductor, made without winding one, which is how a pedal gets a
/// resonant tone control into a box this size. The drawing has three, all the
/// same shape -- a resistor from the node to the follower's input, a capacitor
/// from there to ground, and the follower's output back to the node through a
/// second resistor -- and what it simulates is **L = R1 x R2 x C**.
///
/// So they are built here as the inductors they are, which is what the Mark
/// IIC+'s graphic equaliser already does with its five real ones
/// (`markiic::BANDS`). What that leaves out is the op-amp's own limits: a real
/// gyrator can run out of swing and an inductor cannot. At the levels this
/// stack sees, after the clipper and the Dist control, it does not.
///
/// Each entry: the equivalent inductance, its series resistance, and the
/// capacitor in front of it, which together decide where the band sits.
///
/// | | from the drawing | L | with | resonance |
/// |---|---|---|---|---|
/// | low | R51 100k, R47 330, C30 .068 | 2.244 H | C35 1.5 uF | **87 Hz** |
/// | mid | R43 82k, R41 330, C27 .0068 | 0.184 H | C28 .15 uF | **958 Hz** |
/// | high | R44 100k, R45 330, C26 .0047 | 0.155 H | C29 .1 uF | **1278 Hz** |
///
/// Which is what the published analysis hears: the Low control works "at around
/// 80Hz", and the High one "between about 900Hz and 1.3kHz" -- that is these
/// two upper bands, both on the one knob.
const LOW_BAND: (f64, f64, f64) = (2.244, 330.0, 1.5e-6);
const MID_BAND: (f64, f64, f64) = (0.184, 330.0, 0.15e-6);
const HIGH_BAND: (f64, f64, f64) = (0.1551, 330.0, 0.1e-6);

/// One band of the Colour Mix: the series capacitor, the gyrator's inductance
/// and its resistance, hung off a wiper.
fn band(net: &mut Netlist, wiper: &str, prefix: &str, (l, r, c): (f64, f64, f64)) {
    let a = format!("{prefix}_a");
    let b = format!("{prefix}_b");
    net.capacitor(wiper, &a, c)
        .inductor(&a, &b, l)
        .resistor(&b, "gnd", r);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Heavy Metal");

    // --- supply and bias ------------------------------------------------------------
    // R5 and R6 10 k each with C25 make the 4.5 V the whole pedal sits on.
    net.supply("v9", 47.0, 9.0)
        .capacitor("v9", "gnd", 100e-6) // C2
        .resistor("v9", "vref", 10_000.0) // R6
        .resistor("vref", "gnd", 10_000.0) // R5
        .capacitor("vref", "gnd", 47e-6) // C25
        // R10 and C5 decouple the two gain stages' rail from everything else.
        // Left out at first, and the pedal then had a floor forty decibels down
        // that did not depend on the signal at all: the gain stages were pulling
        // the supply about and the output followers were passing it on.
        .resistor("v9", "v9a", 1_000.0) // R10
        .capacitor("v9a", "gnd", 47e-6); // C5

    // --- input buffer, Q1 -------------------------------------------------------------
    // R1 10 k and C1 .047 into the gate, R8 1 M holding it at the bias point.
    net.input("in", source)
        .resistor("in", "c1", 10_000.0) // R1
        .capacitor("c1", "g1", 0.047e-6) // C1
        .resistor("g1", "vref", 1_000_000.0) // R8
        .jfet("v9", "g1", "s1", BUFFER)
        .resistor("s1", "gnd", 10_000.0); // R7, the source resistor

    // --- first gain stage, Q6 ----------------------------------------------------------
    // C4 1 uF into Q4's follower, then C6 .047 into the stage proper.
    //
    // Q6 is a **shunt-feedback** common-emitter stage, which is the Boss house
    // arrangement of the period and the same one the DS-1's booster uses: R16
    // 470 k from collector back to base is the whole DC bias -- the base has no
    // other path -- and the gain is set by the collector load against the
    // emitter resistance, R19 22 k over R18 22, pulled down by that feedback.
    // C10 100 p across it keeps the top under control.
    net.capacitor("s1", "b4", 1e-6) // C4
        .resistor("b4", "vref", 1_000_000.0) // R11
        .bipolar("v9", "b4", "e4", NPN)
        .resistor("e4", "gnd", 100_000.0) // R21
        .capacitor("e4", "b6", 0.047e-6) // C6
        .bipolar("c6", "b6", "e6", NPN)
        .resistor("c6", "b6", 470_000.0) // R16, the shunt feedback and the bias
        .resistor("v9a", "c6", 22_000.0) // R19
        .resistor("e6", "gnd", 22.0) // R18
        .capacitor("c6", "b6", 100e-12); // C10

    // --- second gain stage, Q7 ---------------------------------------------------------
    // R26 100 k and R28 120 set this one; C31 10 uF over R49 150 is its emitter
    // bypass, which is where most of the gain comes from.
    net.capacitor("c6", "b7", 0.047e-6) // C11
        .bipolar("c7", "b7", "e7", NPN)
        .resistor("c7", "b7", 470_000.0) // R29, shunt feedback again
        .resistor("v9a", "c7", 100_000.0) // R26
        .resistor("e7", "e7b", 120.0) // R28
        .resistor("e7b", "gnd", 150.0) // R49
        .capacitor("e7b", "gnd", 10e-6) // C31
        .capacitor("c7", "b7", 100e-12); // C14

    // --- Dist ---------------------------------------------------------------------------
    // VR4 250 k, across the second gain stage's output with its wiper feeding the
    // clipping amplifier. The drawing's own pin routing puts it here -- its three
    // pins all land in the gain-stage region, between Q6/Q7 and op-amp 1b -- and
    // that is what makes it a *distortion* control rather than a volume: it
    // decides how hard the clipper is driven into its diodes, and the clipping
    // then holds the level roughly where it was.
    //
    // Built after the clipper instead, it only attenuated what had already been
    // squared off, so the knob changed loudness and not much else: twenty
    // decibels down at noon with the same distortion.
    //
    // The taper letter (250KD) is Boss's own and the law behind it was not
    // found; a linear track is used, which sweeps the drive evenly. ESTIMATED.
    // Coupled in, so the track does not drag the collector's bias toward
    // ground: a 250 k path straight off Q7's collector costs the stage forty
    // decibels and most of its operating point.
    // Coupled in, and its cold end on the 4.5 V bias rather than ground: the
    // wiper feeds the clipping amplifier's inverting input through R25, so its
    // direct voltage has to be the one that amplifier sits at. Taken to ground
    // instead, R25 works against R20 and drives the op-amp into its rail, where
    // it stays -- the pedal then passes a millivolt of leakage and nothing else.
    net.capacitor("c7", "dist_top", 0.047e-6)
        .pot("dist_top", "dist", "vref", 250_000.0, Taper::Linear, DIST);

    // --- the clipper, op-amp 1b -------------------------------------------------------
    // R20 220 k round the loop with C9 100 p, and the diodes across it: D3 on its
    // own one way, D4 and D5 in series the other. That asymmetry is the point.
    net.resistor("dist", "u1b_m", 68_000.0) // R25
        .capacitor("u1b_m", "gnd", 0.047e-6) // C22
        .opamp_biased("u1b", "vref", "u1b_m", "vref", SWING)
        .resistor("u1b", "u1b_m", 220_000.0) // R20
        .capacitor("u1b", "u1b_m", 100e-12) // C9
        .diode("u1b_m", "u1b", DiodeSpec::SILICON) // D3
        .diode("u1b", "d45", DiodeSpec::SILICON) // D4
        .diode("d45", "u1b_m", DiodeSpec::SILICON); // D5

    // --- the coring gate ---------------------------------------------------------------
    // D6 and D7 back to back *in series with the signal*. Nothing crosses them
    // until it is bigger than a germanium diode, so the tail of a note is cut
    // off rather than faded out.
    net.capacitor("u1b", "gate_in", 1e-6) // C12
        .resistor("gate_in", "vref", 10_000.0) // R23
        .diode("gate_in", "gate_out", CORING) // D6
        .diode("gate_out", "gate_in", CORING) // D7
        // The leakage a real diode has. Two diodes in series with the signal
        // leave the node between them at a very high impedance whenever both
        // are off, which is most of the time and is what the gate is *for*; the
        // solver then needs half again as many Newton passes to place it. A
        // real germanium pair leaks microamps, and saying so conditions the
        // matrix without changing what the gate does.
        .resistor("gate_in", "gate_out", 10_000_000.0)
        .resistor("gate_out", "vref", 10_000.0); // R30

    // --- recovery, op-amp 1a -----------------------------------------------------------
    net.capacitor("gate_out", "u1a_m", 1e-6) // C15
        .opamp_biased("u1a", "vref", "u1a_m", "vref", SWING)
        .resistor("u1a", "u1a_m", 68_000.0) // R24
        .capacitor("u1a", "u1a_m", 0.001e-6) // C16
        .diode("u1a_m", "u1a", DiodeSpec::SILICON) // D8
        .diode("u1a", "u1a_m", DiodeSpec::SILICON) // D9
        .resistor("u1a", "eqfeed", 47_000.0); // R42

    // --- the Colour Mix -------------------------------------------------------------------
    // A feedback equaliser, the same shape as the Mark IIC+'s graphic: op-amp 3a
    // with R52 3.3 k into one input and R54 3.3 k of feedback into the other, and
    // each control's track running between those two nodes with its resonant leg
    // on the wiper. Toward the input side the leg shunts the signal and cuts its
    // band; toward the feedback side it shunts the loop and boosts it; centred,
    // the two cancel and the control is flat.
    //
    // That is why this is a *boost and cut* tone section rather than the passive
    // losses the other pedals here have -- and why both knobs on full is a real
    // setting rather than simply the loudest one.
    // The Dist track is 250 k and the equaliser's input is 3.3 k, so the wiper
    // is buffered into it -- as the Mark IIC+'s graphic buffers its own input
    // with Q1 for the same reason. Without it the control would lose sixteen
    // decibels into the stack and the tone knobs would be working on nothing.
    // Ground-referenced from here on, with the wiper coupled in: the same
    // construction the Rodent's op-amp needed. Riding the 4.5 V bias through
    // the equaliser left the solver with a follower that would not follow
    // below about a tenth of a volt.
    net.capacitor("eqfeed", "eqac", 1e-6)
        .resistor("eqac", "gnd", 1_000_000.0)
        .opamp("eqin", "eqac", "eqin", SWING)
        .resistor("eqin", "cut", 3_300.0) // R52
        .opamp("u3a", "cut", "boost", SWING)
        .resistor("u3a", "boost", 3_300.0) // R54
        .capacitor("u3a", "boost", 470e-12); // C33

    // VR2, Colour Mix Low: the 87 Hz band.
    net.pot("boost", "w_low", "cut", 10_000.0, Taper::Symmetric { span: 150.0 }, LOW);
    band(&mut net, "w_low", "bl", LOW_BAND);

    // VR3, Colour Mix High: the two upper bands, both on the one knob.
    net.pot("boost", "w_high", "cut", 10_000.0, Taper::Symmetric { span: 150.0 }, HIGH);
    band(&mut net, "w_high", "bm", MID_BAND);
    band(&mut net, "w_high", "bh", HIGH_BAND);

    net.capacitor("u3a", "lvl_top", 10e-6) // C34
        .resistor("lvl_top", "gnd", 10_000.0); // R53

    // --- Level and the output buffer ----------------------------------------------------
    net.rest(LEVEL, LEVEL_REST)
        .pot("lvl_top", "lvl", "gnd", 10_000.0, Taper::Audio, LEVEL) // VR1
        // C32 straight on to the output buffer. Q10 is on the drawing between
        // them, but it sits in the switching corner with D12 and R60 -- it is
        // one of the two JFETs that make this pedal's bypass, like Q1 at the
        // input, not a stage. Built as an amplifier it cost a nonlinear device
        // and two nodes for nothing.
        .capacitor("lvl", "q10", 1e-6) // C32
        .resistor("q10", "vref", 1_000_000.0) // R60
        .capacitor("q10", "b3", 0.047e-6) // C7
        .resistor("b3", "vref", 1_000_000.0) // R48
        .bipolar("v9", "b3", "e3", NPN)
        .resistor("e3", "gnd", 10_000.0) // R13
        .capacitor("e3", "out", 1e-6) // C3
        .resistor("out", "gnd", 10_000.0) // R3
        .resistor("out", "gnd", load);

    net.build(at)
}
