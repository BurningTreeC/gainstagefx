//! The Brit Plexi Bass preamplifier: a late-60s British 100 W bass head, the
//! Brit Plexi's sibling of the same drawing set.
//!
//! Engineering reference (developer documentation, not panel text): Marshall
//! JMP 1992 Super Bass, as drawn by Unicord, drawing 70-13-11, July 1970 (3 x
//! ECC83, 4 x EL34) -- the same set and month as the 1959 drawing
//! `circuits::plexi` is built from. Pot laws from Marshall's 1988 1959 sheet,
//! as for the Brit Plexi. See `docs/models/brit_plexi_bass.md`.
//!
//! The same node structure as the Brit Plexi and the same supply chain, with
//! the 1992's values, and the differences are the whole amplifier:
//!
//! - **V1's two halves share one cathode**, 820 ohm with a bypass electrolytic,
//!   where the 1959 gives its bright half a cool 2.7 k / .68 uF of its own. So
//!   both halves are built: the one not played still draws its current through
//!   the shared resistor and sets the bias of the one that is.
//! - **.022 uF couplings on both channels** and **no bright capacitor** round
//!   the volume: nothing here throws the bass away before the first stage has
//!   amplified it.
//! - **V2a unbypassed**: 820 ohm and no capacitor across it, so the second
//!   stage has about half the 1959's gain.
//! - **A 250 pF / 56 k stack** where the 1959 has 500 pF / 33 k, so the
//!   treble control reaches less and the bass more.
//!
//! The channel built is the one with 500 pF across its 470 k mixer -- the
//! 1959's bright channel's position -- played from its HIGH input, with the
//! other channel's volume at zero. Then, as on the 1959, there is no master:
//! the treble wiper goes straight to the phase inverter. Its matched power
//! stage is `power::PowerSpec::PLEXI_BASS_EL34`.

use crate::circuits::plexi::{INVERTER_DROPPER, INVERTER_IDLE, SCREEN_NODE};
use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// The played channel's VOLUME, 1 M log. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// TREBLE, 250 k (lin on Marshall's 1988 1959 sheet).
pub const TREBLE: usize = 1;
/// BASS, 1 M log, wired as a variable resistor.
pub const BASS: usize = 2;
/// MIDDLE, 25 k (lin).
pub const MIDDLE: usize = 3;

/// The inverter node, which `power::PowerSpec::PLEXI_BASS_EL34` is fed from:
/// the 1959's chain behind the same 20 k dropper, with this preamplifier's
/// current through it. Re-measured by `tests/plexi_bass.rs`.
pub const INVERTER_NODE: f64 = 320.0;

/// V1's shared cathode bypass. Unicord prints "320" with no unit beside the
/// electrolytic's symbol; at any value from tens of microfarads up it is a full
/// bypass above 20 Hz. Read as 320 uF, PLAUSIBLE (the 1959's normal channel has
/// 250 uF in the same place).
pub const V1_BYPASS: f64 = 320e-6;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The preamplifier from the played channel's HIGH input to the treble wiper.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit Plexi Bass preamp");

    // --- supplies ------------------------------------------------------------
    // The 1959's chain, part for part on the drawing: screen node -- 20 k 1 W
    // -- inverter node (50 uF) -- 10 k 1 W -- V2 node (50 uF) -- 10 k 1 W --
    // V1 node (50 uF). Both of V1's halves are built, so nothing stands in for
    // the second one's current.
    net.supply("pin", INVERTER_DROPPER, SCREEN_NODE)
        .capacitor("pin", "gnd", 50e-6)
        .resistor("pin", "gnd", INVERTER_IDLE)
        .resistor("pin", "v2n", 10_000.0)
        .capacitor("v2n", "gnd", 50e-6)
        .resistor("v2n", "v1n", 10_000.0)
        .capacitor("v1n", "gnd", 50e-6);

    // --- the HIGH input and V1, both halves -------------------------------------
    // 1 M across the jack; with nothing in LOW its switch parallels the two
    // 68 k. The other channel's grid sits on its own 68 k pair with nothing
    // plugged in, so it is at ground for signal and for direct current alike.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0)
        .resistor("in", "v1_g", 34_000.0) // 68 k || 68 k
        .resistor("v1_k", "gnd", 820.0)
        .capacitor("v1_k", "gnd", V1_BYPASS)
        .resistor("v1n", "v1_p", 100_000.0)
        .triode("v1_p", "v1_g", "v1_k", ECC83)
        .resistor("v1b_g", "gnd", 34_000.0)
        .resistor("v1n", "v1b_p", 100_000.0)
        .triode("v1b_p", "v1b_g", "v1_k", ECC83);

    // --- the volume and the mixer -----------------------------------------------
    // .022 uF into the top of the 1 M track (no bright capacitor), the wiper
    // through 470 k || 500 pF to V2a's grid, and the other channel's 470 k from
    // that grid to its volume wiper, which is at ground. The idle half's own
    // .022 and volume track hang on its plate and carry nothing.
    net.capacitor("v1_p", "vol_top", 0.022e-6)
        .pot("vol_top", "vol", "gnd", 1_000_000.0, Taper::Audio, VOLUME)
        .resistor("vol", "v2_g", 470_000.0)
        .capacitor("vol", "v2_g", 500e-12)
        .resistor("v2_g", "gnd", 470_000.0);

    // --- V2a -----------------------------------------------------------------------
    // 820 ohm ("1K" in brackets: the later value), no capacitor drawn.
    net.resistor("v2_k", "gnd", 820.0)
        .resistor("v2n", "v2_p", 100_000.0)
        .triode("v2_p", "v2_g", "v2_k", ECC83);

    // --- the cathode follower --------------------------------------------------
    net.resistor("cf", "gnd", 100_000.0)
        .triode("v2n", "v2_p", "cf", ECC83);

    // --- the tone stack ------------------------------------------------------------
    // 250 pF to the top of Treble, 56 k to the slope node, .022 uF from there to
    // the bottom of Treble and the top of Bass, .022 uF to the Middle wiper.
    net.capacitor("cf", "t_top", 250e-12)
        .resistor("cf", "slope", 56_000.0)
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
