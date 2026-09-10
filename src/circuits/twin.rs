//! The Fender Twin Reverb, AB763, from the drawing.
//!
//! The '65 Twin Reverb is Fender's reissue of the 1965 blackface Twin, and the
//! circuit it reissues is the AB763. So the AB763 schematic is the target and
//! the reissue's departures from it -- printed board, modern capacitor
//! tolerances, the 85 W rating -- are not modelled. See `CLAUDE.md` §42.1.
//!
//! ```text
//! TARGET:        Fender Twin Reverb
//! REVISION:      AB763, blackface
//! SCHEMATIC:     "FENDER MODEL TWIN REVERB-AMP AB763 SCHEMATIC", C-FD,
//!                Fender Musical Instruments, Fullerton -- fetched from
//!                schematicheaven.net and read directly
//! SOURCE:        manufacturer schematic
//! CONFIDENCE:    high for the channel and the tone stack, which are legible;
//!                see the notes on the reverb mixing network, which is not
//! APPROXIMATION: the spring tank and the optocoupler are not netlist parts.
//!                See `Reverb send/return` below.
//! ```
//!
//! ## What is here
//!
//! The **Vibrato channel**, which is the Normal channel plus a reverb and a
//! tremolo, up to the point where the drawing hands over to the phase
//! inverter. That boundary is the same one `markiic.rs` draws: what follows is
//! a power amplifier and belongs in `power.rs`.
//!
//! ```text
//! in -- 68k -- V1 (7025) -- tone stack -- Volume + Bright -- V2 (7025) -->
//!                |                                             ^
//!                +-- reverb driver (12AT7) -- TR4 --> [ tank ] |
//!                                                         |    |
//!                              recovery (1/2 7025) -- Reverb --+
//! ```
//!
//! ## What is not, and why
//!
//! **The spring tank.** TR4 drives a mechanical transmission line, not a
//! circuit. A netlist cannot hold it. The drawing's dashed `REVERB UNIT` box
//! says as much -- it is a separate unit with an input and an output -- so the
//! netlist stops at the transformer and the tank goes between the send tap and
//! the return node as its own model.
//!
//! **The tremolo's optocoupler.** A neon bulb lights a light-dependent
//! resistor which shunts the signal, and it does so continuously at two to ten
//! hertz. That resistance cannot be a `pot`: a control that moves marks the
//! matrix dirty and the next sample rebuilds it, so a tremolo driven through
//! the control system would rebuild the matrix every sample. It has to be a
//! device that restamps its own conductance per pass, with the bulb's thermal
//! lag, which is part of what a Fender tremolo sounds like.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// The tone stack, in the order the panel has them.
pub const TREBLE: usize = 0;
pub const BASS: usize = 1;
pub const MIDDLE: usize = 2;
/// The channel volume, 1 M audio.
pub const VOLUME: usize = 3;
/// How much of the tank comes back, 100 k **linear** on the drawing -- not an
/// audio taper, which is unusual for a Fender and is why the control does so
/// much in the first third of its travel.
pub const REVERB: usize = 4;

/// The plate supply where the drawing prints it: +410 V at the preamplifier
/// node, from a +460 V rail through the dropping chain.
pub const SUPPLY: f64 = 410.0;

