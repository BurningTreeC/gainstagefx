//! The wahs: the Black Wah and the Chrome Wah, one circuit with two
//! builds.
//!
//! Engineering reference (developer documentation, not panel text): the
//! Dunlop Cry Baby GCB-95 (rev F, the buffered board of mid-1991 on) and the
//! Vox V847, from ElectroSmash's analyses and parts lists of both. See
//! `docs/models/wahs.md`.
//!
//! ```text
//! (Cry Baby: Cin1 / Q0 MPSA13 follower, collector-fed bias, Rin4 10 k)
//! in -- R1 68 k -- C1 0.01 uF -- Q1 MPSA18 common emitter, R3 22 k / R4 390 (510)
//!    R6 470 k from Q1's collector to the tank, L1 500 mH // R7 33 k from the
//!    tank to the loop, C3 4.7 uF and R8 82 k (100 k) on the tank, R2 1.5 k from
//!    the loop to Q1's base
//!    -- C5 0.22 uF -- the output, and VR1 100 k (the treadle) from there to
//!    ground, its wiper through C4 0.22 uF into Q2 MPSA18, a follower (R5
//!    470 k bias from Q1's collector, R9 1 k, R10 10 k), whose emitter closes
//!    the loop through C2 0.01 uF
//! ```
//!
//! The treadle sets how much of Q1's output Q2 hands back through C2, and so
//! the apparent capacitance the inductor resonates with: the wiper toward the
//! output, more of it handed back, more capacitance and a lower peak -- the
//! heel; toward ground, the toe. 450 Hz to 1.6 kHz, ElectroSmash says, 750 Hz
//! in the middle. The output is taken ahead of the pot, so the treadle moves
//! the peak and not the level.
//!
//! **The treadle is two realtime resistors**, the pot's halves either side of
//! its wiper (`TREADLE_TOP`, `TREADLE_BOTTOM`), which the chain sets every
//! sample with `Simulation::set_realtime_value` (see `treadle_halves`): a pot
//! moved at audio rate, which a control that rebuilds the matrix could not be.

use crate::dsp::netlist::{Adjust, BipolarSpec, Circuit, Fault, Netlist, Taper};

/// The two builds. Not an enum id: the plugin's `wah` parameter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Build {
    /// Dunlop Cry Baby GCB-95, rev F: the buffer, R4 390 ohm, R8 82 k.
    CryBaby,
    /// Vox V847: no buffer, R4 510 ohm, R8 100 k.
    V847,
}

/// The adjustable slots of the treadle's two halves.
pub const TREADLE_TOP: usize = 0;
pub const TREADLE_BOTTOM: usize = 1;

/// VR1: 100 k.
pub const POT: f64 = 100_000.0;

/// The track's law and how much of it the rocker travels. VR1 is printed
/// "Log", and Dunlop's Hot Potz and the Vox's own tracks are custom and
/// unpublished: a logarithmic track of span 10, travelled from 92 % of its
/// rotation at the heel to 9.5 % at the toe, puts the Cry Baby's peak where
/// ElectroSmash's figures do -- 450 Hz heel, 750 Hz in the middle, 1.6 kHz toe
/// (FITTED, three numbers to three; `examples/wah_op.rs` prints the sweep).
/// The plain audio track put the middle at 1.1 kHz.
pub const TRACK_SPAN: f64 = 10.0;
pub const HEEL: f64 = 0.92;
pub const TOE: f64 = 0.095;

/// The pot's halves for a treadle position, 0 heel to 1 toe: the wiper toward
/// ground as the toe goes down. Neither half is ever quite zero, which a
/// resistor in the solver may not be.
pub fn treadle_halves(position: f64) -> (f64, f64) {
    let rotation = HEEL + (TOE - HEEL) * position.clamp(0.0, 1.0);
    let track = Taper::Log { span: TRACK_SPAN };
    let bottom = (POT * track.fraction(rotation)).clamp(10.0, POT - 10.0);
    (POT - bottom, bottom)
}

