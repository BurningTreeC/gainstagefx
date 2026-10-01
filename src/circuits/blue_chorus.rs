//! The Blue Chorus: pre-emphasis, a three-pole filter into a bucket brigade,
//! the same out of it, and the delayed signal mixed half and half with the dry
//! one through the de-emphasis.
//!
//! Engineering reference (developer documentation, not panel text): the Boss
//! CE-2, from Boss's own CE-2/CE-2B Service Notes, first edition, February
//! 1987. See `docs/models/blue_chorus.md`.
//!
//! ```text
//! in -- C1 / R1 -- Q1 follower -- C2 -- IC1a pre-emphasis (x1 below 410 Hz,
//!    x5.7 above 2.34 kHz) -- the dry signal to the mixer, and
//!    C5 -- R9 / R10 / R11 with C6 / C7 / C8 round Q2 -- R13 -- MN3007 IN
//! MN3007 OUT -- R14 -- C10 -- R16 / R17 / R18 with C11 / C12 / C13 round Q3
//!    -- C14 -- R22 -- Q9 (on) -- IC1b: dry through R21, wet through R22,
//!    de-emphasis in its feedback -- R25 / C17 -- out
//! LFO: IC2a Schmitt, IC2b integrator, RATE and DEPTH, R35 / C21 into Q4,
//!    whose emitter sets the MN3101's clock
//! ```
//!
//! **The bucket brigade is not in the netlist.** Its input is a node here
//! (`BBD_IN`) and its output is an auxiliary input (`BBD_RETURN`); what lies
//! between is `dsp::bbd::Brigade`, a delay of 1024 stages at the clock
//! `delay_seconds` reads off Q4's emitter (`CLOCK_CONTROL`). Everything that
//! shapes the sound either side of it -- both filters, the emphasis, the
//! mixer, the oscillator and its depth -- is solved here.
//!
//! **Referenced to the bias line**, as the Orange Phase is to its zener: the
//! op-amps and the transistor followers all ride VR3's 4.5 V, which C18 holds
//! still, so it is this netlist's ground; the battery is a supply either side
//! of it. The LFO's own bias (R33 / R34 / C20) is the same 4.5 V and is the
//! same node.
//!
//! **Held on.** Q9, the effect switch, is a 2SK30A conducting; the flip-flop
//! that drives it (Q6 / Q7) and the bypass path are not built.

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, Netlist, Taper};

/// VR1 RATE, 100 k B. The Drive knob turns it.
pub const RATE: usize = 0;
/// VR2 DEPTH, 100 k B. The pedal's tone knob turns it.
pub const DEPTH: usize = 1;

/// The node the MN3007's input is driven at, behind R13.
pub const BBD_IN: &str = "bbd_in";
/// The auxiliary input the MN3007's output drives: the slot's first.
pub const BBD_RETURN: usize = 0;
/// Q4's emitter: the LFO after DEPTH and R35 / C21, which is what the MN3101's
/// clock follows.
pub const CLOCK_CONTROL: &str = "q4e";
/// The longest delay `delay_seconds` gives, for sizing the line.
pub const LONGEST: f64 = 9.0e-3;

pub const OUTPUT: &str = "out";

/// The bias line against the battery: VR3 at its middle between R27 and R28,
/// 4.5 V of the 9 ("a centered operating-point", the adjustment says).
const BIAS: f64 = 4.5;
const BATTERY: f64 = 9.0;
/// The op-amps reach within about 1.5 V of either rail. ESTIMATED.
const HEADROOM: f64 = 1.5;
/// IC1's halves clip at the bias plus and minus this. APPROXIMATED.
const RAIL: f64 = BIAS - HEADROOM;

/// IC2, a TL022 for the oscillator: Texas's typical 0.5 MHz and 0.5 V/us, and
/// a gain of 10 V/mV (ESTIMATED; the sheet gives 72 dB typical at its own
/// load). The Schmitt's switching is the op-amp's own dynamics.
const GBW_HZ: f64 = 0.5e6;
const SLEW: f64 = 0.5e6;
const OPEN_LOOP: f64 = 1.0e4;
const INTEGRATOR: f64 = 1e-9;
/// IC2a's bandwidth as built: not the part's 0.5 MHz. APPROXIMATED, and for
/// the solver's sake rather than the circuit's.
///
/// A comparator's flip is a positive-feedback instability far faster than any
/// sample: IC2a's own would grow by e every third of a microsecond, against a
/// step of twenty. An implicit solver stepping over an instability that fast
/// turns the balance it should leave into a stable point of the stepped
/// equations, and the comparator stops there -- measured: the triangle parked
/// at the threshold, IC2a half way off its rail, and Newton at its ceiling of
/// 64 passes on every sample thereafter. (The Orange Phase's Schmitt escapes
/// this because its C7 feeds the jump straight back to the inverting input.)
/// At 5 kHz the flip grows by less than a third of e a step at 44.1 kHz, the
/// solver follows it, and it takes a millisecond instead of a microsecond:
/// the triangle's period, a third of a second at its shortest, is longer by a
/// fraction of a per cent.
const COMPARATOR_GBW_HZ: f64 = 5.0e3;

