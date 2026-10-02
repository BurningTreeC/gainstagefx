//! The Modern Purple: six op-amp stages, two clippers, a passive bass and
//! treble, an active middle.
//!
//! Engineering reference (developer documentation, not panel text): the Revv
//! G3, the original of 2018 (not the V2), from PedalPCB's trace of a genuine
//! unit ("Tyrian Distortion", document revision 10.14.19, designators below
//! are its). See `docs/models/modern_purple.md`.
//!
//! ```text
//! in -- R1 1M -- C1 22n -- (R3 470K) -- R5 10K -- (C3 100p) -- IC2.2 x2.74
//!    (R7 82K / R6 47K) -- C6 2n2 [+ C7 2n2 by AGGRESSION] -- (R10 33K)
//!    -- R11 10K -- (C9 1n) -- IC2.1, R17 56K + GAIN 1M over a leg of R16 33K
//!    [ // R14 33K or R15 10K by AGGRESSION] into C10 220n, C11 100p
//! -- C12 22n -- R18 22K -- IC2.4 inverting, R19 56K // C13 100p // two 1N4148
//!    pairs -- R20 4K7 -- IC2.3 inverting, R21 82K // C14 100p
//! -- C15 22n -- R22 4K7 -- (C16 10n, red LEDs D6/D7 to ground)
//! -- R23 56K -- (C17 4n7) -- BASS A100K (R24 82K, C18 47n, C19 100n, R25 2K)
//!    and TREBLE A50K (C20 4n7, C21 47n), wiper to wiper through R26 4K7
//! -- R27 100K -- C22 47n -- IC1.2 (R28 470K, R29 / R31 22K, R30 150K + C23
//!    220n, C24 2n2: unity in the bass, x1.15 above) -- MID B100K across C25 /
//!    C26 10n to R32 10K, wiper through R33 1K5 -- IC1.3 x48 (R35 470K // C27
//!    100p over R34 10K) -- C28 220n -- R36 2K -- VOLUME A50K
//! ```
//!
//! **Aggression is not a clipping switch**, whatever the box's manual calls
//! it: both its halves change gain. Engaged either way it puts C7 beside C6,
//! which halves the low cut into R10 (2.2 kHz to 1.1 kHz); and it puts R14
//! (one way, Blue) or R15 (the other, Red) beside R16 in the gain stage's leg,
//! which at full GAIN takes the stage from x33 to x65 or x139. Off is neither.
//! The clipping is fixed: the diode pairs in IC2.4's loop and the LEDs to
//! ground.
//!
//! The pedal rides a 4.5 V reference (R8 / R9 20 K, buffered by IC1.1 and
//! IC1.4 to VREF_B and VREF_C). Here that reference is the netlist's ground
//! and the 9 V supply is +-4.5 V about it -- this repository's failure mode 4:
//! a section referenced to ground and coupled into, as `rodent` is. Every
//! part the drawing returns to ground is at a signal ground either way, and
//! the coupling capacitors make the two the same circuit.

use crate::dsp::netlist::{steps, Circuit, DiodeSpec, Fault, Netlist, Taper};

/// GAIN, B1M as a rheostat in the gain stage's feedback. The Drive knob.
pub const GAIN: usize = 0;
/// BASS, A100K.
pub const BASS: usize = 1;
/// MID, B100K.
pub const MID: usize = 2;
/// TREBLE, A50K.
pub const TREBLE: usize = 3;
/// VOLUME, A50K.
pub const VOLUME: usize = 4;
/// AGGRESSION, the three-way toggle: off, Blue, Red in knob order (the box
/// has Off in the middle of its throw).
pub const AGGRESSION: usize = 5;

/// Where VOLUME rests: **unity through the pedal** for a guitar's 110 Hz and
/// 330 Hz at its level with every other knob at rest, which is what the
/// panel's Level knob means at noon. Found by `examples/modern_purple_op.rs`.
pub const VOLUME_REST: f64 = 0.353;
/// Where AGGRESSION rests: Blue, the middle of the knob, where the plugin's
/// default knob value of a half puts it.
pub const AGGRESSION_REST: f64 = 0.5;

