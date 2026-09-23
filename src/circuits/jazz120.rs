//! Roland JC-120 Jazz Chorus, CH-1 (the clean channel).
//!
//! Read off the **PA BOARD ASSY CIRCUIT DIAGRAM** of the JC-120UT/JT service
//! notes dated Sep. 2000 -- the current-production amplifier. That sheet is a
//! 1334 ppi scan in `docs/schematics/roland_jc120ut_jt_service_notes_2000.pdf`
//! and every value and every connection below is read off it directly. The
//! checkpoint is `docs/models/jazz_120.md`.
//!
//! **This is the clean channel only.** CH-2's distortion and reverb, the BBD
//! chorus/vibrato and the two power amplifiers are separate work; what is here
//! is the signal path a JC-120 is famous for, from the jacks to the channel's
//! output at CN3.
//!
//! The sheet carries its own operating points, and they are the check on this
//! model rather than anything fitted:
//!
//! | node | printed |
//! |---|---|
//! | CH1 INPUT HIGH, 1 kHz | -30 dBm, 69.3 mVpp |
//! | J3 drain | -10 dBm, 693 mVpp |
//! | CN3, channel out | +13 dBm, 9.79 Vpp |
//!
//! measured with the Setting block's `VOL, EQ MAX` and `BRI OFF`. Twenty
//! decibels across the first stage and forty-three across the channel;
//! `tests/jazz120.rs` holds the model to both.

use crate::dsp::netlist::{Adjust, BipolarSpec, Circuit, Fault, JfetSpec, Netlist, Taper};

/// The panel, in the order the amplifier has them.
pub const VOLUME: usize = 0;
pub const TREBLE: usize = 1;
pub const MIDDLE: usize = 2;
pub const BASS: usize = 3;

/// The two jacks on the CH1 INPUT BOARD: R1 33 k from HIGH and R2 68 k from
/// LOW into the same node. The printed sensitivities are -30 dBm and -20 dBm
/// at 1 kHz.
pub const INPUT_SERIES_SLOT: usize = 0;
pub const INPUT_HIGH_SERIES_OHMS: f64 = 33_000.0;
pub const INPUT_LOW_SERIES_OHMS: f64 = 68_000.0;

/// SW2, the BRI switch. It does not switch a capacitor in and out: C7 is
/// always across the Volume, and the switch **shorts R4** so the capacitor
/// couples fully. Off, C7 works through 680 k and barely tilts anything.
///
/// One ohm rather than a milliohm for the closed switch: against C7's 24 kOhm
/// at 20 kHz anything under a few ohms is the same short, and a milliohm would
/// put a thousand siemens in a row whose other entries are microsiemens -- a
/// conditioning hazard bought for nothing.
pub const BRIGHT_SLOT: usize = 1;
pub const BRIGHT_ON_OHMS: f64 = 1.0;
pub const BRIGHT_OFF_OHMS: f64 = 680_000.0;

/// Diagnostic nodes, for `tap()`.
/// J3's drain: the head amp's output and the sheet's -10 dBm test point.
pub const HEAD_OUT: &str = "j3_d";
/// The Treble wiper, which is the tone network's only output.
pub const STACK_OUT: &str = "ts_out";
/// The Volume wiper.
pub const VOLUME_OUT: &str = "vol";
/// CN3, the channel's output into the main amplifier board.
pub const CHANNEL_OUT: &str = "ch1_out";

// --- devices -------------------------------------------------------------
/// J3 and Q1, both 2SK184 on the sheet.
const Q_FET: JfetSpec = JfetSpec::J2SK184;
/// Q2, the output follower.
const Q_OUT: BipolarSpec = BipolarSpec::NPN_2SC1815_GR;

// --- the supply ----------------------------------------------------------
/// +VA, which the main amplifier board makes with a 1.5 k feed, an MTZ-30B
/// zener and a 2SD1763 follower. DOCUMENTED: the zener sets it.
const RAIL_VOLTS: f64 = 30.0;
/// ESTIMATED: the follower's output impedance behind that zener. An emitter
/// follower off a 30 V reference is stiff and the whole channel draws a few
/// milliamps.
const RAIL_OHMS: f64 = 47.0;
/// R20, the dropper from +VA to +VO, and C11 behind it. **The whole head amp
/// runs from +VO** -- both stages and the bias dividers -- so this resistor
/// carries the channel's entire current and is worth a few volts.
const RAIL_DROPPER: f64 = 680.0;
const RAIL_DECOUPLING: f64 = 470e-6;

