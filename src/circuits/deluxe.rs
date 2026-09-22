//! Fender Deluxe Reverb AB763, Vibrato channel.
//!
//! Read value by value off the original Fender drawing at 600 dpi; the sheet is
//! in `docs/schematics/fender_deluxe_reverb_ab763.pdf` and the checkpoint is
//! `docs/models/american_deluxe.md`.
//!
//! Built the same way as `circuits::twin`, because it is the same circuit at a
//! different size: one MNA network carrying the channel, the reverb send and
//! recovery, the mixer and the optical tremolo, with the spring's mechanical
//! propagation as the only deliberate break. The phase inverter and output
//! stage stay in `circuits::power` as `PowerSpec::DELUXE_6V6` so the power
//! amplifier can still be switched independently.
//!
//! What differs from the Twin, all of it visible on the drawing:
//!
//! * **no Middle control.** A fixed 6.8 kΩ leg where the Twin has a 10 kΩ pot.
//! * **47 pF bright capacitor** across the Volume, not 120 pF, and not switched:
//!   the Deluxe's panel has no Bright switch.
//! * **lower rails.** Preamp plates at +170/+180 V against the Twin's +270 V.
//! * **one 12AT7 reverb driver** at +410 V into TR4, and a recovery stage
//!   sharing its cathode with the mixer rather than with a second Normal-channel
//!   triode.
//! * the tremolo shunt hangs on the mixer plate through 220 kΩ *then* 0.1 µF,
//!   where the Twin couples first and mixes after.

use crate::dsp::netlist::{Adjust, Circuit, CoreSpec, Fault, Netlist, Taper, TriodeSpec};

/// The tone stack, in the order the panel has them. There is no Middle: the
/// AB763 Deluxe grounds the stack through a fixed 6.8 kΩ resistor.
pub const TREBLE: usize = 0;
pub const BASS: usize = 1;
/// The channel volume, 1 MΩ audio.
pub const VOLUME: usize = 2;
/// 100 kΩ linear on the drawing.
pub const REVERB: usize = 3;
/// Stock 50 kΩ reverse-audio Vibrato Intensity control.
pub const INTENSITY: usize = 4;

/// Adjustable slots for the two stock Vibrato-channel input jacks. Jack 1
/// (High) parallels the pair of 68 kΩ grid stoppers to 34 kΩ and presents 1 MΩ
/// at the jack. Jack 2 (Low) uses one 68 kΩ in series and the other to ground.
pub const INPUT_SERIES_SLOT: usize = 0;
pub const INPUT_JACK_LOAD_SLOT: usize = 1;
pub const INPUT_GRID_SHUNT_SLOT: usize = 2;
pub const INPUT_HIGH_SERIES_OHMS: f64 = 34_000.0;
pub const INPUT_HIGH_JACK_LOAD_OHMS: f64 = 1_000_000.0;
pub const INPUT_LOW_SERIES_OHMS: f64 = 68_000.0;
pub const INPUT_LOW_GRID_SHUNT_OHMS: f64 = 68_000.0;
/// A numerically finite open switch, as in `circuits::twin`.
pub const INPUT_OPEN_OHMS: f64 = 1.0e12;
/// The audio-rate optocoupler slot.
pub const LDR_SLOT: usize = 3;
/// The spring pickup is the sole intentional break in the electrical netlist.
pub const TANK_RETURN_AUX: usize = 0;

/// Mechanical tank terminals.
pub const SEND: &str = "tank_in";
pub const RETURN: &str = "tank_out";
/// The Vibrato second stage's plate, where the 500 pF reverb send is taken --
/// before the 0.02 µF dry coupling, exactly as on the Twin.
pub const REVERB_SEND_PLATE: &str = "v2_p";
/// Useful diagnostic nodes.
pub const MIX_GRID: &str = "mix";
/// The mixer plate, which is what the 0.001 µF in `PowerSpec::DELUXE_6V6`
/// couples to. On this drawing the two 220 kΩ mixing resistors and the tremolo
/// shunt all hang on this same node.
pub const MIXER_TO_PI: &str = "v4b_p";

