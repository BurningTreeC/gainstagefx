//! Fender Twin Reverb AB763, Vibrato channel.
//!
//! All electrically coupled channel/reverb/tremolo stages live in one MNA
//! circuit. The only deliberate split is the spring's mechanical propagation:
//! the transformer secondary is read from `SEND`, `dsp::spring::Tank` advances
//! the delayed mechanical state, and its pickup voltage re-enters this same
//! circuit through `TANK_RETURN_AUX`. That keeps the real V2B loading, shared
//! V4 cathode, dry/wet mixer and optical shunt electrically coupled.
//!
//! The phase inverter/power stage remains the separate `power::TWIN` circuit
//! because GainStageFX intentionally permits the power amplifier to be switched
//! independently. Its AB763 values and shared A/B/C supply chain are modelled in
//! `circuits::power`; the hand-off here is the real channel-mix node.

use crate::dsp::netlist::{Adjust, Circuit, CoreSpec, Fault, Netlist, Taper, TriodeSpec};

/// The tone stack, in the order the panel has them.
pub const TREBLE: usize = 0;
pub const BASS: usize = 1;
pub const MIDDLE: usize = 2;
/// The channel volume, 1 M audio.
pub const VOLUME: usize = 3;
/// 100 k linear on the AB763 drawing.
pub const REVERB: usize = 4;
/// Stock 50 kΩ reverse-audio Vibrato Intensity control.
pub const INTENSITY: usize = 5;

/// Adjustable slots for the two stock Vibrato-channel input jacks. Jack 1
/// (High) parallels the pair of 68 k grid stoppers to 34 k and presents 1 MΩ
/// at the jack. Jack 2 (Low) uses one 68 k in series and the other 68 k to
/// ground, making the familiar roughly -6 dB divider and 136 kΩ input.
pub const INPUT_SERIES_SLOT: usize = 0;
pub const INPUT_JACK_LOAD_SLOT: usize = 1;
pub const INPUT_GRID_SHUNT_SLOT: usize = 2;
pub const INPUT_HIGH_SERIES_OHMS: f64 = 34_000.0;
pub const INPUT_HIGH_JACK_LOAD_OHMS: f64 = 1_000_000.0;
pub const INPUT_LOW_SERIES_OHMS: f64 = 68_000.0;
pub const INPUT_LOW_GRID_SHUNT_OHMS: f64 = 68_000.0;
/// A numerically finite open switch. Twelve orders above the source network is
/// electrically irrelevant here while keeping the matrix well conditioned.
pub const INPUT_OPEN_OHMS: f64 = 1.0e12;
/// Bright switch capacitor and audio-rate optocoupler slots in the unified circuit.
pub const BRIGHT_CAP_SLOT: usize = 3;
pub const LDR_SLOT: usize = 4;
pub const BRIGHT_CAP_FARADS: f64 = 120e-12;
pub const BRIGHT_OFF_FARADS: f64 = 1.0e-18;
/// The spring pickup is the sole intentional break in the electrical netlist.
pub const TANK_RETURN_AUX: usize = 0;
/// Useful diagnostic nodes in the unified circuit.
pub const V4B_GRID: &str = "mix";
pub const V4B_TO_PI: &str = "channel_mix";

const V7025: TriodeSpec = TriodeSpec::ECC83;
const V12AT7: TriodeSpec = TriodeSpec::ECC81;

/// Mechanical tank terminals.
pub const SEND: &str = "tank_in";
pub const RETURN: &str = "tank_out";
/// V2-B plate: the AB763 takes the 500 pF reverb send directly from here,
/// before the .02 uF dry-path coupling capacitor.
pub const REVERB_SEND_PLATE: &str = "v2_p";