// --- the input network ---------------------------------------------------
/// C4, across the input node.
const INPUT_TRIM_CAP: f64 = 150e-12;
/// C5, into the head amp.
const INPUT_COUPLING: f64 = 0.068e-6;
/// R7, the gate stopper.
const GATE_STOPPER: f64 = 680.0;
/// R8, the gate return. It does **not** go to ground: it returns to the
/// R14/R9 divider, which is what puts the gate a little above zero and what
/// makes the published 680 kOhm input impedance the figure it is.
const GATE_RETURN: f64 = 680_000.0;
/// R14 and R9, the divider off +VO that R8 returns to.
const BIAS_TOP: f64 = 68_000.0;
const BIAS_BOTTOM: f64 = 1_800.0;
/// R16, J3's drain load. **This is a common-source stage, not a follower.**
const DRAIN_LOAD: f64 = 15_000.0;
/// R11 and R13, both from J3's source to ground -- two parts, one node. Their
/// parallel value, 1.229 k, is unbypassed, which is what holds the stage to
/// the sheet's own 20 dB.
const SOURCE_R_A: f64 = 6_800.0;
const SOURCE_R_B: f64 = 1_500.0;
/// C8, drain to gate.
const MILLER_CAP: f64 = 15e-12;

// --- the tone network ----------------------------------------------------
/// VR2, 250 kB linear. The wiper is the network's output.
const TREBLE_POT: f64 = 250_000.0;
/// VR4, 250 kA audio, wired as a rheostat -- wiper strapped to terminal 3.
const BASS_POT: f64 = 250_000.0;
/// VR3, 10 kB linear, also a rheostat. A tenth of a Fender's, which is most of
/// why this stack does not behave like one.
const MID_POT: f64 = 10_000.0;
/// VR1, 1 MB linear.
const VOLUME_POT: f64 = 1_000_000.0;
/// C9, the treble cap. The sheet gives two: 150 pF (part 0112638901) on the
/// J and new-U amplifiers, 120 pF (0112637900) on the old U. This is the
/// current one.
const TREBLE_CAP: f64 = 150e-12;
/// C13, from the slope resistor into the Treble/Bass junction.
const BASS_CAP: f64 = 0.082e-6;
/// C12, from the same slope resistor into the **Middle** pot's top. Roland
/// splits the slope-fed signal into two caps and two entry points, which is
/// the one structural difference from a Fender stack and the reason a single
/// slope cap can never reproduce all three published ranges at once.
const MID_CAP: f64 = 0.056e-6;
/// R19, between the Bass rheostat's bottom and the Middle pot's top.
const STACK_SERIES: f64 = 1_000.0;
/// R18, the slope resistor -- the same job it does in a Fender stack.
const SLOPE_R: f64 = 100_000.0;
/// C7 and R4, the bright network across the Volume, and SW2 across R4. C7 is
/// 330 pF (0112638901) here, 220 pF (0112638900) on the old U amplifier.
const BRIGHT_CAP: f64 = 330e-12;

// --- the stage behind the Volume ----------------------------------------
/// C2 and R1, into Q1's gate.
const VOL_COUPLING: f64 = 0.068e-6;
const Q1_GATE_STOPPER: f64 = 680.0;
/// R3, Q1's gate return, into the R2/R5 divider below it. That divider puts
/// the gate at about 60 mV, so the stage self-biases on its source resistor.
const Q1_GATE_RETURN: f64 = 1_500_000.0;
const Q1_BIAS_TOP: f64 = 330_000.0;
const Q1_BIAS_BOTTOM: f64 = 680.0;
/// R15, Q1's drain load, which is also Q2's base feed.
const Q1_DRAIN_LOAD: f64 = 33_000.0;
/// R12 for the direct current and R10 in series with C6 across it, so the
/// alternating-current degeneration is 1.5 k in parallel with 68 ohms. That
/// is what makes this the channel's gain stage.
const Q1_SOURCE_DC_R: f64 = 1_500.0;
const Q1_SOURCE_AC_R: f64 = 68.0;
const Q1_SOURCE_BYPASS: f64 = 47e-6;
/// R17, the load Q2's follower works into, and C10 in front of CN3.
const FOLLOWER_OUT_R: f64 = 6_800.0;
const OUT_COUPLING: f64 = 4.7e-6;

/// The tone network's values, gathered so `examples/jc120` can put the sheet's
/// model variants side by side without editing the circuit. Every one of them
/// is read off the 1334 ppi sheet; nothing here is fitted.
#[derive(Clone, Copy, Debug)]
pub struct StackValues {
    pub treble_cap: f64,
    pub mid_cap: f64,
    pub bass_cap: f64,
    pub slope: f64,
    pub series: f64,
}

impl StackValues {
    /// The current-production JC-120JT and new JC-120UT.
    pub const AS_READ: StackValues = StackValues {
        treble_cap: TREBLE_CAP,
        mid_cap: MID_CAP,
        bass_cap: BASS_CAP,
        slope: SLOPE_R,
        series: STACK_SERIES,
    };
    /// The old JC-120UT, whose only difference in this network is C9.
    pub const OLD_U: StackValues = StackValues {
        treble_cap: 120e-12,
        ..StackValues::AS_READ
    };
}

/// Build CH-1 as one network, jacks to CN3.
fn complete_netlist(source: f64, load: f64) -> Netlist {
    netlist_with(StackValues::AS_READ, source, load)
}

