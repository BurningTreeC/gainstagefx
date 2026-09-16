//! The Brit Plexi preamplifier: the bright channel of a late-60s British 100 W
//! lead amplifier with no master volume.
//!
//! Engineering reference (developer documentation, not panel text): Marshall JMP
//! 1959 Super Lead, as drawn by Unicord, drawing 70-6-11 issue B, July 1970
//! (3 x ECC83, 4 x EL34). Pot laws from Marshall's "1959 STD Preamp" drawing,
//! issue 4, 18-5-88; supply voltages from Marshall's c. 1967 100 W drawing. See
//! `docs/models/brit_plexi.md`.
//!
//! Three triodes, and none of them wasted. The bright channel's first stage runs
//! cool on 2.7 k with only .68 uF across it, and its .0022 uF coupling cap throws
//! away the bass before anything else happens -- the reverse of the normal
//! channel, which is why the two sound so different. The 5000 pF bright cap round
//! the volume pot passes top end even with the control low. The mixer hands V2a
//! half the signal through 470 k into the other channel's 470 k, with 500 pF
//! across it keeping the treble. V2a is fully bypassed and hot, and the cathode
//! follower behind it is direct-coupled and drives the stack.
//!
//! Then there is no master. The treble wiper goes straight to the phase inverter,
//! so the Volume is the only thing deciding how hard the power valves are driven,
//! and a plexi turned up is a power stage clipping. Its matched power stage is
//! `power::PowerSpec::PLEXI_EL34`.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// VOLUME I, the bright channel's 1 M log. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// TREBLE, 250 k (lin on the 1988 sheet).
pub const TREBLE: usize = 1;
/// BASS, 1 M log, wired as a variable resistor.
pub const BASS: usize = 2;
/// MIDDLE, 25 k (lin on the 1988 sheet).
pub const MIDDLE: usize = 3;

/// The node the whole low-current supply chain hangs from: the screens' supply,
/// 455 V (Marshall's c. 1967 100 W drawing has 460 V at the plates). ESTIMATED.
pub const SCREEN_NODE: f64 = 455.0;

/// Unicord's 20 k 1 W from the screen node to the inverter node.
pub const INVERTER_DROPPER: f64 = 20_000.0;

/// The phase inverter's idle plate current, as a resistance on its node: the
/// inverter is solved in the power stage's netlist, but its current flows
/// through this supply chain. APPROXIMATED; see `examples/plexi_op.rs`.
pub const INVERTER_IDLE: f64 = 136_000.0;

/// The inverter node that results, which the power stage's inverter is fed
/// from. Re-measured by `tests/plexi.rs`.
pub const INVERTER_NODE: f64 = 320.0;

/// The normal channel's first triode, which is not built: its volume is down,
/// so it passes nothing, but it still draws its idle current through the V1
/// node's dropper. That current, as a resistance, measured on the model's own
/// ECC83 with 100 k and 820 ohm (1.25 mA at 300 V). APPROXIMATED.
pub const NORMAL_V1_IDLE: f64 = 240_000.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the bright channel's HIGH input to the treble wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit Plexi preamp");

    // --- supplies ------------------------------------------------------------
    // Screen node -- 20 k 1 W -- inverter node (50 uF) -- 10 k 1 W -- V2 node
    // (50 uF) -- 10 k 1 W -- V1 node (50 uF).
    net.supply("pin", INVERTER_DROPPER, SCREEN_NODE)
        .capacitor("pin", "gnd", 50e-6)
        .resistor("pin", "gnd", INVERTER_IDLE)
        .resistor("pin", "v2n", 10_000.0)
        .capacitor("v2n", "gnd", 50e-6)
        .resistor("v2n", "v1n", 10_000.0)
        .capacitor("v1n", "gnd", 50e-6)
        .resistor("v1n", "gnd", NORMAL_V1_IDLE);

    // --- the HIGH input and V1b ------------------------------------------------
    // 1 M across the jack. With nothing in LOW its switch puts the two 68 k in
    // parallel, so the grid sees 34 k and no attenuation.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0)
        .resistor("in", "v1_g", 34_000.0) // 68 k || 68 k
        .resistor("v1_k", "gnd", 2_700.0)
        .capacitor("v1_k", "gnd", 0.68e-6)
        .resistor("v1n", "v1_p", 100_000.0)
        .triode("v1_p", "v1_g", "v1_k", ECC83);

    // --- VOLUME I and the mixer -----------------------------------------------
    // .0022 uF into the top of the 1 M track, .005 uF from the top to the wiper,
    // the wiper through 470 k || 500 pF to V2a's grid, and the normal channel's
    // 470 k from that grid to its volume wiper, which is at ground.
    net.capacitor("v1_p", "vol_top", 0.0022e-6)
        .pot("vol_top", "vol", "gnd", 1_000_000.0, Taper::Audio, VOLUME)
        .capacitor("vol_top", "vol", 0.005e-6)
        .resistor("vol", "v2_g", 470_000.0)
        .capacitor("vol", "v2_g", 500e-12)
        .resistor("v2_g", "gnd", 470_000.0);

    // --- V2a -----------------------------------------------------------------------
    // 820 ohm (1 k on some units) with .68 uF across it.
    net.resistor("v2_k", "gnd", 820.0)
        .capacitor("v2_k", "gnd", 0.68e-6)
        .resistor("v2n", "v2_p", 100_000.0)
        .triode("v2_p", "v2_g", "v2_k", ECC83);

    // --- the cathode follower --------------------------------------------------
    // Grid on V2a's plate, plate on the V2 node, 100 k from the cathode to ground.
    net.resistor("cf", "gnd", 100_000.0)
        .triode("v2n", "v2_p", "cf", ECC83);

    // --- the tone stack ------------------------------------------------------------
    // 500 pF to the top of Treble, 33 k to the slope node, .022 uF from there to
    // the bottom of Treble and the top of Bass, .022 uF to the Middle wiper. Bass
    // is a variable resistor and turned up puts resistance in: the reverse law.
    net.capacitor("cf", "t_top", 500e-12)
        .resistor("cf", "slope", 33_000.0)
        .pot("t_top", "out", "t_bot", 250_000.0, Taper::Linear, TREBLE)
        .capacitor("slope", "t_bot", 0.022e-6)
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        )
        .pot("b_bot", "m_w", "gnd", 25_000.0, Taper::Linear, MIDDLE)
        .capacitor("slope", "m_w", 0.022e-6)
        .resistor("out", "gnd", load);

    net.build(at)
}