/// 7025 is a low-noise, controlled-heater 12AX7. Electrically an ECC83.
const V7025: TriodeSpec = TriodeSpec::ECC83;
/// Where the reverb transformer's secondary drives the tank.
pub const SEND: &str = "tank_in";
/// Where the tank comes back into the recovery stage's grid.
pub const RETURN: &str = "tank_out";

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The reverb recovery stage, and the Reverb control.
///
/// Half a 7025 with a 220 k plate load -- higher than the channel stages' 100 k
/// because what comes back off a tank is tiny -- then the .003 out, the 100 k,
/// and the 100 k **linear** Reverb pot. Linear is unusual for a Fender and is
/// why the control does so much in the first third of its travel.
///
/// ## Two departures from the drawing, and they are departures
///
/// On the amplifier the send is taken from **V1's plate**, ahead of the tone
/// stack, and the return is mixed into **V2's grid** through a 3.3 M with
/// 10 pF across it. So the tank hears the channel's full bandwidth and the
/// wet signal is then amplified and coloured by V2, which is a good part of
/// why a blackface reverb sits *under* the dry signal rather than on top of
/// it.
///
/// Neither is possible here. That send and that return make a **loop through
/// the preamplifier**, and a `Chain` runs its circuits one way: A's output
/// into B's input, with no path back. Holding the loop would need the whole
/// channel and the whole recovery stage in one matrix with the tank inside
/// it, and the tank is not a circuit at all -- it is a mechanical delay line
/// with a dispersion relation, which is the entire reason it lives in
/// `dsp::spring`.
///
/// So: the send is taken from the channel's input rather than from V1's
/// plate, with `SEND_GAIN` standing in for that stage, and the return is
/// summed at the output rather than at V2's grid. The recovery stage itself,
/// the pot and its law are the drawing's.
pub fn reverb_return(source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Twin Reverb recovery");
    net.input(RETURN, source)
        .resistor(RETURN, "gnd", 220_000.0)
        .resistor("rc_k", "gnd", 820.0)
        .capacitor("rc_k", "gnd", 25e-6)
        .supply("rc_p", 220_000.0, SUPPLY)
        .triode("rc_p", RETURN, "rc_k", V7025)
        .capacitor("rc_p", "rv_send", 0.003e-6)
        .resistor("rv_send", "rv_top", 100_000.0)
        .pot("rv_top", "out", "gnd", 100_000.0, Taper::Linear, REVERB)
        .resistor("out", "gnd", load);
    net.build("out")
}

/// What V1 does to the signal before the reverb driver sees it, standing in
/// for the stage the send is really taken from. Measured small-signal gain of
/// V1 into the driver's grid leak; an estimate of a stage, not a value off the
/// drawing. See `reverb_return`.
///
/// The driver itself -- a 12AT7 with both halves in parallel into the reverb
/// transformer -- is not modelled as a circuit either, for the same reason:
/// nothing downstream of it comes back, so the only thing it contributes to
/// what is heard is level.
pub const SEND_GAIN: f64 = 6.0;

/// What the whole send-and-return path has to be divided by before it is
/// summed on to the channel.
///
/// This is the price of the departure `reverb_return` describes. On the
/// drawing the recovery stage returns into V2's grid, *inside* the channel, so
/// everything after it -- the rest of the preamplifier, the make-up that holds
/// the channel at a constant level -- acts on the reverb and the dry signal
/// alike. Here the return is summed at the output instead, after the make-up
/// has already normalised the dry path to unity. So the wet arrives with the
/// path's raw gain on it and the dry does not.
///
/// Measured, at Reverb wide open: `SEND_GAIN` 30 times the tank's 0.53 times
/// the recovery stage's 24.3 is 383, which is **51.7 dB**. That is what the
/// control was actually doing. At the shipped preset's 0.35 the tank came out
/// 38.6 dB above the dry signal, and at the top of the knob the plugin put out
/// +24 dBFS -- which is why turning the Reverb down sounded like turning the
/// amplifier off. Reported as "the Reverb button works somehow like a volume
/// button".
///
/// Dividing it out puts the wet at the dry's level with the pot wide open,
/// which is what the pot is for: it is a 100 k linear track on the drawing and
/// it blends, so at 3 or 4 on the dial the tank sits about nine decibels under
/// the signal, where a Fender's does.
pub const RETURN_TRIM: f64 = 6.0 / 383.4;