fn netlist_with(v: StackValues, source: f64, load: f64) -> Netlist {
    let mut net = Netlist::new("Roland JC-120 CH-1");

    // The CH1 INPUT BOARD: two jacks into two series resistors into one node,
    // so the switch is one adjustable rather than a change of topology.
    net.input("jack", source);
    let series = net.adjustable("jack", "in", Adjust::Resistor, INPUT_HIGH_SERIES_OHMS);
    debug_assert_eq!(series, INPUT_SERIES_SLOT);

    // +VA from the main amplifier board, and +VO behind R20. Everything in
    // the channel hangs off +VO.
    net.supply("va", RAIL_OHMS, RAIL_VOLTS)
        .resistor("va", "vo", RAIL_DROPPER)
        .capacitor("vo", "gnd", RAIL_DECOUPLING);

    // J3, the 2SK184 common-source input stage. R16 is its drain load and the
    // stack hangs off the drain, so the network is driven from 15 k rather
    // than from a follower -- which matters to every figure below.
    net.capacitor("in", "gnd", INPUT_TRIM_CAP)
        .capacitor("in", "j3_in", INPUT_COUPLING)
        .resistor("j3_in", "j3_g", GATE_STOPPER)
        .resistor("j3_g", "j3_bias", GATE_RETURN)
        .resistor("vo", "j3_bias", BIAS_TOP)
        .resistor("j3_bias", "gnd", BIAS_BOTTOM)
        .resistor("vo", HEAD_OUT, DRAIN_LOAD)
        .capacitor(HEAD_OUT, "j3_g", MILLER_CAP)
        .resistor("j3_s", "gnd", SOURCE_R_A)
        .resistor("j3_s", "gnd", SOURCE_R_B)
        .jfet(HEAD_OUT, "j3_g", "j3_s", Q_FET);

    // The tone network. Passive, and the stack's bottom returns to ground:
    // there is no feedback path around it. Two things separate it from a
    // Fender stack -- the 10 k Middle pot, and the second slope cap C12, which
    // enters at the Middle pot's top rather than at the Bass pot's bottom, on
    // the far side of R19.
    //
    // Both the Bass and the Middle pots are rheostats with the wiper strapped
    // to terminal 3, so a *reverse* taper is what leaves the forward law in
    // circuit. See `Taper::ReverseLinear`.
    net.capacitor(HEAD_OUT, "t_top", v.treble_cap)
        .pot(
            "t_top",
            STACK_OUT,
            "t_bot",
            TREBLE_POT,
            Taper::Linear,
            TREBLE,
        )
        .resistor(HEAD_OUT, "slope", v.slope)
        .capacitor("slope", "t_bot", v.bass_cap)
        .capacitor("slope", "m_top", v.mid_cap)
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            BASS_POT,
            Taper::ReverseAudio,
            BASS,
        )
        .resistor("b_bot", "m_top", v.series)
        .pot("m_top", "gnd", "gnd", MID_POT, Taper::ReverseLinear, MIDDLE)
        .pot(
            STACK_OUT,
            VOLUME_OUT,
            "gnd",
            VOLUME_POT,
            Taper::Linear,
            VOLUME,
        );

    // The bright network: C7 permanently across the Volume, in series with R4,
    // and SW2 shorting R4 when BRI is on.
    net.capacitor(STACK_OUT, "bri", BRIGHT_CAP);
    let bright = net.adjustable("bri", VOLUME_OUT, Adjust::Resistor, BRIGHT_OFF_OHMS);
    debug_assert_eq!(bright, BRIGHT_SLOT);

    // Q1, the second 2SK184, and Q2 behind it as an emitter follower.
    net.capacitor(VOLUME_OUT, "q1_in", VOL_COUPLING)
        .resistor("q1_in", "q1_g", Q1_GATE_STOPPER)
        .resistor("q1_g", "q1_bias", Q1_GATE_RETURN)
        .resistor("vo", "q1_bias", Q1_BIAS_TOP)
        .resistor("q1_bias", "gnd", Q1_BIAS_BOTTOM)
        .resistor("vo", "q1_d", Q1_DRAIN_LOAD)
        .resistor("q1_s", "gnd", Q1_SOURCE_DC_R)
        .resistor("q1_s", "q1_ac", Q1_SOURCE_AC_R)
        .capacitor("q1_ac", "gnd", Q1_SOURCE_BYPASS)
        .jfet("q1_d", "q1_g", "q1_s", Q_FET)
        // Q2: collector on the rail, base straight off Q1's drain, output
        // from the emitter through C10 to CN3.
        .resistor("q2_e", "gnd", FOLLOWER_OUT_R)
        .bipolar("vo", "q1_d", "q2_e", Q_OUT)
        .capacitor("q2_e", CHANNEL_OUT, OUT_COUPLING)
        .resistor(CHANNEL_OUT, "gnd", load);

    net
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    complete_netlist(source, load).build(CHANNEL_OUT)
}

/// The channel with an internal node brought out, for stage-by-stage probing.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    complete_netlist(source, load).build(at)
}

/// The channel built with a stated set of tone-network values, so the
/// measurement harness can compare the sheet's model variants. Not used by the
/// plugin.
pub fn tap_with(v: StackValues, source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    netlist_with(v, source, load).build(at)
}