/// MPSA18, hFE 500-1500 on the sheet: its middle. ESTIMATED.
const MPSA18: BipolarSpec = BipolarSpec {
    saturation: 2.0e-14,
    forward_beta: 800.0,
    reverse_beta: 4.0,
    early: 100.0,
};

/// The inductor: 500 mH and some 15 ohm of winding, the typical figures of
/// the parts ElectroSmash lists (200 mH to 1 H, 10 to 200 ohm). ESTIMATED;
/// linear -- a Fasel's core saturation on very hot input is not modelled.
const L1: f64 = 0.5;
const L1_WINDING: f64 = 15.0;

pub const OUTPUT: &str = "out";

pub fn build(build: Build, source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(build, source, load, OUTPUT)
}

pub fn tap(build: Build, source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new(match build {
        Build::CryBaby => "Black Wah",
        Build::V847 => "Chrome Wah",
    });
    net.supply("v9", 10.0, 9.0);

    // --- the input --------------------------------------------------------------
    net.input("in", source);
    let into = match build {
        Build::CryBaby => {
            // Q0, an MPSA13 Darlington, as a follower: its base biased from
            // its own collector through Rin2, Rin3 1 k to the rail. Built as
            // the pair it is, each half with the MPSA18's figures (ESTIMATED;
            // the MPSA13's hFE is at least 5000, which the pair gives).
            net.capacitor("in", "b0", 0.01e-6) // Cin1
                .resistor("b0", "gnd", 2_200_000.0) // Rin1
                .capacitor("b0", "gnd", 22e-12) // Cin2
                .resistor("v9", "c0", 1_000.0) // Rin3
                .resistor("c0", "b0", 1_800_000.0) // Rin2
                .bipolar("c0", "b0", "d0", MPSA18) // Q0, first of the pair
                .bipolar("c0", "d0", "e0", MPSA18) // Q0, second
                .resistor("e0", "gnd", 10_000.0); // Rin4
            "e0"
        }
        Build::V847 => "in",
    };
    let (r4, r8) = match build {
        Build::CryBaby => (390.0, 82_000.0),
        Build::V847 => (510.0, 100_000.0),
    };

    // --- the active filter ---------------------------------------------------------
    net.resistor(into, "r1", 68_000.0) // R1
        .capacitor("r1", "b1", 0.01e-6) // C1
        .resistor("b1", "loop", 1_500.0) // R2
        .bipolar("c1", "b1", "e1", MPSA18) // Q1
        .resistor("v9", "c1", 22_000.0) // R3
        .resistor("e1", "gnd", r4) // R4
        .resistor("c1", "tank", 470_000.0) // R6
        .inductor("tank", "lw", L1) // L1
        .resistor("lw", "loop", L1_WINDING)
        .resistor("tank", "loop", 33_000.0) // R7
        .capacitor("tank", "gnd", 4.7e-6) // C3
        .resistor("tank", "gnd", r8); // R8

    // --- the treadle and the output stage -----------------------------------------
    net.capacitor("c1", OUTPUT, 0.22e-6) // C5
        .resistor(OUTPUT, "gnd", load);
    let (top, bottom) = treadle_halves(0.5);
    let made_top = net.adjustable(OUTPUT, "wiper", Adjust::RealtimeResistor, top); // VR1
    let made_bottom = net.adjustable("wiper", "gnd", Adjust::RealtimeResistor, bottom);
    debug_assert_eq!((made_top, made_bottom), (TREADLE_TOP, TREADLE_BOTTOM));
    net.capacitor("wiper", "b2", 0.22e-6) // C4
        .resistor("c1", "b2", 470_000.0) // R5
        .bipolar("c2", "b2", "e2", MPSA18) // Q2
        .resistor("v9", "c2", 1_000.0) // R9
        .resistor("e2", "gnd", 10_000.0) // R10
        .capacitor("e2", "loop", 0.01e-6); // C2

    net.build(at)
}