// --- AB763 reverb driver / 022921 transformer ---------------------------
// The 022921 / 125A20B family is nominally 25 kOhm : 8 Ohm at 3.5 W.
// Hammond's 1750A replacement publishes 1065 Ohm primary DCR and 31 H
// primary inductance.  Those measured values are used directly here.
const REVERB_SEND_COUPLING: f64 = 500e-12;
const REVERB_DRIVER_GRID_LEAK: f64 = 1_000_000.0;
const REVERB_DRIVER_CATHODE: f64 = 2_200.0;
const REVERB_DRIVER_BYPASS: f64 = 25e-6;
const REVERB_XFMR_RATIO: f64 = 55.901_699_437_494_74;
const REVERB_XFMR_PRIMARY_DCR: f64 = 1_065.0;
const REVERB_XFMR_SECONDARY_DCR: f64 = 0.966;
const REVERB_XFMR_PRIMARY_L: f64 = 31.0;
// A simple series-leakage element is fitted to the published 70 Hz-15 kHz
// response instead of copying the transformer's 1.54 H short-circuit test
// number literally; doing that in a one-lump model would create an artificial
// ~2.5 kHz roll-off that the measured transformer does not have.
const REVERB_XFMR_EFFECTIVE_LEAKAGE: f64 = 50e-6;
const REVERB_TANK_INPUT_Z: f64 = 8.0;

const DRY_MIX_R: f64 = 3_300_000.0;
const DRY_BYPASS_C: f64 = 10e-12;
const WET_MIX_R: f64 = 470_000.0;
const MIX_GRID_LEAK: f64 = 220_000.0;
// After V4-B the stock AB763 Vibrato channel does not drive the PI directly.
// The .1 uF coupling cap lands on the 50 k reverse-audio Intensity pot, whose
// end-to-end resistance is a permanent load to ground, and the signal then
// crosses the 220 k channel-mixing resistor on its way to the PI.  The LDR
// moves the pot wiper, not the end-to-end 50 k load, so keeping this fixed load
// here is correct; the optical model applies the extra wiper/LDR loading
// relative to this permanent end-to-end pot load.
const CHANNEL_MIX_R: f64 = 220_000.0;