/// Q1-Q3, 2SC732TM, GR rank (hFE 200-400), and Q4, 2SC945, P rank (200-400):
/// the middle of the rank. ESTIMATED, as the catalogue's other GR parts.
const NPN_GR: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 300.0,
    reverse_beta: 3.0,
    early: 100.0,
};

/// Q9, the effect switch, conducting: a 2SK30A-Y's channel at no gate bias,
/// a few hundred ohms in series with R22's 47 k. APPROXIMATED as a resistor.
const SWITCH_ON: f64 = 400.0;

/// The MN3007's output, a source follower: some kilohms. ESTIMATED; it works
/// into R14 and C10, so it sets nothing in the band.
const BBD_OUTPUT: f64 = 1_000.0;
/// What R13 drives at the MN3007's input: its gate, which draws nothing worth
/// a component. ESTIMATED.
const BBD_INPUT: f64 = 1_000_000.0;

/// Where C19 starts: the triangle just past the Schmitt's lower threshold,
/// about 33/47 of the swing below the bias, so the oscillator starts at once
/// and from the same phase every time rather than from the operating point's
/// balance between the thresholds (`Netlist::charged`).
const C19_START: f64 = -2.3;

/// The delay for a voltage at `CLOCK_CONTROL`.
///
/// 1024 stages at the MN3101's clock: `N / (2 f)`. The clock's law against
/// Q4's emitter is the MN3101's own oscillator with Q5 and C22, which no data
/// sheet states, so it is taken from a measurement of a real CE-2 (a
/// DIYstompboxes thread, "BOSS CE-2: clock frequencies"): about 110 kHz with
/// DEPTH down and 100 to 128 kHz across the sweep at its middle. Those put
/// the sweep's middle at 114 kHz against 110 at rest, which no plausible
/// monotonic law fits (a quadratic through all three turns back at full
/// depth); a clock linear in the control keeps the rest, 110 kHz, and the
/// span, 28 kHz across the middle-depth swing (`examples/blue_chorus_op.rs`
/// fits `CLOCK_SLOPE`), and sweeps 96 to 124 kHz there. SECONDARY figures,
/// ESTIMATED law.
pub fn delay_seconds(control: f64) -> f64 {
    let clock = (CLOCK_REST_HZ + CLOCK_SLOPE * (control - CONTROL_REST)).clamp(60_000.0, 200_000.0);
    1024.0 / (2.0 * clock)
}
/// The clock with DEPTH down, and Q4's emitter there.
const CLOCK_REST_HZ: f64 = 110_000.0;
pub const CONTROL_REST: f64 = -0.8947;
/// Hertz of clock per volt at Q4's emitter.
pub const CLOCK_SLOPE: f64 = 16_524.0;

