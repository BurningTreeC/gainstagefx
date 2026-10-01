//! The Oregon T preamplifier: an early-70s American 150 W valve head.
//!
//! Engineering reference (developer documentation, not panel text): Sunn
//! Model T, Sunn Musical Equipment Company drawing D-1029 revision A, ECN 441,
//! 25 September 1973, with its voltage chart. See `docs/models/oregon_t.md`.
//!
//! Three 12AX7s, two of them here. V1 is two channels, BRITE and NORMAL, on
//! one shared cathode (R5 680 ohm, C1 250 uF) -- so both halves are built, as
//! the one not played sets the other's bias. Each couples through .022 uF to a
//! 1 M **linear** volume, and the two meet at V2's grid through 470 k each,
//! the BRITE channel's with C4 470 pF across it. V2's first half is a
//! bypassed gain stage, its second a direct-coupled cathode follower, and the
//! follower drives a Marshall-family stack -- 270 pF treble capacitor, 56 k
//! slope, a 25 k middle -- whose treble wiper goes into **R19, the master**,
//! a 1 M linear pot, before the phase inverter. Sunn's notes put every pot
//! here on a linear track: R8, R9 and R19 and the three tone controls.
//!
//! The channel played is BRITE, from J1, with NORMAL's volume at zero. The
//! drawing's own voltage chart was taken from the BOTH IN jack (J3) with both
//! volumes up, and `tap_with(Input::Both, ..)` builds that for the tests that
//! check it. NORMAL's volume is a control of its own (`NORMAL_VOLUME`), with
//! no panel knob. The matched power stage is `power::PowerSpec::OREGON_6550`.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// R8, the BRITE channel's 1 M linear volume. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// R15, TREBLE, 250 k linear.
pub const TREBLE: usize = 1;
/// R16, BASS, 1 M linear, wired as a variable resistor.
pub const BASS: usize = 2;
/// R18, MID, 25 k linear.
pub const MIDDLE: usize = 3;
/// R19, VOL: the master, 1 M linear, between the treble wiper and the phase
/// inverter. The Level knob turns it.
pub const MASTER: usize = 4;
/// R9, the NORMAL channel's 1 M linear volume. No panel knob; it rests at
/// zero, and the BOTH IN tests turn it up.
pub const NORMAL_VOLUME: usize = 5;

/// Where the master rests when the panel's Level knob is at noon: half way,
/// a linear track's middle. ESTIMATED.
pub const MASTER_REST: f64 = 0.5;

/// C, the preamplifier's rail: B, 419 V on the chart with no signal, behind
/// R47 10 k with C20 on it. B is taken as stiff (it is fed through R46 10 k
/// from A, and also feeds the inverter, which is solved in the power stage).
pub const B_NODE: f64 = 419.0;

/// The second section of the C19/C20 can, "60-80 uF 450 V": the 80 uF one on
/// C. Which section is which is not marked; PLAUSIBLE.
pub const C_RESERVOIR: f64 = 80e-6;

/// V2's cathode bypass, printed "22" beside an electrolytic with its unit cut
/// off. Read as 22 uF (PLAUSIBLE), a full bypass above 9 Hz with R13.
pub const V2_BYPASS: f64 = 22e-6;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// Which input jack the guitar is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// J1, BRITE IN. NORMAL's input is shorted to ground by J3's and J4's
    /// switches, through R4.
    Brite,
    /// J3, BOTH IN: both channels from one jack, as the voltage chart was
    /// measured.
    Both,
}

/// The preamplifier from BRITE IN to the master's wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap_with(Input::Brite, source, load, "out")
}

/// The same preamplifier brought out at a chosen node.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    tap_with(Input::Brite, source, load, at)
}