/// TL072 / TL074, from the data sheet: 3 MHz, 13 V/us, 200 V/mV (DOCUMENTED,
/// typical).
const GBW_HZ: f64 = 3.0e6;
const SLEW: f64 = 13.0e6;
const OPEN_LOOP: f64 = 2.0e5;
const INTEGRATOR: f64 = 1e-9;
/// Half the 9 V supply (less the 1N5817's few hundred millivolts, which the
/// trace's R2 10R and D1 take), and how near a TL07x swings to its rails.
const HALF_SUPPLY: f64 = 4.3;
const HEADROOM: f64 = 1.5;

/// A switch in a stepped rheostat: this large when open.
const OPEN: f64 = 1.0e8;
/// And this small when closed.
const CLOSED: f64 = 1.0;

/// 1N4148 (`DiodeSpec::SILICON`).
const SILICON: DiodeSpec = DiodeSpec::SILICON;
/// The 3 mm red LEDs, the catalogue's red LED (`DiodeSpec::LED`), as the
/// Brit Drive's.
const RED_LED: DiodeSpec = DiodeSpec::LED;

/// A TL07x section as the solver's transconductor into an integrator, its
/// swing two diodes on the integrator node, behind a follower
/// (`orange_phase::tl061` with this part's figures and this supply).
fn tl07x(net: &mut Netlist, name: &str, plus: &str, minus: &str, out: &str) {
    let stage = format!("{name}_int");
    let gm = std::f64::consts::TAU * GBW_HZ * INTEGRATOR;
    net.transconductor(plus, minus, &stage, "gnd", gm, SLEW * INTEGRATOR)
        .capacitor(&stage, "gnd", INTEGRATOR)
        .resistor(&stage, "gnd", OPEN_LOOP / gm)
        .diode(&stage, "rail_high", SILICON)
        .diode("rail_low", &stage, SILICON)
        .opamp(out, &stage, out, 1_000.0);
}