const V7025: TriodeSpec = TriodeSpec::ECC83;
const V12AT7: TriodeSpec = TriodeSpec::ECC81;

// Published ECC83 inter-electrode capacitances, for the same reason the Twin
// carries them: the triode model is otherwise memoryless, and the bright
// capacitor deliberately feeds fast transients around the Volume pot.
const ECC83_CAG: f64 = 1.6e-12;
const ECC83_CGK: f64 = 1.6e-12;
const ECC83_CAK: f64 = 0.33e-12;

// --- the tone stack, from the drawing -----------------------------------
const TREBLE_CAP: f64 = 250e-12;
const TREBLE_POT: f64 = 250_000.0;
const SLOPE_R: f64 = 100_000.0;
const BASS_CAP: f64 = 0.1e-6;
const BASS_POT: f64 = 250_000.0;
const MID_CAP: f64 = 0.047e-6;
/// Where the Twin has a Middle control, the Deluxe has this.
const MID_R: f64 = 6_800.0;
const VOLUME_POT: f64 = 1_000_000.0;
/// 47 pF, permanently across the Volume: the Deluxe has no Bright switch.
const BRIGHT_CAP: f64 = 47e-12;

// --- AB763 reverb driver and its 125A20B transformer --------------------
// TR4 on this drawing is the 125A20B, the same part the Twin uses, so the
// measured Hammond 1750A replacement data in `circuits::twin` describes this
// transformer too and is reused rather than re-estimated.
const REVERB_SEND_COUPLING: f64 = 500e-12;
const REVERB_DRIVER_GRID_LEAK: f64 = 1_000_000.0;
const REVERB_DRIVER_CATHODE: f64 = 2_200.0;
const REVERB_DRIVER_BYPASS: f64 = 25e-6;
const REVERB_XFMR_RATIO: f64 = 55.901_699_437_494_74;
const REVERB_XFMR_PRIMARY_DCR: f64 = 1_065.0;
const REVERB_XFMR_SECONDARY_DCR: f64 = 0.966;
const REVERB_XFMR_PRIMARY_L: f64 = 31.0;
const REVERB_XFMR_EFFECTIVE_LEAKAGE: f64 = 50e-6;
const REVERB_TANK_INPUT_Z: f64 = 8.0;
/// 4AB3C1B output transducer: 2.25 kΩ nominal at 1 kHz.
const TANK_PICKUP_Z: f64 = 2_250.0;

const RECOVERY_GRID_LEAK: f64 = 220_000.0;
const RECOVERY_COUPLING: f64 = 0.003e-6;
const REVERB_POT: f64 = 100_000.0;
const WET_MIX_R: f64 = 470_000.0;
const DRY_COUPLING: f64 = 0.02e-6;
const DRY_MIX_R: f64 = 3_300_000.0;
const DRY_BYPASS_C: f64 = 10e-12;
const MIX_GRID_LEAK: f64 = 220_000.0;
const CHANNEL_MIX_R: f64 = 220_000.0;
const NORMAL_COUPLING: f64 = 0.047e-6;
const TREMOLO_COUPLING: f64 = 0.1e-6;
const INTENSITY_POT: f64 = 50_000.0;

/// DERIVED. The drawing labels the inverter node +325 V and the reservoir node
/// +410 V across a 10 kOhm dropper, which is 8.5 mA through it; the preamplifier
/// below accounts for about five of that, so the inverter takes the rest.
const PI_STANDING_LOAD: f64 = 105_000.0;

const PLATE_LOAD: f64 = 100_000.0;
const FIRST_CATHODE: f64 = 1_500.0;
const FIRST_BYPASS: f64 = 25e-6;
/// The 820 Ω / 25 µF networks the drawing shares between two triodes each:
/// node [A] between the two channels' second stages, node [E] between the
/// reverb recovery stage and the mixer.
const SHARED_CATHODE: f64 = 820.0;
const SHARED_BYPASS: f64 = 25e-6;

