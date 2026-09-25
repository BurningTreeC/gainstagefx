//! The Orange Dist: a transistor that clips first, an op-amp and a diode pair
//! that clip what is left, and a passive scoop after them.
//!
//! Engineering reference (developer documentation, not panel text): Boss DS-1
//! Distortion, the original **TA7136P** version built until November 1994,
//! from Boss's own service notes ("DS-1 (DS-1A)", second edition, December
//! 1994, the "Circuit Diagram (DS-1)" page) and Toshiba's TA7136AP data sheet.
//! See `docs/models/orange_dist.md`.
//!
//! What sets it apart from every other distortion in the plugin is the order
//! of things. **Q2 is a 35-40 dB transistor booster that clips on its own**,
//! asymmetrically, before the op-amp sees anything; the op-amp then adds up to
//! 27 dB more and the 1S2473 pair after it squares off whatever is left. And
//! the booster is a high-pass: its shunt feedback puts about 3.5 k at its base,
//! so C3's 47 nF cuts the bass *before* the clipping, which is the DS-1's
//! bright, tight distortion and why it does not turn to mud on a low chord.
//!
//! The DS-1A that replaced it (a BA728N, with C5 68 nF, C7 100 pF, C8 .47 uF
//! and a buffer in front of the gain stage) is a different pedal in those
//! parts and is not what this is.

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, Netlist, Taper};

/// VR1, the Dist control, 100 k linear ("100KB") across the gain stage's
/// feedback: the wiper is the feedback tap, so turning it up moves resistance
/// from the ground leg into the feedback.
pub const DIST: usize = 0;
/// VR3, the Tone control, 20 k linear, crossfading the low-pass and the
/// high-pass arms.
pub const TONE: usize = 1;
/// VR2, the Level control, 100 k linear.
pub const LEVEL: usize = 2;

/// Where the level control rests: **unity through the pedal** with Dist and
/// Tone centred, which is what the panel's Level knob means at noon.
///
/// Measured by `examples/ds1level.rs` on a guitar's low chord: 0.348 of the
/// 100 k linear track is 0 dB broadband. It is low because the pedal has
/// a great deal of gain in front of a linear level pot; at this rest it runs
/// from +10 dB (Dist up, Tone down) to -6 dB (Dist down, Tone up).
pub const LEVEL_REST: f64 = 0.348;

/// The TA7136P's gain-bandwidth with the DS-1's 150 pF compensation. ESTIMATED.
///
/// Toshiba publishes the open-loop curve with pin 1 left open, and it falls
/// through 84 dB at 10 kHz and 72 dB at 30 kHz: a gain-bandwidth of about
/// 140 MHz against the chip's own internal capacitance. Boss hangs C6, 150 pF,
/// from pin 1 to the output -- a Miller capacitor across the second stage --
/// which divides that by `(C6 + C_int) / C_int`. With an internal 1-3 pF the
/// result is 1-3 MHz; this is the middle. Nothing in this pedal is sensitive
/// to it: at the full 27 dB of the gain stage the loop still reaches 90 kHz,
/// and C7 closes it at 6.4 kHz long before that.
pub const GBW_HZ: f64 = 2.0e6;
/// And its slew rate, from the same estimate: an input pair at the current
/// that gain-bandwidth implies, charging C6. ESTIMATED.
pub const SLEW: f64 = 0.6e6;
/// Open-loop gain, 92 dB typical on Toshiba's sheet. DOCUMENTED.
pub const OPEN_LOOP: f64 = 39_800.0;
/// What the output swings either way from the 4.5 V bias point. DOCUMENTED:
/// Toshiba's maximum-output curve reads 2.8 V rms at 0.1 % distortion on a
/// +/-4.5 V supply with the bias resistor chosen for 300 uA, as R12's 27 k
/// does here -- four volts either way.
pub const SWING: f64 = 4.0;
/// The bias point the pedal runs at: half the battery, from R24 and R25.
pub const BIAS: f64 = 4.5;

/// The internal integrating capacitor of the op-amp model. A scaling choice:
/// what is modelled is the gain-bandwidth and the slew rate. See
/// `circuits::rodent::lm308`, which is the same construction.
const INTEGRATOR: f64 = 1e-9;

/// 2SC2240GR (Q1, Q2) and 2SC732TM GR (Q3), Toshiba's GR-rank small-signal
/// NPNs. The catalogue's 2SC1815 GR is the same family and rank. APPROXIMATED.
const NPN: BipolarSpec = BipolarSpec::NPN_2SC1815_GR;

/// D4 and D5, "1S2473 or 1S1588": silicon switching diodes of the 1N4148
/// class, which is what the catalogue's silicon diode is. APPROXIMATED.
const CLIPPER: DiodeSpec = DiodeSpec::SILICON;

