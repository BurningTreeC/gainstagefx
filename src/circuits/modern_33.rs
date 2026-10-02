//! The Modern 33: one transistor and one op-amp as a single stage, on a
//! charge-pumped 33 V rail.
//!
//! Engineering reference (developer documentation, not panel text): the
//! Fortin 33, from PedalPCB's trace of a genuine unit ("Triangulum", document
//! revision 03.02.20). Its circuit is the front end of TC Electronic's
//! Integrated Preamp. See `docs/models/modern_33.md`.
//!
//! ```text
//! in -- R8 4K7 -- C10 47n -- Q1 base (BC550C), R5 1M to VREF
//! Q1 collector: R3 100K to VCC, IC1 (TL071) -in; C8 100p from IC1's output
//! IC1 +in at VREF; IC1 out -- R7 220K -- Q1 emitter -- C12 1u -- node E
//! E: R18 820R + R12 1K8 -- C14 100n -- R13 220R to ground
//!    R17 3K9 -- node B: C16 470n to ground, R16 20K + R6 1K to ground
//!    R11 4K7 + R4 12K -- C13 10n -- node O
//! IC1 out -- C11 4u7 -- node O: LEVEL A5K to ground, its wiper the output;
//!    R14 2K2 -- R15 47R + R2 120R (C15 47n across them) -- node B
//! VCC ~33 V; VREF = VCC x 820K / (10K + 100K + 820K), C9 4u7 at 10K / 100K
//! ```
//!
//! The gain is one plus R7 over what the emitter sees to ground through C12:
//! at low frequencies C12, C14 and C16 leave the emitter open and the stage
//! falls toward unity -- the cut below the guitar's low strings that makes it
//! "tight" -- and in the middle of the band the three legs bring it up to the
//! boost the box is known for. Nothing in it is reduced: the network is
//! solved as drawn.
//!
//! Left out: Q2, R9, R10, D1 and D2, the mute circuit copied from the TC pedal
//! whose connection to the input jack was not. PedalPCB's parts list: they
//! "are present in the original but serve no functional purpose".

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, Netlist, Taper};

/// LEVEL, A5K, the box's one knob.
pub const LEVEL: usize = 0;

/// Where LEVEL rests: **unity through the pedal** for a guitar's 110 Hz and
/// 330 Hz at its level, which is what the panel's Level knob means at noon.
/// Found by `examples/modern_33_op.rs`; full LEVEL is +7.4 dB on that mix and
/// +22 dB in the middle of the band.
pub const LEVEL_REST: f64 = 0.683;

/// The rail the TC1044 and its five-stage multiplier make from 9 V: builders
/// measure about 33 V on these boards (WIDELY REPORTED); the multiplier's
/// output impedance under this stage's milliamp is left out (ESTIMATED).
pub const RAIL: f64 = 33.0;

/// TL071, from the data sheet: 3 MHz, 13 V/us, 200 V/mV (DOCUMENTED, typical).
const GBW_HZ: f64 = 3.0e6;
const SLEW: f64 = 13.0e6;
const OPEN_LOOP: f64 = 2.0e5;
const INTEGRATOR: f64 = 1e-9;
/// How close a TL071's output swings to its rails at these currents.
const HEADROOM: f64 = 1.5;

/// BC550C: the BC549C's figures, the same die in a quieter grade (as the
/// TS808's buffers, `ts808::BC549`).
const BC550C: BipolarSpec = BipolarSpec {
    saturation: 1.0e-14,
    forward_beta: 500.0,
    reverse_beta: 4.0,
    early: 100.0,
};

/// A TL071 as the solver's transconductor into an integrator, its swing two
/// diodes on the integrator node, behind a follower (`orange_phase::tl061`
/// with this part's figures and this rail).
fn tl071(net: &mut Netlist, plus: &str, minus: &str, out: &str) {
    let gm = std::f64::consts::TAU * GBW_HZ * INTEGRATOR;
    let drop = 0.55;
    net.transconductor(plus, minus, "ic1_int", "gnd", gm, SLEW * INTEGRATOR)
        .capacitor("ic1_int", "gnd", INTEGRATOR)
        .resistor("ic1_int", "gnd", OPEN_LOOP / gm)
        .supply("ic1_high", 1.0, RAIL - HEADROOM - drop)
        .supply("ic1_low", 1.0, HEADROOM + drop)
        .diode("ic1_int", "ic1_high", DiodeSpec::SILICON)
        .diode("ic1_low", "ic1_int", DiodeSpec::SILICON)
        .opamp(out, "ic1_int", out, 1_000.0);
}

pub const OUTPUT: &str = "out";

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, OUTPUT)
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Modern 33");

    // --- the rail and the reference ---------------------------------------------
    net.supply("vcc", 10.0, RAIL)
        .capacitor("vcc", "gnd", 100e-6) // C107
        .resistor("vcc", "gnd", 100_000.0) // R1
        .resistor("vcc", "c9", 10_000.0) // R101
        .capacitor("c9", "gnd", 4.7e-6) // C9
        .resistor("c9", "vref", 100_000.0) // R102
        .resistor("vref", "gnd", 820_000.0); // R103

    // --- the stage ----------------------------------------------------------------
    net.input("in", source)
        .resistor("in", "r8", 4_700.0) // R8
        .capacitor("r8", "b1", 47e-9) // C10
        .resistor("b1", "vref", 1_000_000.0) // R5
        .bipolar("c1", "b1", "e1", BC550C) // Q1
        .resistor("vcc", "c1", 100_000.0) // R3
        .capacitor("o1", "c1", 100e-12) // C8
        .resistor("o1", "e1", 220_000.0) // R7
        .capacitor("e1", "e", 1e-6); // C12
    tl071(&mut net, "vref", "c1", "o1"); // IC1

    // --- the emitter's three legs --------------------------------------------------
    net.resistor("e", "r18", 820.0) // R18
        .resistor("r18", "c14", 1_800.0) // R12
        .capacitor("c14", "r13", 100e-9) // C14
        .resistor("r13", "gnd", 220.0) // R13
        .resistor("e", "b", 3_900.0) // R17
        .capacitor("b", "gnd", 470e-9) // C16
        .resistor("b", "r16", 20_000.0) // R16
        .resistor("r16", "gnd", 1_000.0) // R6
        .resistor("e", "r11", 4_700.0) // R11
        .resistor("r11", "c13", 12_000.0) // R4
        .capacitor("c13", "o", 10e-9); // C13

    // --- the output ------------------------------------------------------------------
    net.rest(LEVEL, LEVEL_REST)
        .capacitor("o1", "o", 4.7e-6) // C11
        .resistor("o", "a", 2_200.0) // R14
        .resistor("a", "r15", 47.0) // R15
        .resistor("r15", "b", 120.0) // R2
        .capacitor("a", "b", 47e-9) // C15
        .pot("o", OUTPUT, "gnd", 5_000.0, Taper::Audio, LEVEL) // LEVEL
        .resistor(OUTPUT, "gnd", load);

    net.build(at)
}