/// Either input, brought out at a chosen node.
pub fn tap_with(input: Input, source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Oregon T preamp");

    // --- supply ----------------------------------------------------------------
    net.supply("c", 10_000.0, B_NODE) // R47
        .capacitor("c", "gnd", C_RESERVOIR);

    // --- inputs and V1 ---------------------------------------------------------
    // R1 / R4 68 k into the grids, R2 / R3 1 M from each grid to ground, both
    // cathodes on R5 680 ohm with C1 250 uF across it, R6 / R7 100 k from C.
    net.input("in", source)
        .resistor("in", "v1a_g", 68_000.0) // R1
        .resistor("v1a_g", "gnd", 1_000_000.0) // R2
        .resistor("v1b_g", "gnd", 1_000_000.0); // R3
    match input {
        Input::Brite => net.resistor("v1b_g", "gnd", 68_000.0), // R4, shorted input
        Input::Both => net.resistor("in", "v1b_g", 68_000.0),   // R4
    };
    net.resistor("v1_k", "gnd", 680.0) // R5
        .capacitor("v1_k", "gnd", 250e-6) // C1
        .resistor("c", "v1a_p", 100_000.0) // R6
        .resistor("c", "v1b_p", 100_000.0) // R7
        .triode("v1a_p", "v1a_g", "v1_k", ECC83)
        .triode("v1b_p", "v1b_g", "v1_k", ECC83);

    // --- the volumes and the mixer ----------------------------------------------
    // C2 / C3 .022 uF into R8 / R9, each wiper through 470 k (R10, with C4
    // 470 pF across it; R11) to V2's grid.
    net.capacitor("v1a_p", "brite_top", 0.022e-6) // C2
        .pot(
            "brite_top",
            "brite",
            "gnd",
            1_000_000.0,
            Taper::Linear,
            VOLUME,
        ) // R8
        .resistor("brite", "v2_g", 470_000.0) // R10
        .capacitor("brite", "v2_g", 470e-12) // C4
        .capacitor("v1b_p", "normal_top", 0.022e-6) // C3
        .rest(NORMAL_VOLUME, 0.0)
        .pot(
            "normal_top",
            "normal",
            "gnd",
            1_000_000.0,
            Taper::Linear,
            NORMAL_VOLUME,
        ) // R9
        .resistor("normal", "v2_g", 470_000.0); // R11

    // --- V2 ------------------------------------------------------------------------
    // The gain stage: R12 100 k from C, R13 820 ohm bypassed. The follower:
    // grid on the gain stage's plate, plate on C, R14 100 k.
    net.resistor("v2_k", "gnd", 820.0) // R13
        .capacitor("v2_k", "gnd", V2_BYPASS)
        .resistor("c", "v2_p", 100_000.0) // R12
        .triode("v2_p", "v2_g", "v2_k", ECC83)
        .resistor("cf", "gnd", 100_000.0) // R14
        .triode("c", "v2_p", "cf", ECC83);

    // --- the tone stack ------------------------------------------------------------
    // C5 270 pF to the top of R15, R17 56 k to the slope node, C6 .022 to the
    // bottom of R15 and the top of R16, C7 .022 to R18's wiper. R16 is a
    // rheostat: turned up it puts resistance in, the reverse law.
    net.capacitor("cf", "t_top", 270e-12) // C5
        .resistor("cf", "slope", 56_000.0) // R17
        .pot("t_top", "t_w", "t_bot", 250_000.0, Taper::Linear, TREBLE) // R15
        .capacitor("slope", "t_bot", 0.022e-6) // C6
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            1_000_000.0,
            Taper::ReverseLinear,
            BASS,
        ) // R16
        .pot("b_bot", "m_w", "gnd", 25_000.0, Taper::Linear, MIDDLE) // R18
        .capacitor("slope", "m_w", 0.022e-6); // C7

    // --- the master ------------------------------------------------------------------
    // R19 1 M linear, the treble wiper on its top.
    net.rest(MASTER, MASTER_REST)
        .pot("t_w", "out", "gnd", 1_000_000.0, Taper::Linear, MASTER)
        .resistor("out", "gnd", load);

    net.build(at)
}