/// Q6 and Q7, the 2SK30ATM switches of Boss's electronic bypass, as the
/// channel resistance they leave in the signal path when the pedal is on.
///
/// ESTIMATED. A JFET at zero gate bias and no drain voltage is a resistor of
/// `1 / gm0`; Toshiba's sheet guarantees |Yfs| of at least 1.2 mS across the
/// part's ranks, and the GR and Y ranks here, at 2.6-6.5 and 1.2-3.0 mA of
/// Idss, sit well above that minimum. A few mS is 300 ohms. Q7 is in series
/// with R18's 10 k and does not matter; Q6 is in series with C3 into the
/// booster's low input impedance, and measured by `examples/ds1_op.rs` with
/// 100 and 800 ohms in its place, the booster moves by under a decibel below
/// 3.3 kHz and by three at 10 kHz.
const SWITCH_ON: f64 = 300.0;

/// A TA7136P between `plus`, `minus` and `out` on the pedal's battery.
///
/// Pin 2 is its non-inverting input and pin 3 its inverting one, which is
/// the way round Boss wires it: the signal into pin 2, the feedback into
/// pin 3. The compensation (C6, C24) and the bias resistor on pin 5 (R12) are
/// the gain-bandwidth and the swing above rather than parts: they are inside
/// what the model is.
///
/// Built as the Rodent's LM308 is -- a saturating transconductance into an
/// integrating capacitor, a resistance across it for the open-loop gain, two
/// diodes against fixed levels for the swing, and an ideal follower -- with
/// everything referenced to ground, for the reason `rodent::lm308` gives.
pub fn ta7136(net: &mut Netlist, prefix: &str, plus: &str, minus: &str, out: &str) {
    let stage = format!("{prefix}_int");
    let high = format!("{prefix}_high");
    let low = format!("{prefix}_low");
    let gm = std::f64::consts::TAU * GBW_HZ * INTEGRATOR;
    let limit = SLEW * INTEGRATOR;
    // A silicon clamp carrying the whole slewing current drops about 0.55 V,
    // which puts the output limits at the bias point plus and minus `SWING`.
    let drop = 0.55;
    net.transconductor(plus, minus, &stage, "gnd", gm, limit)
        .capacitor(&stage, "gnd", INTEGRATOR)
        .resistor(&stage, "gnd", OPEN_LOOP / gm)
        .supply(&high, 1.0, BIAS + SWING - drop)
        .supply(&low, 1.0, BIAS - SWING + drop)
        .diode(&stage, &high, DiodeSpec::SILICON)
        .diode(&low, &stage, DiodeSpec::SILICON)
        .opamp(out, &stage, out, 1_000.0);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Orange Dist");

    // --- supply and bias -----------------------------------------------------------
    // The battery through the house 47 ohm, C23 100 uF across it. R24 and R25,
    // 10 k each with C15 47 uF, make the 4.5 V everything is biased from. D1
    // is a reverse-polarity clamp and carries nothing.
    net.supply("v9", 47.0, 9.0)
        .capacitor("v9", "gnd", 100e-6) // C23
        .resistor("v9", "vref", 10_000.0) // R24
        .resistor("vref", "gnd", 10_000.0) // R25
        .capacitor("vref", "gnd", 47e-6); // C15

    // --- input buffer, Q1 ------------------------------------------------------------
    // R1 1 k and C1 47 nF into the base, R2 470 k to the bias point: the
    // pedal's 470 k input impedance. An emitter follower with R3 10 k.
    net.input("in", source)
        .resistor("in", "c1", 1_000.0) // R1
        .capacitor("c1", "b1", 0.047e-6) // C1
        .resistor("b1", "vref", 470_000.0) // R2
        .bipolar("v9", "b1", "e1", NPN) // Q1, 2SC2240GR
        .resistor("e1", "gnd", 10_000.0); // R3

    // --- the transistor booster, Q2 --------------------------------------------------
    // C2 .47 uF with R4 100 k to the bias point, then Q6 -- the bypass switch,
    // on -- with R5 1 M holding its far side at the same point. C3 47 nF into
    // Q2's base: a common emitter with R8 10 k, R9 22 ohm, and R7 470 k with C4
    // 250 pF from collector to base. R6 100 k is the base's only other DC
    // path, and it goes to ground.
    //
    // The shunt feedback is what makes this stage what it is: R7 divided by
    // the stage's own gain is about 3.5 k at the base, so C3 is a high-pass
    // near a kilohertz rather than the 34 Hz it makes with R6 alone.
    net.capacitor("e1", "a", 0.47e-6) // C2
        .resistor("a", "vref", 100_000.0) // R4
        .resistor("a", "sw", SWITCH_ON) // Q6, 2SK30ATM GR
        .resistor("sw", "vref", 1_000_000.0) // R5
        .capacitor("sw", "b2", 0.047e-6) // C3
        .resistor("b2", "gnd", 100_000.0) // R6
        .bipolar("c2", "b2", "e2", NPN) // Q2, 2SC2240GR
        .resistor("v9", "c2", 10_000.0) // R8
        .resistor("e2", "gnd", 22.0) // R9
        .resistor("c2", "b2", 470_000.0) // R7
        .capacitor("c2", "b2", 250e-12); // C4

    // --- the gain stage, IC --------------------------------------------------------------
    // C5 .47 uF with R10 100 k to the bias point into pin 2. The feedback is
    // VR1's wiper, through R11 100 k into pin 3; VR1 runs from the output
    // (lug 1) to R13 4k7 and C8 1 uF to ground (lug 3), and C7 250 pF goes from
    // the output to the wiper. Dist up moves the wiper towards lug 3: 1 + 100 k
    // over R13, 27 dB, with its low corner at 34 Hz, against unity at the
    // bottom. C7 closes it at 6.4 kHz with the control up.
    net.capacitor("c2", "plus", 0.47e-6) // C5
        .resistor("plus", "vref", 100_000.0); // R10
    ta7136(&mut net, "ic", "plus", "minus", "amp");
    net.resistor("fb", "minus", 100_000.0) // R11
        .capacitor("amp", "fb", 250e-12) // C7
        // `pot(a, wiper, b)` puts `R f(p)` between wiper and `b`: `b` is lug 1
        // at the output, so the control up puts the whole track in feedback.
        .pot("r13", "fb", "amp", 100_000.0, Taper::Linear, DIST) // VR1
        .resistor("r13", "c8", 4_700.0) // R13
        .capacitor("c8", "gnd", 1e-6); // C8

    // --- the clipper -----------------------------------------------------------------------
    // R14 2.2 k and C9 .47 uF into D4 and D5, back to back to the bias point,
    // with C10 .01 uF across them.
    net.resistor("amp", "r14", 2_200.0) // R14
        .capacitor("r14", "clip", 0.47e-6) // C9
        .diode("clip", "vref", CLIPPER) // D4
        .diode("vref", "clip", CLIPPER) // D5
        .capacitor("clip", "vref", 0.01e-6); // C10

    // --- tone -----------------------------------------------------------------------------
    // The low-pass arm: R16 6.8 k with C12 .1 uF, 234 Hz, to VR3's lug 1. The
    // high-pass arm: C11 22 nF and R15 2.2 k into R17 6.8 k, 800 Hz, to lug 3.
    // R15 is on both of Boss's drawings; ElectroSmash's leaves it out, which
    // is where its 1,063 Hz comes from. Tone up turns towards lug 3.
    net.resistor("clip", "lp", 6_800.0) // R16
        .capacitor("lp", "vref", 0.1e-6) // C12
        .capacitor("clip", "c11", 0.022e-6) // C11
        .resistor("c11", "hp", 2_200.0) // R15
        .resistor("hp", "vref", 6_800.0) // R17
        .pot("hp", "tone", "lp", 20_000.0, Taper::Linear, TONE); // VR3

    // --- level and the output buffer -----------------------------------------------------
    // VR2 100 k, lug 1 to the bias point. R18 10 k and Q7, the output switch,
    // to C13 47 nF, with R19 1 M holding the switch's side at the bias point.
    // Q3 an emitter follower on R20 1 M, R21 10 k, then R22 1 k, C14 1 uF and
    // R23 100 k to the jack.
    net.rest(LEVEL, LEVEL_REST)
        .pot("tone", "level", "vref", 100_000.0, Taper::Linear, LEVEL) // VR2
        .resistor("level", "r18", 10_000.0) // R18
        .resistor("r18", "q7", SWITCH_ON) // Q7, 2SK30ATM Y
        .resistor("q7", "vref", 1_000_000.0) // R19
        .capacitor("q7", "b3", 0.047e-6) // C13
        .resistor("b3", "vref", 1_000_000.0) // R20
        .bipolar("v9", "b3", "e3", NPN) // Q3, 2SC732TM GR
        .resistor("e3", "gnd", 10_000.0) // R21
        .resistor("e3", "c14", 1_000.0) // R22
        .capacitor("c14", "out", 1e-6) // C14
        .resistor("out", "gnd", 100_000.0) // R23
        .resistor("out", "gnd", load);

    net.build(at)
}