/// Build the AB763 Vibrato-channel signal path as one electrically coupled
/// network. The spring itself remains a mechanical delay outside MNA; its
/// pickup re-enters through `TANK_RETURN_AUX`, so the recovery/mixer stages
/// still load and interact with the dry channel in the same Newton solve.
fn complete_netlist(source: f64, load: f64) -> Netlist {
    let mut net = Netlist::new("Twin Reverb AB763 complete Vibrato channel");

    // Stock switched input jacks.
    net.input("jack", source);
    let input_series = net.adjustable("jack", "v1_g", Adjust::Resistor, INPUT_HIGH_SERIES_OHMS);
    let jack_load = net.adjustable("jack", "gnd", Adjust::Resistor, INPUT_HIGH_JACK_LOAD_OHMS);
    let grid_shunt = net.adjustable("v1_g", "gnd", Adjust::Resistor, INPUT_OPEN_OHMS);
    debug_assert_eq!(input_series, INPUT_SERIES_SLOT);
    debug_assert_eq!(jack_load, INPUT_JACK_LOAD_SLOT);
    debug_assert_eq!(grid_shunt, INPUT_GRID_SHUNT_SLOT);

    // Shared AB763 downstream supply nodes. This module starts at the B rail
    // because the switchable power module owns the A reservoir/choke. From B
    // onward the original 1 k / 4.7 k droppers and 20 uF filters are common to
    // the reverb driver and all channel plate loads. The 104 ohm source
    // resistance is the measured DCR of a 125C1A replacement choke.
    net.supply("bplus_b", 104.0, 458.0)
        .capacitor("bplus_b", "gnd", 20e-6)
        .resistor("bplus_b", "bplus_c", 1_000.0)
        .capacitor("bplus_c", "gnd", 20e-6)
        .resistor("bplus_c", "bplus_d", 4_700.0)
        .capacitor("bplus_d", "gnd", 20e-6);

    // V1A, Vibrato channel first gain stage. AB763 uses 1.5 kΩ / 25 µF.
    net.resistor("v1_k", "gnd", 1_500.0)
        .capacitor("v1_k", "gnd", 25e-6)
        .resistor("bplus_d", "v1_p", 100_000.0)
        .triode("v1_p", "v1_g", "v1_k", V7025);

    // Fender three-control stack.
    net.capacitor("v1_p", "t_top", 250e-12)
        .pot("t_top", "ts_out", "t_bot", 250_000.0, Taper::Audio, TREBLE)
        .resistor("v1_p", "slope", 100_000.0)
        .capacitor("slope", "t_bot", 0.1e-6)
        .capacitor("slope", "b_bot", 0.047e-6)
        .pot("t_bot", "b_bot", "b_bot", 250_000.0, Taper::ReverseAudio, BASS)
        .pot("b_bot", "gnd", "gnd", 10_000.0, Taper::ReverseAudio, MIDDLE)
        .pot("ts_out", "vol", "gnd", 1_000_000.0, Taper::Audio, VOLUME);

    // Bright switch: the real 120 pF top-to-wiper capacitor, switchable
    // without changing topology. Existing sessions default to the old "on"
    // behaviour; the plugin exposes the switch explicitly now.
    let bright = net.adjustable("ts_out", "vol", Adjust::Capacitor, BRIGHT_CAP_FARADS);
    debug_assert_eq!(bright, BRIGHT_CAP_SLOT);

    // V2B is DC-coupled from the Volume wiper. The .02 µF capacitor belongs
    // on the *plate output* feeding the dry/reverb mixer, not at this grid.
    // V2B shares the stock 820 Ω / 25 µF cathode network with the otherwise
    // quiet Normal-channel second stage; include that half so the shared
    // cathode bias/current is electrically correct even though this plugin
    // exposes the Vibrato channel only.
    net.resistor("v2_k", "gnd", 820.0)
        .capacitor("v2_k", "gnd", 25e-6)
        .resistor("bplus_d", "v2_p", 100_000.0)
        .triode("v2_p", "vol", "v2_k", V7025)
        .resistor("normal2_g", "gnd", 1_000_000.0)
        .resistor("bplus_d", "normal2_p", 100_000.0)
        .triode("normal2_p", "normal2_g", "v2_k", V7025);

    // Dry branch: .02 µF coupling, then the characteristic 3.3 MΩ resistor
    // with 10 pF bypass directly into the V4B mixing grid.
    net.capacitor("v2_p", "dry_pre", 0.02e-6)
        .resistor("dry_pre", "mix", DRY_MIX_R)
        .capacitor("dry_pre", "mix", DRY_BYPASS_C)
        .resistor("mix", "gnd", MIX_GRID_LEAK);

    // Reverb send is taken directly from the V2B plate. Keeping the driver
    // here means its 500 pF / 1 MΩ grid network and the paralleled 12AT7
    // actually load V2B instead of being replaced by a Thevenin guess.
    let core = CoreSpec {
        henry: REVERB_XFMR_PRIMARY_L * 40.0
            / (REVERB_XFMR_RATIO * REVERB_XFMR_RATIO),
        knee: 0.017,
        sharpness: 7.0,
    };
    net.capacitor("v2_p", "drv_g", REVERB_SEND_COUPLING)
        .resistor("drv_g", "gnd", REVERB_DRIVER_GRID_LEAK)
        .resistor("drv_k", "gnd", REVERB_DRIVER_CATHODE)
        .capacitor("drv_k", "gnd", REVERB_DRIVER_BYPASS)
        .resistor("bplus_b", "xfmr_top", REVERB_XFMR_PRIMARY_DCR)
        .inductor("xfmr_top", "drv_p", REVERB_XFMR_PRIMARY_L)
        .transformer("xfmr_top", "drv_p", "xfmr_sec", "gnd", REVERB_XFMR_RATIO)
        .core("xfmr_sec", "gnd", core)
        .resistor("xfmr_sec", "sec_copper", REVERB_XFMR_SECONDARY_DCR)
        .inductor("sec_copper", SEND, REVERB_XFMR_EFFECTIVE_LEAKAGE)
        .resistor(SEND, "gnd", REVERB_TANK_INPUT_Z)
        .triode("drv_p", "drv_g", "drv_k", V12AT7)
        .triode("drv_p", "drv_g", "drv_k", V12AT7);

    // Mechanical tank pickup returns as an independent voltage source into
    // the same circuit. V4A and V4B share the real 820 Ω / 25 µF cathode
    // network, something the previous split simulations could not represent.
    // 4AB3C1B output transducer: 2.25 kΩ nominal at 1 kHz.
    let tank_return = net.aux_input(RETURN, 2_250.0);
    debug_assert_eq!(tank_return, TANK_RETURN_AUX);
    net.resistor(RETURN, "gnd", 220_000.0)
        .resistor("v4_k", "gnd", 820.0)
        .capacitor("v4_k", "gnd", 25e-6)
        .resistor("bplus_d", "v4a_p", 100_000.0)
        .triode("v4a_p", RETURN, "v4_k", V7025)
        .capacitor("v4a_p", "rv_top", 0.0033e-6)
        .pot("rv_top", "rv_wiper", "gnd", 100_000.0, Taper::Linear, REVERB)
        .resistor("rv_wiper", "mix", WET_MIX_R)
        .resistor("bplus_d", "v4b_p", 100_000.0)
        .triode("v4b_p", "mix", "v4_k", V7025);

    // Stock optical tremolo network at the V4B hand-off. The 50 kΩ
    // reverse-audio pot is a real circuit element; the LDR is an audio-rate
    // variable resistor stamped in Newton, so the roach changes the actual
    // node loading instead of multiplying the signal after the valve.
    net.capacitor("v4b_p", "intensity_top", 0.1e-6)
        .pot("intensity_top", "intensity_wiper", "gnd", 50_000.0, Taper::ReverseAudio, INTENSITY);
    let ldr = net.adjustable("intensity_wiper", "gnd", Adjust::RealtimeResistor, 5_000_000.0);
    debug_assert_eq!(ldr, LDR_SLOT);
    // The unused Normal channel still loads the common channel-mix node in a
    // real Twin. Its quiet second triode above therefore continues through its
    // own coupling capacitor and 220 kΩ mixing resistor instead of being
    // replaced by a guessed shunt. The Vibrato channel enters through the
    // matching 220 kΩ resistor after the tremolo network.
    net.capacitor("normal2_p", "normal_mix", 0.047e-6)
        .resistor("normal_mix", "channel_mix", CHANNEL_MIX_R)
        .resistor("intensity_top", "channel_mix", CHANNEL_MIX_R)
        .resistor("channel_mix", "gnd", load)
        .rest(INTENSITY, 1.0);

    net
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    complete_netlist(source, load).build(V4B_TO_PI)
}