/// Build the AB763 Deluxe Vibrato channel as one electrically coupled network.
fn complete_netlist(source: f64, load: f64) -> Netlist {
    let mut net = Netlist::new("Deluxe Reverb AB763 complete Vibrato channel");

    // Stock switched input jacks.
    net.input("jack", source);
    let input_series = net.adjustable("jack", "v1_g", Adjust::Resistor, INPUT_HIGH_SERIES_OHMS);
    let jack_load = net.adjustable("jack", "gnd", Adjust::Resistor, INPUT_HIGH_JACK_LOAD_OHMS);
    let grid_shunt = net.adjustable("v1_g", "gnd", Adjust::Resistor, INPUT_OPEN_OHMS);
    debug_assert_eq!(input_series, INPUT_SERIES_SLOT);
    debug_assert_eq!(jack_load, INPUT_JACK_LOAD_SLOT);
    debug_assert_eq!(grid_shunt, INPUT_GRID_SHUNT_SLOT);

    // The drawing's supply nodes below the reservoir. The power module owns the
    // rectifier and the first filter; from the reverb driver's node downward the
    // two 10 kΩ droppers and 16 µF filters are this circuit's.
    net.supply("bplus_b", 150.0, 410.0)
        .capacitor("bplus_b", "gnd", 16e-6)
        .resistor("bplus_b", "bplus_c", 10_000.0)
        .capacitor("bplus_c", "gnd", 16e-6)
        .resistor("bplus_c", "bplus_d", 10_000.0)
        .capacitor("bplus_d", "gnd", 16e-6)
        // The phase inverter hangs on the C node in the real amplifier and is
        // in the power module here, so without this its standing current is
        // missing from the dropper chain and every plate below sits some
        // forty volts high. This is that current as the load it is: a 12AT7
        // long-tailed pair idling off +325 V. It is a DC term only -- the node
        // has 16 uF across it, so at audio frequencies it is already a short.
        .resistor("bplus_c", "gnd", PI_STANDING_LOAD);

    // V1, Vibrato channel first gain stage: 100 kΩ plate, 1.5 kΩ / 25 µF.
    net.resistor("v1_k", "gnd", FIRST_CATHODE)
        .capacitor("v1_k", "gnd", FIRST_BYPASS)
        .resistor("bplus_d", "v1_p", PLATE_LOAD)
        .triode("v1_p", "v1_g", "v1_k", V7025);

    // The stack, with a fixed 6.8 kΩ where a Twin has its Middle pot.
    net.capacitor("v1_p", "t_top", TREBLE_CAP)
        .pot("t_top", "ts_out", "t_bot", TREBLE_POT, Taper::Audio, TREBLE)
        .resistor("v1_p", "slope", SLOPE_R)
        .capacitor("slope", "t_bot", BASS_CAP)
        .capacitor("slope", "b_bot", MID_CAP)
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            BASS_POT,
            Taper::ReverseAudio,
            BASS,
        )
        .resistor("b_bot", "gnd", MID_R)
        .pot("ts_out", "vol", "gnd", VOLUME_POT, Taper::Audio, VOLUME)
        // Permanent, not switched.
        .capacitor("ts_out", "vol", BRIGHT_CAP);

    // V2, the Vibrato second stage, sharing its 820 Ω / 25 µF cathode with the
    // Normal channel's second stage. Node [A] on the drawing. The Normal side
    // stays in the network even though this plugin drives the Vibrato jack, so
    // its shared bias current and its load on the mixer node are real rather
    // than guessed.
    net.resistor("shared_a_k", "gnd", SHARED_CATHODE)
        .capacitor("shared_a_k", "gnd", SHARED_BYPASS)
        .resistor("bplus_d", "v2_p", PLATE_LOAD)
        .capacitor("v2_p", "vol", ECC83_CAG)
        .capacitor("vol", "shared_a_k", ECC83_CGK)
        .capacitor("v2_p", "shared_a_k", ECC83_CAK)
        .triode("v2_p", "vol", "shared_a_k", V7025)
        .resistor("normal2_g", "gnd", 1_000_000.0)
        .resistor("bplus_d", "normal2_p", PLATE_LOAD)
        .triode("normal2_p", "normal2_g", "shared_a_k", V7025);

    // Dry branch: 0.02 µF, then the 3.3 MΩ with its 10 pF bypass into the
    // mixer grid.
    net.capacitor("v2_p", "dry_pre", DRY_COUPLING)
        .resistor("dry_pre", MIX_GRID, DRY_MIX_R)
        .capacitor("dry_pre", MIX_GRID, DRY_BYPASS_C)
        .resistor(MIX_GRID, "gnd", MIX_GRID_LEAK);

    // Reverb send straight off the V2 plate, so the driver's 500 pF / 1 MΩ
    // network and its paralleled 12AT7 actually load V2.
    let core = CoreSpec {
        henry: REVERB_XFMR_PRIMARY_L * 40.0 / (REVERB_XFMR_RATIO * REVERB_XFMR_RATIO),
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

    // The tank pickup returns as an independent source. The recovery stage and
    // the mixer share the drawing's second 820 Ω / 25 µF network, node [E].
    let tank_return = net.aux_input(RETURN, TANK_PICKUP_Z);
    debug_assert_eq!(tank_return, TANK_RETURN_AUX);
    net.resistor(RETURN, "gnd", RECOVERY_GRID_LEAK)
        .resistor("shared_e_k", "gnd", SHARED_CATHODE)
        .capacitor("shared_e_k", "gnd", SHARED_BYPASS)
        .resistor("bplus_d", "recov_p", PLATE_LOAD)
        .triode("recov_p", RETURN, "shared_e_k", V7025)
        .capacitor("recov_p", "rv_top", RECOVERY_COUPLING)
        .pot(
            "rv_top",
            "rv_wiper",
            "gnd",
            REVERB_POT,
            Taper::Linear,
            REVERB,
        )
        .resistor("rv_wiper", MIX_GRID, WET_MIX_R)
        .resistor("bplus_d", MIXER_TO_PI, PLATE_LOAD)
        .triode(MIXER_TO_PI, MIX_GRID, "shared_e_k", V7025);

    // The optical tremolo, in the order this drawing has it: 220 kΩ off the
    // mixer plate, then 0.1 µF into the 50 kΩ reverse-audio Intensity pot. The
    // photoresistor is an audio-rate variable resistor stamped in Newton, so it
    // changes the node's real loading rather than multiplying the output.
    net.resistor(MIXER_TO_PI, "trem_r", CHANNEL_MIX_R)
        .capacitor("trem_r", "intensity_top", TREMOLO_COUPLING)
        .pot(
            "intensity_top",
            "intensity_wiper",
            "gnd",
            INTENSITY_POT,
            Taper::ReverseAudio,
            INTENSITY,
        );
    let ldr = net.adjustable(
        "intensity_wiper",
        "gnd",
        Adjust::RealtimeResistor,
        5_000_000.0,
    );
    debug_assert_eq!(ldr, LDR_SLOT);

    // The quiet Normal channel still reaches the mixer plate through its own
    // 0.047 µF and 220 kΩ, so it loads this node the way it does in the amp.
    net.capacitor("normal2_p", "normal_mix", NORMAL_COUPLING)
        .resistor("normal_mix", MIXER_TO_PI, CHANNEL_MIX_R)
        .resistor(MIXER_TO_PI, "gnd", load)
        .rest(INTENSITY, 1.0);

    net
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    complete_netlist(source, load).build(MIXER_TO_PI)
}

/// The complete channel with an internal node brought out, for stage-by-stage
/// probing without a second approximation of the circuit.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    complete_netlist(source, load).build(at)
}