pub const OUTPUT: &str = "out";

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, OUTPUT)
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Modern Purple");
    let drop = 0.55;
    net.supply("rail_high", 1.0, HALF_SUPPLY - HEADROOM - drop)
        .supply("rail_low", 1.0, -(HALF_SUPPLY - HEADROOM - drop));

    // --- the input stage ------------------------------------------------------------
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0) // R1
        .capacitor("in", "n1", 22e-9) // C1
        .resistor("n1", "gnd", 470_000.0) // R3
        .resistor("n1", "p22", 10_000.0) // R5
        .capacitor("p22", "gnd", 100e-12) // C3
        .resistor("o22", "m22", 82_000.0) // R7
        .resistor("m22", "gnd", 47_000.0); // R6
    tl07x(&mut net, "ic22", "p22", "m22", "o22");

    // --- the gain stage ---------------------------------------------------------------
    // AGGRESSION's two halves, one control: C7 beside C6 through a switch, and
    // a leg beside R16, each a stepped rheostat with the same three steps.
    net.rest(AGGRESSION, AGGRESSION_REST)
        .capacitor("o22", "n2", 2.2e-9) // C6
        .capacitor("o22", "c7", 2.2e-9) // C7
        .pot(
            "c7",
            "c7",
            "n2",
            OPEN,
            steps([1.0, CLOSED / OPEN, CLOSED / OPEN]),
            AGGRESSION,
        ) // SW1.2
        .resistor("n2", "gnd", 33_000.0) // R10
        .resistor("n2", "p21", 10_000.0) // R11
        .capacitor("p21", "gnd", 1e-9) // C9
        .resistor("m21", "r17", 56_000.0) // R17
        .pot("o21", "o21", "r17", 1_000_000.0, Taper::Linear, GAIN) // GAIN
        .capacitor("m21", "o21", 100e-12) // C11
        .resistor("m21", "c10", 33_000.0) // R16
        .pot(
            "m21",
            "m21",
            "c10",
            OPEN,
            steps([1.0, 33_000.0 / OPEN, 10_000.0 / OPEN]),
            AGGRESSION,
        ) // SW1.1: R14 (Blue), R15 (Red)
        .capacitor("c10", "gnd", 220e-9); // C10
    tl07x(&mut net, "ic21", "p21", "m21", "o21");

    // --- the clippers -----------------------------------------------------------------
    net.capacitor("o21", "n4", 22e-9) // C12
        .resistor("n4", "m24", 22_000.0) // R18
        .resistor("m24", "o24", 56_000.0) // R19
        .capacitor("m24", "o24", 100e-12) // C13
        .diode("o24", "d24a", SILICON) // D4
        .diode("d24a", "m24", SILICON) // D2
        .diode("m24", "d24b", SILICON) // D3
        .diode("d24b", "o24", SILICON) // D5
        .resistor("o24", "m23", 4_700.0) // R20
        .resistor("m23", "o23", 82_000.0) // R21
        .capacitor("m23", "o23", 100e-12) // C14
        .capacitor("o23", "n5", 22e-9) // C15
        .resistor("n5", "clip", 4_700.0) // R22
        .capacitor("clip", "gnd", 10e-9) // C16
        .diode("gnd", "clip", RED_LED) // D6
        .diode("clip", "gnd", RED_LED); // D7
    tl07x(&mut net, "ic24", "gnd", "m24", "o24");
    tl07x(&mut net, "ic23", "gnd", "m23", "o23");

    // --- bass and treble --------------------------------------------------------------
    net.resistor("clip", "t1", 56_000.0) // R23
        .capacitor("t1", "gnd", 4.7e-9) // C17
        .resistor("t1", "bt", 82_000.0) // R24
        .capacitor("bt", "w", 47e-9) // C18
        .pot("bt", "w", "bb", 100_000.0, Taper::Audio, BASS) // BASS
        .capacitor("w", "bb", 100e-9) // C19
        .resistor("bb", "gnd", 2_000.0) // R25
        .resistor("w", "x", 4_700.0) // R26
        .capacitor("t1", "tt", 4.7e-9) // C20
        .pot("tt", "x", "tb", 50_000.0, Taper::Audio, TREBLE) // TREBLE
        .capacitor("tb", "gnd", 47e-9) // C21
        .resistor("x", "r27", 100_000.0) // R27
        .capacitor("r27", "p12", 47e-9) // C22
        .resistor("p12", "gnd", 470_000.0) // R28
        .capacitor("m12", "o12", 2.2e-9) // C24
        .resistor("m12", "m12b", 22_000.0) // R29
        .resistor("m12b", "o12", 22_000.0) // R31
        .resistor("m12b", "c23", 150_000.0) // R30
        .capacitor("c23", "gnd", 220e-9); // C23
    tl07x(&mut net, "ic12", "p12", "m12", "o12");

    // --- the middle and the output ----------------------------------------------------
    net.rest(VOLUME, VOLUME_REST)
        .pot("o12", "wm", "z", 100_000.0, Taper::Linear, MID) // MID
        .capacitor("o12", "g", 10e-9) // C25
        .capacitor("z", "g", 10e-9) // C26
        .resistor("g", "gnd", 10_000.0) // R32
        .resistor("wm", "p13", 1_500.0) // R33
        .resistor("m13", "gnd", 10_000.0) // R34
        .resistor("m13", "o13", 470_000.0) // R35
        .capacitor("m13", "o13", 100e-12) // C27
        .capacitor("o13", "r36", 220e-9) // C28
        .resistor("r36", "vt", 2_000.0) // R36
        .pot("vt", OUTPUT, "gnd", 50_000.0, Taper::Audio, VOLUME) // VOLUME
        .resistor(OUTPUT, "gnd", load);
    tl07x(&mut net, "ic13", "p13", "m13", "o13");

    net.build(at)
}