/// The same channel brought out at a chosen node, for measuring one stage at a
/// time. `SEND` and `RETURN` are the two the tank lives between.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Twin Reverb AB763");

    // --- input and V1 -----------------------------------------------------
    // Two jacks, each behind its own 68 k, into a 1 M grid leak. Jack 1 is the
    // one modelled: jack 2 puts the two 68 k in parallel as a divider and is
    // six decibels down, which is a different input rather than a quieter one.
    net.input("in", source)
        .resistor("in", "v1_g", 68_000.0)
        .resistor("v1_g", "gnd", 1_000_000.0)
        // 1600 ohm on the vibrato channel and 1500 on the normal one. The
        // drawing prints both and they are not a misprint: the vibrato channel
        // is biased very slightly colder because it carries the reverb and
        // tremolo mixing behind it.
        .resistor("v1_k", "gnd", 1_600.0)
        .capacitor("v1_k", "gnd", 25e-6)
        .supply("v1_p", 100_000.0, SUPPLY)
        .triode("v1_p", "v1_g", "v1_k", V7025);

    // --- the tone stack ---------------------------------------------------
    // The Fender stack, and the Twin has a real Middle control where the
    // smaller blackface amplifiers have a fixed 6.8 k resistor. That 10 k pot
    // is most of why a Twin can be made to sound scooped and a Deluxe cannot.
    //
    // The 500 pF from the bottom of the middle pot to ground is on the vibrato
    // channel only. It is a small shunt across the bottom leg, and it is the
    // one component that makes the two channels' stacks measure differently.
    // A Fender stack has **three** capacitors and the first build of this had
    // two: the 0.047 uF to the middle leg was there and the 0.1 uF to the bass
    // leg was not. Measured, that left the treble control four decibels of
    // range and the middle control nine tenths of one, which is a stack that
    // does not work. `CLAUDE.md` §42.3 gives the canonical AB763 values as
    // 250 pF treble, 0.1 uF bass and 0.047 uF middle; this render of the
    // drawing resolves the 250 pF and the .047 and does not resolve the 0.1,
    // so that one is taken from §42.3 and from the topology rather than read,
    // and is marked as such.
    net.capacitor("v1_p", "t_top", 250e-12)
        .pot("t_top", "ts_out", "t_bot", 250_000.0, Taper::Linear, TREBLE)
        .resistor("v1_p", "slope", 100_000.0)
        .capacitor("slope", "t_bot", 0.1e-6)
        .capacitor("slope", "b_bot", 0.047e-6)
        // Both legs are wired as variable resistors, so what stays in circuit
        // is `1 - fraction` and a forward taper would run them backwards.
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            250_000.0,
            Taper::ReverseLinear,
            BASS,
        )
        .pot(
            "b_bot",
            "gnd",
            "gnd",
            10_000.0,
            Taper::ReverseLinear,
            MIDDLE,
        )
        // The vibrato channel's own 500 pF, across the bottom of the stack.
        // The first build gave it a node of its own with a one-ohm resistor to
        // ground so that node had a direct path -- which is the node added to
        // be tidy that §59.1 forbids, and it cost an unknown in a matrix
        // solved a hundred thousand times a second. Across the middle pot it
        // needs no node at all. Which of the two the drawing means is not
        // resolvable at this render; this is the reading that invents nothing.
        .capacitor("b_bot", "gnd", 500e-12);

    // --- Volume, and the Bright switch ------------------------------------
    // 1 M audio, with 120 pF across the top half. The capacitor's effect
    // depends on where the wiper sits -- wide open it has nothing to bypass --
    // which is why a bright switch on a Fender does a great deal at low volume
    // and nothing at high. It is modelled in circuit; the switch itself is a
    // front-panel control the plugin does not yet reach.
    net.pot("ts_out", "vol", "gnd", 1_000_000.0, Taper::Audio, VOLUME)
        .capacitor("ts_out", "vol", 120e-12);

    // --- V2, the recovery stage -------------------------------------------
    // .02 on the vibrato channel where the normal channel has .047: the
    // vibrato channel is deliberately thinner going in, because the reverb
    // return is about to be mixed onto the same grid.
    net.capacitor("vol", "v2_g", 0.02e-6)
        .resistor("v2_g", "gnd", 220_000.0)
        .resistor("v2_k", "gnd", 820.0)
        .capacitor("v2_k", "gnd", 25e-6)
        .supply("v2_p", 100_000.0, SUPPLY)
        .triode("v2_p", "v2_g", "v2_k", V7025)
        .capacitor("v2_p", "out", 0.1e-6)
        .resistor("out", "gnd", load);

    net.build(at)
}