/// Build the complete channel but expose an internal node as the output.
/// Used by diagnostics/tests without maintaining a second approximation of the
/// circuit.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    complete_netlist(source, load).build(at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::time::Simulation;

    #[test]
    fn twin_high_input_starts_with_the_stock_jack_values() {
        let circuit = build(10_000.0, 1_000_000.0).expect("Twin builds");
        let sim = Simulation::new(circuit, 48_000.0);
        assert_eq!(sim.value(INPUT_SERIES_SLOT), Some(INPUT_HIGH_SERIES_OHMS));
        assert_eq!(
            sim.value(INPUT_JACK_LOAD_SLOT),
            Some(INPUT_HIGH_JACK_LOAD_OHMS)
        );
        assert_eq!(sim.value(INPUT_GRID_SHUNT_SLOT), Some(INPUT_OPEN_OHMS));
    }
    #[test]
    fn complete_twin_contains_the_coupled_effects_nodes() {
        let circuit = build(10_000.0, 1_000_000.0).expect("complete Twin builds");
        assert!(circuit.unknown_named(REVERB_SEND_PLATE).is_some());
        assert!(circuit.unknown_named(SEND).is_some());
        assert!(circuit.unknown_named(RETURN).is_some());
        assert!(circuit.unknown_named(V4B_GRID).is_some());
        assert!(circuit.unknown_named(V4B_TO_PI).is_some());
        assert_eq!(circuit.aux_inputs, 1);
    }

    #[test]
    fn complete_twin_uses_minimal_exact_nonlinear_schur_core() {
        let circuit = build(10_000.0, 1_000_000.0).expect("complete Twin builds");
        let sim = Simulation::new(circuit, 48_000.0);
        let reduction = sim
            .nonlinear_reduction()
            .expect("complete Twin should use exact nonlinear Schur reduction");
        println!(
            "twin_channel_schur,unknowns={},boundary={},internal={},boundary_names={:?}",
            sim.unknowns(),
            reduction.0,
            reduction.1,
            sim.nonlinear_boundary_names()
        );
        assert_eq!(sim.unknowns(), 37, "unexpected complete Twin MNA size");
        assert_eq!(
            reduction,
            (18, 19),
            "Twin channel exact Schur core changed: nonlinear and audio-rate matrix terminals must remain on the Newton boundary"
        );
    }

}
