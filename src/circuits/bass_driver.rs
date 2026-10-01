//! The Bass Driver: a buffer, a passive notch, a presence stage, two drive
//! stages clipping on a zener pair, two Sallen-Key low-passes, a Blend with
//! the dry signal, and an active mid, bass and treble.
//!
//! Engineering reference (developer documentation, not panel text): the Tech
//! 21 SansAmp Bass Driver DI, **V2**, from kanengomibako's trace of a unit bought
//! in January 2022 (corrected April 2022), with board photographs and measured
//! responses. Tech 21 publishes no drawing. See `docs/models/bass_driver.md`.
//!
//! ```text
//! in -- U1B buffer --+-- notch (R5, C5-C7, R6, R7) -- U1A, PRESENCE in its loop
//!                    |     -- U901A, DRIVE's left track its feedback
//!                    |     -- DRIVE's right track -- U901B, a 3.3 V zener pair across it
//!                    |     -- Sallen-Key 2.7 kHz -- Sallen-Key 3.3 kHz --+
//!                    +------------------- BLEND -------------------------+
//!   -- LEVEL -- U3B x3.2 -- MID (U3A) -- BASS / TREBLE (U6B) -- U6A -- out
//! ```
//!
//! Every stage is capacitor-coupled and every op-amp sits at the 4.5 V bias,
//! so the circuit is built about the bias as its ground with +-4.5 V rails --
//! the project's way with op-amp sections on a bias (failure mode 4).

use crate::dsp::netlist::{Adjust, Circuit, DiodeSpec, Fault, Netlist, Taper};

/// VR13 DRIVE, 100 k B: one track whose wiper is U901A's output, its left
/// part U901A's feedback and its right part in series into U901B. Turned up
/// both stages gain together, 13 dB to 49 dB in all.
pub const DRIVE: usize = 0;
/// VR58 PRESENCE, 100 k B, in U1A's feedback.
pub const PRESENCE: usize = 1;
/// VR50 BLEND, 100 k B, from the dry buffer (down) to the emulation (up).
pub const BLEND: usize = 2;
/// VR48 BASS, 100 k B.
pub const BASS: usize = 3;
/// VR1 MID, 100 k B.
pub const MID: usize = 4;
/// VR57 TREBLE, 100 k B.
pub const TREBLE: usize = 5;
/// VR51 LEVEL, 100 k B.
pub const LEVEL: usize = 6;

/// Where the level rests: unity through the pedal with every other control
/// at noon, Blend too (half the emulation reaches Level there, so Blend up is
/// about 6 dB hotter), measured by `examples/bass_driver_op.rs`.
pub const LEVEL_REST: f64 = 0.1518;

/// Blend where the box's own sample settings put it: all the way up, all
/// SansAmp. The panel reaches it; this is where it rests when nothing does.
pub const BLEND_REST: f64 = 1.0;

/// The switches' contacts: adjustable resistors, closed and open.
pub const SWITCH_CLOSED: f64 = 1.0;
pub const SWITCH_OPEN: f64 = 1.0e9;
const C: f64 = SWITCH_CLOSED;
const O: f64 = SWITCH_OPEN;

/// SW4B, BASS SHIFT: R32 to C28 10 nF (80 Hz, up) or to C20 47 nF (40 Hz). In
/// the panel's order, the built position second: 40 Hz, 80 Hz.
pub const BASS_SHIFT_SLOTS: [usize; 2] = [0, 1];
pub const BASS_SHIFT: [[f64; 2]; 2] = [[O, C], [C, O]];

/// SW5A / SW5B, MID SHIFT: closed, C36 joins C13 and C19 joins C21 (500 Hz,
/// up); open, each hangs on 1 M (1000 Hz). In the panel's order, the built
/// position second: 1 kHz, 500 Hz.
pub const MID_SHIFT_SLOTS: [usize; 2] = [2, 3];
pub const MID_SHIFT: [[f64; 2]; 2] = [[O, O], [C, C]];

/// What the TLC2262s swing either way of the bias on a fresh 9 V battery:
/// rail to rail. ESTIMATED; on the mains adapter, behind D12, about 0.3 V less.
const RAIL: f64 = 4.5;

/// The 1N4148s are not in the signal path; the zener pair is, as two zeners
/// anode to anode: each direction one in breakdown and the other forward.
const FORWARD: DiodeSpec = DiodeSpec::SILICON;

