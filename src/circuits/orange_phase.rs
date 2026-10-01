//! The Orange Phase: four JFET all-pass stages, a transistor mixer, and the
//! oscillator that sweeps them.
//!
//! Engineering reference (developer documentation, not panel text): the MXR
//! Phase 90, script logo (1974-77), from ElectroSmash's traced schematic and
//! parts list, with the block logo's R28 on a switch. See
//! `docs/models/orange_phase.md`.
//!
//! ```text
//! in -- R3 / C5 -- U1a follower -- four stages, each 10 k in, 10 k round,
//!    47 nF to +in and 24 k from there to the bias with a 2N5952 across it
//!    -- Q1 2N4125 sums the dry (R16) and the shifted (R8), R7 across it
//!    -- C2 / R2 -- out
//! LFO: U1b a Schmitt (R19 / R21), C10 15 uF charged from its output through
//!      SPEED, and C10 into -in through R24 with C7 back from the output; the
//!      ramp on C10 reaches the gates through R22 3.9 M, against the bias
//!      trimmer's R20 1 M
//! ```
//!
//! **Referenced to the zener.** Every signal stage rides the 5.1 V of D2, and
//! an op-amp in this solver that follows a bias point can stop following below
//! a tenth of a volt (`CLAUDE.md`). So the zener is this netlist's ground: the
//! battery's negative terminal is a supply 5.1 V below it, and its positive one
//! does not appear at all -- nothing here draws from it except through the
//! ideal op-amps and the zener's own resistor, which an ideal reference makes
//! irrelevant. The JFETs' gates and sources keep their real difference, which
//! is all they know about.
//!
//! Not built: D1, the reversal diode, and C8 across the zener, which an ideal
//! reference has no use for.

use crate::dsp::netlist::{
    steps, BipolarSpec, Circuit, DiodeSpec, Fault, JfetSpec, Netlist, Taper,
};

/// R36 SPEED, 470 k as a rheostat. The Drive knob turns it.
pub const SPEED: usize = 0;
/// R28 in or out: the block logo's 24 k from the last stage back into the
/// second, as a two-position switch on the pedal's one tone knob. Down is the
/// script (out), up the block (in).
pub const BLOCK: usize = 1;
/// The 250 k bias trimmer. Not on the panel: set by the procedure.
pub const BIAS: usize = 2;

/// Where the switch rests: R28 in, the block, as Dunlop's M-101 is built.
pub const BLOCK_REST: f64 = 1.0;
/// Where the trimmer rests: fitted to the bias procedure by
/// `examples/orange_phase_op.rs` -- the bottom of the oscillator's swing puts
/// the gates at the JFETs' cut-off, "for a maximum span".
pub const BIAS_REST: f64 = 0.4442;

/// D2, the reference everything is built about.
const ZENER: f64 = 5.1;
const BATTERY: f64 = 9.0;
/// A TL061 on 9 V reaches within about 1.5 V of either rail. ESTIMATED from
/// the data sheet's swing into 10 k on +-15 V.
const HEADROOM: f64 = 1.5;
/// The signal stages clip at the nearer of their two limits, 2.4 V above the
/// zener (the other is 3.6 V below it). APPROXIMATED as symmetric: nothing in
/// the signal path reaches either but a hot input with R28 in.
const RAIL: f64 = BATTERY - HEADROOM - ZENER;

/// U1b's TL061, for the oscillator: Texas's typical 1 MHz, 3.5 V/us and
/// 6 V/mV. The Schmitt's switching is the op-amp's own dynamics, which an
/// ideal one does not have.
const GBW_HZ: f64 = 1.0e6;
const SLEW: f64 = 3.5e6;
const OPEN_LOOP: f64 = 6.0e3;
/// The integrating capacitor of the op-amp model; see `tl061`.
const INTEGRATOR: f64 = 1e-9;

/// Q2-Q5, a hand-matched set of 2N5952: the four identical, at the middle of
/// the data sheet (IDSS 4-8 mA, VGS(off) -1.3 to -3.5 V). ESTIMATED. MXR
/// matched them so that one gate voltage tunes all four stages alike; the
/// trimmer is then set for the set's own cut-off (`BIAS_REST`).
pub const J2N5952: JfetSpec = JfetSpec {
    idss: 6.0e-3,
    pinch_off: -2.4,
};

/// Q1, a 2N4125 PNP: hFE 50-150 on the sheet; its middle. ESTIMATED.
const Q2N4125: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 120.0,
    reverse_beta: 3.0,
    early: 100.0,
};

/// What is left of SPEED at the fast end: a carbon track's end resistance,
/// about one per cent of 470 k. ESTIMATED; without it the fast end would tie
/// C10 to U1b's output and the oscillator would stop.
const SPEED_END: f64 = 4_700.0;

/// R28 out: the switch as a rheostat this large, so that its two steps are
/// open and exactly 24 k (inside every pot's clamp, which a short is not).
const R28_OPEN: f64 = 1.0e8;
const R28: f64 = 24_000.0;

/// Where C10 starts, re the zener: just below the oscillator's lower
/// threshold (R19 / R21 of U1b's low swing, about -0.89 V), so the first sweep
/// starts at once and from the same phase every time. The hardware's C10
/// starts from nothing when the battery goes in and its first cycle takes as
/// long as charging it from the battery's negative does -- seconds at the slow
/// end, every time the pedal is reset here. And it cannot be left to the
/// operating point, which is the balance between the thresholds: whether the
/// oscillator ever leaves it was rounding (`Netlist::charged`).
const C10_START: f64 = -1.0;

pub const OUTPUT: &str = "out";