/// A TL022 half as the solver's transconductor into an integrator behind a
/// follower, its swing two diodes on the integrator node.
fn tl022(net: &mut Netlist, prefix: &str, plus: &str, minus: &str, out: &str, gbw: f64) {
    let stage = format!("{prefix}_int");
    let high = format!("{prefix}_high");
    let low = format!("{prefix}_low");
    let gm = std::f64::consts::TAU * gbw * INTEGRATOR;
    let drop = 0.55;
    net.transconductor(plus, minus, &stage, "gnd", gm, SLEW * INTEGRATOR)
        .capacitor(&stage, "gnd", INTEGRATOR)
        .resistor(&stage, "gnd", OPEN_LOOP / gm)
        .supply(&high, 1.0, BATTERY - HEADROOM - BIAS - drop)
        .supply(&low, 1.0, HEADROOM - BIAS + drop)
        .diode(&stage, &high, DiodeSpec::SILICON)
        .diode(&low, &stage, DiodeSpec::SILICON)
        .opamp(out, &stage, out, 1_000.0);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, OUTPUT)
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Blue Chorus");

    // The battery either side of the bias line.
    net.supply("vp", 1.0, BATTERY - BIAS)
        .supply("v0", 1.0, -BIAS);

    // --- input follower and pre-emphasis -----------------------------------------
    net.input("in", source)
        .capacitor("in", "n1", 0.047e-6) // C1
        .resistor("n1", "q1b", 1_000.0) // R1
        .resistor("q1b", "gnd", 470_000.0) // R2
        .bipolar("vp", "q1b", "q1e", NPN_GR) // Q1
        .resistor("q1e", "v0", 10_000.0) // R3
        .capacitor("q1e", "x", 0.47e-6) // C2
        .resistor("x", "m1", 47_000.0) // R4
        .capacitor("x", "x5", 0.0068e-6) // C3
        .resistor("x5", "m1", 10_000.0) // R5
        .resistor("a1", "m1", 47_000.0) // R6
        .capacitor("a1", "m1", 100e-12) // C4
        .resistor("p1", "gnd", 10_000.0) // R7
        .opamp("a1", "p1", "m1", RAIL); // IC1a

    // --- anti-aliasing, into the bucket brigade -----------------------------------
    net.capacitor("a1", "f0", 0.033e-6) // C5
        .resistor("f0", "gnd", 100_000.0) // R8
        .resistor("f0", "f1", 10_000.0) // R9
        .capacitor("f1", "v0", 0.0033e-6) // C6
        .resistor("f1", "f2", 10_000.0) // R10
        .capacitor("f2", "q2e", 0.0082e-6) // C7
        .resistor("f2", "q2b", 10_000.0) // R11
        .capacitor("q2b", "v0", 470e-12) // C8
        .bipolar("vp", "q2b", "q2e", NPN_GR) // Q2
        .resistor("q2e", "v0", 10_000.0) // R12
        .resistor("q2e", BBD_IN, 4_700.0) // R13
        .resistor(BBD_IN, "gnd", BBD_INPUT);

    // --- out of the bucket brigade: reconstruction ----------------------------------
    let ret = net.aux_input("bbd_out", BBD_OUTPUT);
    debug_assert_eq!(ret, BBD_RETURN);
    net.resistor("bbd_out", "vp", 56_000.0) // R14
        .capacitor("bbd_out", "r0", 0.033e-6) // C10
        .resistor("r0", "gnd", 330_000.0) // R15
        .resistor("r0", "r1", 10_000.0) // R16
        .capacitor("r1", "v0", 0.0033e-6) // C11
        .resistor("r1", "r2", 10_000.0) // R17
        .capacitor("r2", "q3e", 0.0082e-6) // C12
        .resistor("r2", "q3b", 10_000.0) // R18
        .capacitor("q3b", "v0", 470e-12) // C13
        .bipolar("vp", "q3b", "q3e", NPN_GR) // Q3
        .resistor("q3e", "v0", 10_000.0) // R19
        .capacitor("q3e", "w0", 0.033e-6) // C14
        .resistor("w0", "gnd", 1_000_000.0) // R20
        .resistor("w0", "w1", 47_000.0) // R22
        .resistor("w1", "m2", SWITCH_ON); // Q9, on

    // --- the mixer and de-emphasis -------------------------------------------------
    net.resistor("a1", "m2", 47_000.0) // R21, the dry signal
        .resistor("o2", "m2", 47_000.0) // R24
        .capacitor("o2", "m2", 100e-12) // C16
        .capacitor("o2", "x15", 0.0068e-6) // C15
        .resistor("x15", "m2", 10_000.0) // R23
        .opamp("o2", "gnd", "m2", RAIL) // IC1b
        .resistor("o2", "o3", 470.0) // R25
        .capacitor("o3", OUTPUT, 1e-6) // C17
        .resistor(OUTPUT, "gnd", 100_000.0) // R26
        .resistor(OUTPUT, "gnd", load);

    // --- the oscillator -------------------------------------------------------------
    // IC2a: a Schmitt, R30 from its output and R29 from the triangle both into
    // its non-inverting input (pin 5), its inverting one (pin 6) on the bias.
    net.resistor("sq", "sp", 47_000.0) // R30
        .resistor("tri", "sp", 33_000.0); // R29
    tl022(&mut net, "ic2a", "sp", "gnd", "sq", COMPARATOR_GBW_HZ);
    // RATE across the square and R31, its wiper through R32 into the
    // integrator: turned up, the wiper is at the square's end and the triangle
    // fastest.
    net.pot("sq", "rw", "r31", 100_000.0, Taper::Linear, RATE) // VR1
        .resistor("r31", "gnd", 10_000.0) // R31
        .resistor("rw", "im", 1_000_000.0) // R32
        .capacitor("tri", "im", 0.1e-6) // C19
        .charged("tri", "im", C19_START);
    tl022(&mut net, "ic2b", "gnd", "im", "tri", GBW_HZ);
    // DEPTH across the triangle and the bias, its wiper through R35 and C21's
    // smoothing into Q4.
    net.pot("tri", "dw", "gnd", 100_000.0, Taper::Linear, DEPTH) // VR2
        .resistor("dw", "q4b", 220_000.0) // R35
        .capacitor("q4b", "v0", 0.01e-6) // C21
        .bipolar("vp", "q4b", CLOCK_CONTROL, NPN_GR) // Q4
        .resistor(CLOCK_CONTROL, "cv", 4_700.0) // R36
        .resistor("cv", "v0", 4_700.0); // R37

    net.build(at)
}