/// One BZB984-C3V3 zener's reverse breakdown, the way a SPICE zener model has
/// it: an exponential with a modest emission behind a series resistance
/// (`ZENER_RS`). Fitted to Nexperia's figures: 3.3 V at 5 mA (3.1-3.5), and a
/// differential resistance there of at most 95 ohm (this gives 65); about
/// 3.0 V at 1 mA and 2.75 V at 0.1 mA. APPROXIMATED.
///
/// Not a single exponential through the data sheet's 5 uA of leakage at 1 V,
/// the first fit: that figure is a maximum, and the curve through it conducts
/// 0.7 uS at no voltage at all, two of them a 2.5 M shunt across R903 at rest
/// that took 0.75 dB off the drive stages at every setting. The tracer's own
/// simulation of the trace has the stages' arithmetic exactly (12.9 dB at
/// minimum), and so does this one.
pub const ZENER_3V3: DiodeSpec = DiodeSpec {
    saturation: 4.166e-20,
    emission: 3.0,
};

/// The zener's series resistance; see `ZENER_3V3`.
pub const ZENER_RS: f64 = 50.0;

/// Q4, the effect switch, on: an MMBF4393's resistance with its gate at the
/// source. Q1 and Q2 are off with the effect on, and are left out.
const Q4_ON: f64 = 100.0;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Bass Driver");

    // --- input and buffer ------------------------------------------------------
    // D5 / D6 to the rails conduct only past them; not built.
    net.input("in", source)
        .capacitor("in", "c24", 47e-9) // C24
        .resistor("c24", "in1", 10_000.0) // R45
        .resistor("in1", "gnd", 1_000_000.0) // R2
        .linear_opamp("buf", "in1", "buf"); // U1B

    // --- the notch, and U1A with PRESENCE ------------------------------------------
    // PRESENCE's wiper is U1A's -in, its right end the output, its left end
    // R10 and C34 to ground: turned up, more of the track is feedback and the
    // top lifts. `pot(a, wiper, b)` puts `R f(p)` between the wiper and `b`.
    net.resistor("buf", "nx", 100_000.0) // R5
        .capacitor("buf", "ny", 22e-9) // C5
        .resistor("ny", "gnd", 2_200.0) // R6
        .capacitor("ny", "nz", 22e-9) // C6
        .capacitor("nx", "nz", 22e-9) // C7
        .resistor("nz", "gnd", 22_000.0) // R7
        .capacitor("nx", "nw", 22e-9) // C8
        .resistor("nw", "gnd", 100_000.0) // R8
        .resistor("nw", "p1a", 10_000.0) // R9
        .opamp("u1a", "p1a", "m1a", RAIL) // U1A
        .capacitor("u1a", "m1a", 100e-12) // C10
        .pot("pa", "m1a", "u1a", 100_000.0, Taper::Linear, PRESENCE) // VR58
        .resistor("pa", "pc", 3_300.0) // R10
        .capacitor("pc", "gnd", 10e-9); // C34

    // --- the drive stages (U5, the "CH40" module) ----------------------------------
    // U901A inverting: R12 and C22 in, R14 and DRIVE's track from its -in to the
    // wiper, which is its output. Turned up, the wiper is at the right end, all
    // of the track is feedback and none of it is in series into U901B.
    net.resistor("u1a", "r12", 10_000.0) // R12
        .capacitor("r12", "d1", 1e-6) // C22
        .opamp("o1", "gnd", "d1", RAIL) // U901A
        .resistor("d1", "dl", 22_000.0) // R14
        .pot("dr", "o1", "dl", 100_000.0, Taper::Linear, DRIVE) // VR13
        .capacitor("dr", "c4", 1e-6) // C4
        .resistor("c4", "m7", 10_000.0) // R15
        .opamp("o2", "gnd", "m7", RAIL) // U901B
        .resistor("o2", "m7", 220_000.0) // R903
        // D901: anode to anode at "za", each zener behind its bulk resistance.
        .resistor("m7", "z1", ZENER_RS)
        .diode("z1", "za", ZENER_3V3)
        .diode("za", "z1", FORWARD)
        .resistor("o2", "z2", ZENER_RS)
        .diode("z2", "za", ZENER_3V3)
        .diode("za", "z2", FORWARD);

    // --- the low-passes --------------------------------------------------------------
    net.resistor("o2", "l1", 10_000.0) // R47
        .capacitor("l1", "gnd", 10e-9) // C42
        .resistor("l1", "l2", 22_000.0) // R16
        .resistor("l2", "l2b", 10_000.0) // R17
        .capacitor("l2b", "gnd", 47e-9) // C14
        .capacitor("l2", "u2b", 10e-9) // C15
        .resistor("l2", "l3", 33_000.0) // R18
        .capacitor("l3", "gnd", 470e-12) // C16
        .linear_opamp("u2b", "l3", "u2b") // U2B
        .capacitor("u2b", "h1", 22e-9) // C41
        .resistor("h1", "gnd", 100_000.0) // R46
        .resistor("h1", "h2", 33_000.0) // R19
        .capacitor("h2", "u2a", 2.2e-9) // C17
        .resistor("h2", "h3", 33_000.0) // R20
        .capacitor("h3", "gnd", 1e-9) // C18
        .linear_opamp("u2a", "h3", "u2a") // U2A
        .capacitor("u2a", "wet", 1e-6); // C2

    // --- BLEND, LEVEL and U3B ----------------------------------------------------------
    // BLEND from the dry buffer to the emulation, its wiper to LEVEL's top;
    // LEVEL's bottom through R39 1 k to the bias. U3B 1 + 22 k / 10 k.
    net.rest(BLEND, BLEND_REST)
        .pot("wet", "bl", "buf", 100_000.0, Taper::Linear, BLEND) // VR50
        .rest(LEVEL, LEVEL_REST)
        .pot("bl", "lv", "lb", 100_000.0, Taper::Linear, LEVEL) // VR51
        .resistor("lb", "gnd", 1_000.0) // R39
        .resistor("lv", "p3b", 10_000.0) // R36
        .opamp("u3b", "p3b", "m3b", RAIL) // U3B
        .resistor("m3b", "gnd", 10_000.0) // R67
        .resistor("u3b", "m3b", 22_000.0); // R63

    // --- MID (U3A) -------------------------------------------------------------------------
    // A unity inverting stage with a band in its loop: VR1 between R21 and R62,
    // C13 (and C36 at 500 Hz) across it, its wiper through C21 (and C19) to -in.
    net.resistor("u3b", "m3a", 100_000.0) // R23
        .opamp("u3a", "gnd", "m3a", RAIL) // U3A
        .resistor("u3a", "m3a", 100_000.0) // R55
        .capacitor("u3a", "m3a", 100e-12) // C11
        .resistor("u3b", "ml", 3_300.0) // R21
        .pot("ml", "mw", "mr", 100_000.0, Taper::Linear, MID) // VR1
        .resistor("mr", "u3a", 3_300.0) // R62
        .capacitor("ml", "mr", 10e-9) // C13
        .resistor("ml", "mk", 1_000_000.0) // R13
        .capacitor("mk", "mr", 10e-9) // C36
        .capacitor("m3a", "mw", 10e-9) // C21
        .capacitor("m3a", "ms", 10e-9) // C19
        .resistor("ms", "mw", 1_000_000.0); // R27

    // --- BASS and TREBLE (U6B) ----------------------------------------------------------------
    net.capacitor("u3a", "bin", 2e-6) // C12, C23
        .resistor("bin", "t", 1_000_000.0) // R30
        .opamp("u6b", "gnd", "t", RAIL) // U6B
        .resistor("u6b", "t", 1_000_000.0) // R35
        .capacitor("u6b", "t", 22e-12) // C32
        .resistor("bin", "ta", 3_300.0) // R31
        .pot("ta", "tw", "tb", 100_000.0, Taper::Linear, TREBLE) // VR57
        .resistor("tb", "u6b", 3_300.0) // R34
        .capacitor("ta", "tb", 4.7e-9) // C30
        .capacitor("tb", "u6b", 22e-9) // C31
        .capacitor("tw", "t", 1e-9) // C29
        .resistor("bin", "ba", 3_300.0) // R29
        .pot("ba", "bw", "bb", 100_000.0, Taper::Linear, BASS) // VR48
        .resistor("bb", "u6b", 3_300.0) // R33
        .capacitor("ba", "bb", 100e-9) // C27
        .capacitor("bw", "c28", 10e-9) // C28
        .capacitor("bw", "c20", 47e-9) // C20
        .resistor("c28", "c20", 1_000_000.0) // R4
        .resistor("sw4", "t", 100_000.0); // R32

    // --- the switches, built where `BASS_SHIFT[1]` and `MID_SHIFT[1]` have them -----------------------
    for ((a, b), (slot, value)) in [("sw4", "c28"), ("sw4", "c20")]
        .into_iter()
        .zip(BASS_SHIFT_SLOTS.into_iter().zip(BASS_SHIFT[1]))
    {
        let made = net.adjustable(a, b, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }
    for ((a, b), (slot, value)) in [("ml", "mk"), ("ms", "mw")]
        .into_iter()
        .zip(MID_SHIFT_SLOTS.into_iter().zip(MID_SHIFT[1]))
    {
        let made = net.adjustable(a, b, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }

    // --- the 1/4" output ------------------------------------------------------------------------------
    // R24 and Q4 into U6A. Its output-level switch at -10 dB, the instrument
    // level the manual gives for a stomp box: U6A a follower. C35 / C38, R40
    // and R41 to the jack.
    net.resistor("u6b", "q4", 4_700.0) // R24
        .resistor("q4", "on", Q4_ON) // Q4
        .opamp("u6a", "on", "u6a", RAIL) // U6A
        .capacitor("u6a", "c35", 2e-6) // C35, C38
        .resistor("c35", "gnd", 100_000.0) // R40
        .resistor("c35", "out", 1_000.0) // R41
        .resistor("out", "gnd", load);

    net.build(at)
}