/// A TL061 as the solver's transconductor into an integrator -- its bandwidth
/// and slew -- with its swing two diodes on the integrator node, behind a
/// follower: `rodent::lm308` with this part's figures and this supply's limits.
fn tl061(net: &mut Netlist, prefix: &str, plus: &str, minus: &str, out: &str) {
    let stage = format!("{prefix}_int");
    let high = format!("{prefix}_high");
    let low = format!("{prefix}_low");
    let gm = std::f64::consts::TAU * GBW_HZ * INTEGRATOR;
    let drop = 0.55;
    net.transconductor(plus, minus, &stage, "gnd", gm, SLEW * INTEGRATOR)
        .capacitor(&stage, "gnd", INTEGRATOR)
        .resistor(&stage, "gnd", OPEN_LOOP / gm)
        .supply(&high, 1.0, BATTERY - HEADROOM - ZENER - drop)
        .supply(&low, 1.0, HEADROOM - ZENER + drop)
        .diode(&stage, &high, DiodeSpec::SILICON)
        .diode(&low, &stage, DiodeSpec::SILICON)
        .opamp(out, &stage, out, 1_000.0);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, OUTPUT)
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Orange Phase");

    // The battery's negative terminal, which is the drawing's ground.
    net.supply("v0", 1.0, -ZENER);

    // --- the input buffer --------------------------------------------------------
    net.input("in", source)
        .resistor("in", "n1", 10_000.0) // R3
        .capacitor("n1", "b", 0.01e-6) // C5
        .resistor("b", "gnd", 470_000.0) // R14, to the reference
        .opamp("a", "b", "a", RAIL); // U1a

    // --- the four stages ---------------------------------------------------------
    // Each a first-order all-pass about the zener: the input through 10 k to
    // -in with 10 k round it, and through 47 nF to +in, which 24 k ties to the
    // reference and the JFET's channel shunts. U2a, U2d, U2c, U2b in signal
    // order (R1 / R5 / C1 / R6 / Q2, R11 / R10 / C3 / R25 / Q3, R12 / R13 / C6 /
    // R26 / Q4, R17 / R18 / C9 / R23 / Q5).
    for (k, (input, out)) in [("a", "s1"), ("s1", "s2"), ("s2", "s3"), ("s3", "s4")]
        .into_iter()
        .enumerate()
    {
        let (minus, plus) = (format!("m{k}"), format!("p{k}"));
        net.resistor(input, &minus, 10_000.0)
            .resistor(&minus, out, 10_000.0)
            .capacitor(input, &plus, 47e-9)
            .resistor(&plus, "gnd", 24_000.0)
            .jfet(&plus, "gate", "gnd", J2N5952)
            .opamp(out, &plus, &minus, RAIL);
    }
    // R28: from the last stage's output to the second stage's -in. Its two
    // steps are open and 24 k; with `a` and the wiper on one node, what is in
    // circuit is the wiper-to-`b` part.
    net.rest(BLOCK, BLOCK_REST).pot(
        "m1",
        "m1",
        "s4",
        R28_OPEN,
        steps([1.0, R28 / R28_OPEN]),
        BLOCK,
    );

    // --- the mixer ----------------------------------------------------------------
    // Q1 common emitter, its emitter on the reference: the dry signal and the
    // shifted one into its base through equal resistors, R7 from its collector,
    // so each is inverted at unity and they are summed half and half. R2 is
    // returned to the reference rather than to the battery: the same at audio,
    // and the output then has no standing offset to hand on.
    net.resistor("a", "qb", 150_000.0) // R16
        .resistor("s4", "qb", 150_000.0) // R8
        .bipolar_pnp("qc", "qb", "gnd", Q2N4125) // Q1
        .resistor("qc", "qb", 150_000.0) // R7
        .resistor("qc", "v0", 56_000.0) // R4
        .capacitor("qc", OUTPUT, 47e-9) // C2
        .resistor(OUTPUT, "gnd", 150_000.0) // R2
        .resistor(OUTPUT, "gnd", load);

    // --- the oscillator -----------------------------------------------------------
    // SPEED: with `b` and the wiper on one node, what is in circuit is
    // `R (1 - f)`, and a C track makes that `R audio(1 - p)` -- all of it at
    // the slow stop, a tenth at half rotation. The track's law is ESTIMATED: a
    // reverse-log speed control is the usual one, and it spreads the rates
    // evenly round the knob.
    net.resistor("gnd", "lp", 150_000.0) // R19
        .resistor("lfo", "lp", 470_000.0) // R21
        .resistor("c10", "lm", 150_000.0) // R24
        .capacitor("lm", "lfo", 0.01e-6) // C7
        .pot("c10", "sw", "sw", 470_000.0, Taper::AntiAudio, SPEED) // R36
        .resistor("sw", "lfo", SPEED_END)
        .capacitor("c10", "v0", 15e-6) // C10
        .charged("c10", "v0", C10_START + ZENER)
        .resistor("c10", "gate", 3_900_000.0); // R22
    tl061(&mut net, "u1b", "lp", "lm", "lfo");

    // --- the bias -----------------------------------------------------------------
    // The trimmer across the zener, its wiper through R20 to the gates, C4 on
    // them. Fully up the gates sit on the reference (the JFETs on); fully down
    // at the battery's negative (off).
    net.rest(BIAS, BIAS_REST)
        .pot("gnd", "tw", "v0", 250_000.0, Taper::Linear, BIAS)
        .resistor("tw", "gate", 1_000_000.0) // R20
        .capacitor("gate", "v0", 47e-9); // C4

    net.build(at)
}
