//! The Peavey EVH 5150 lead channel preamplifier, from the schematic.
//!
//! Six triode stages between the jack and the tone stack -- V1A, V1B, V2A,
//! V2B, V5B, V5A -- which is two more than the Mark IIC+ has and is the whole
//! reason this amplifier is the one people reach for when they want more gain
//! than an amplifier ought to have.
//!
//! Two things about it are worth saying before the parts, because they are
//! what make it sound like itself rather than like six stages in a row:
//!
//! **It throws most of the signal away between stages.** R32 and R87 are 1 M
//! in series into 100 k and 1 M to ground, and R6 is 470 k into 1 M. Each of
//! those is a divider that loses six to twenty decibels, and they are there
//! so that six stages can be cascaded without the thing turning into an
//! oscillator. The gain is made and then thrown away, over and over, and what
//! survives each round trip has been through a valve that was already
//! clipping.
//!
//! **V2A is run cold on purpose.** R15 is 39 k at the cathode with nothing
//! across it, where every other stage here has 1.8 k or 2.2 k. That starves
//! the stage, drops its gain to nearly nothing and makes it clip hard on one
//! side long before the other -- the "cold clipper", and it is where the
//! squared-off bottom end of this amplifier comes from.
//!
//! ## What is modelled and what is not
//!
//! The lead channel, with the relays in the position that selects it. K2A
//! picks VR2's wiper (the ULTRA PRE control) over VR1's, so the clean side --
//! R19, R21, R43, R78, C10, C51, C43, VR1 and the K4 contacts -- is not in
//! circuit and is not built. K1A and K1B are left open, which leaves R20 off
//! the V2A grid and R18 with C26 off V1B's cathode. That last one is a stated
//! choice rather than a reading: the drawing does not say which way the relay
//! sits for which channel, and open is the tighter and louder of the two,
//! which is what the lead channel is.
//!
//! **The revision is the original 5150.** Every stage here is read off
//! Peavey's "VHALEN 120 PREAMP BD." (CB #98802910, drawing 81501510, sheet 1
//! of 3, 5-FEB-92): the pots on that sheet are themselves labelled ULTRA PRE
//! and ULTRA POST, which is what once made these labels look like a 5150 II's.
//! See `docs/models/american_5150.md`.
//!
//! The tone stack is the drawing's own (built 2026-09-25; until then a 33 k
//! resistor stood in for it and the plugin's generic stack sat after the power
//! stage): R89 into R24 33 k and C59 with R94 to ground, a Fender-family stack
//! with a 570 pF treble capacitor, a 47 k slope, and a middle control whose
//! wiper takes the .022. Its output is the treble wiper, which both post-gain
//! controls hang on; the lead channel's, VR7 ULTRA POST, is the power stage's
//! master (`power::PowerSpec::EVH5150`).
//!
//! As with the Mark IIC+, the supply is the one number not on the drawing.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

/// VR2, marked ULTRA PRE: the lead channel's gain control.
pub const PRE: usize = 0;

/// The plate supply. The drawing names two rails, V1 and V2, and prints the
/// voltage of neither -- see the note above. One figure for both.
///
/// It was 400, which was a guess. Service discussion of Peavey's 6L6 hundred
/// watt amplifiers puts the preamplifier nodes at three hundred to three fifty
/// (diyAudio, "peavey 5150/6505/6505+ HT"), so this is the middle of a sourced
/// range rather than the top of an unsourced one. It is still an
/// approximation: a forum rule of thumb is the bottom of `CLAUDE.md` §3.3's
/// hierarchy, and the real amplifier drops its two rails through an RC ladder
/// that this does not model.
///
/// Measured, it moves the preamplifier's ceiling and nothing else: 37.7 dB at
/// the tone stack on 400 V against 35.8 on 330, with the ULTRA PRE control
/// behaving identically at both. So it is not the reason this amplifier
/// saturates where it does.
pub const SUPPLY: f64 = 330.0;

/// VR5, HIGH, 250 k linear.
pub const TREBLE: usize = 1;
/// VR3, LOW, 1 M audio, wired as a variable resistor.
pub const BASS: usize = 2;
/// VR4, MID, 50 k linear, its wiper taking the middle capacitor.
pub const MIDDLE: usize = 3;

/// What the stack's output drives: the two post-gain controls, VR7 ULTRA POST
/// and VR6 CLEAN POST, 1 M each and both always across the treble wiper; the
/// relay picks which wiper goes on. VR6 is built in the netlist; this is VR7,
/// which is also the power stage's master pot. The chain builds the block with
/// it.
pub const POST_LOAD: f64 = 1_000_000.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "tone")
}

/// The same preamplifier brought out at a chosen node, for measuring one
/// stage at a time.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("EVH 5150 lead preamp");

    // --- the jack ---------------------------------------------------------
    // R27 and R30 are the two input jacks' series resistors; only one guitar
    // is plugged in, so only R27 carries signal and R30 is left out. C20
    // across the pair is 39 pF, which does nothing in the audio band and is
    // there for radio frequencies.
    net.input("jack", source)
        .resistor("jack", "j", 22_000.0) // R27
        .capacitor("j", "gnd", 39e-12) // C20
        .capacitor("j", "g1", 0.1e-6) // C15
        .resistor("g1", "gnd", 1_000_000.0) // R25, the grid leak
        .resistor("g1", "v1a_g", 68_000.0); // R22, and 68 k is a large stopper

    // --- V1A --------------------------------------------------------------
    net.resistor("v1a_k", "gnd", 1_820.0) // R12
        .capacitor("v1a_k", "gnd", 1e-6) // C3, fully bypassed
        .supply("v1a_p", 220_000.0, SUPPLY) // R2
        .triode("v1a_p", "v1a_g", "v1a_k", ECC83);

    // --- the ULTRA PRE control --------------------------------------------
    // R29 470 k into the pot, with C17 .0022 across it so the top of the band
    // does not lose what the midrange loses. Then the pot itself is loaded
    // twice: R41 across the whole track and R40 across its upper half, with
    // C19 across that. Loading an audio pot like this straightens its law --
    // and R40 with C19 means the control also gets brighter as it comes up,
    // rather than only louder.
    net.capacitor("v1a_p", "bus", 0.022e-6) // C6
        .resistor("bus", "pre_top", 470_000.0) // R29
        .capacitor("bus", "pre_top", 0.0022e-6) // C17
        .pot("pre_top", "pre_w", "gnd", 1_000_000.0, Taper::Audio, PRE) // VR2
        .resistor("pre_top", "gnd", 1_000_000.0) // R41, across the track
        .resistor("pre_top", "pre_w", 1_000_000.0) // R40, across the top half
        .capacitor("pre_top", "pre_w", 0.001e-6); // C19

    // --- V1B ---------------------------------------------------------------
    net.resistor("pre_w", "v1b_g", 470_000.0) // R82
        .resistor("v1b_k", "gnd", 1_820.0) // R17
        .capacitor("v1b_k", "gnd", 1e-6) // C4
        .supply("v1b_p", 100_000.0, SUPPLY) // R3
        .triode("v1b_p", "v1b_g", "v1b_k", ECC83);

    // --- V2A, the cold clipper ---------------------------------------------
    // R6 into R9 loses three decibels before the grid, and then R15 is 39 k
    // unbypassed. See the note at the top: this stage is not here for gain.
    net.capacitor("v1b_p", "n2", 0.022e-6) // C2
        .resistor("n2", "v2a_g", 470_000.0) // R6
        .resistor("v2a_g", "gnd", 1_000_000.0) // R9
        .resistor("v2a_k", "gnd", 39_000.0) // R15, and nothing across it
        .supply("v2a_p", 100_000.0, SUPPLY) // R1
        .triode("v2a_p", "v2a_g", "v2a_k", ECC83);

    // --- V2B ---------------------------------------------------------------
    // C48 with R11 takes the top off what leaves the cold clipper before R7
    // passes it on: 330 k and .001 corner at about 480 Hz, so the buzz the
    // hard clipping just made is filtered before the next stage amplifies it.
    net.capacitor("v2a_p", "n3", 0.022e-6) // C1
        .resistor("n3", "gnd", 330_000.0) // R11
        .capacitor("n3", "gnd", 0.001e-6) // C48
        .resistor("n3", "v2b_g", 220_000.0) // R7
        .resistor("v2b_k", "gnd", 1_820.0) // R13
        .capacitor("v2b_k", "gnd", 1e-6) // C5
        .supply("v2b_p", 220_000.0, SUPPLY) // R4
        .triode("v2b_p", "v2b_g", "v2b_k", ECC83);

    // --- V5B ---------------------------------------------------------------
    // R32 1 M in series into R101 100 k to ground is twenty one decibels
    // thrown away between two stages that each have thirty odd.
    net.resistor("v2b_p", "n4", 1_000_000.0) // R32
        .capacitor("n4", "v5b_g", 0.022e-6) // C57
        .resistor("v5b_g", "gnd", 100_000.0) // R101
        .resistor("v5b_k", "gnd", 2_200.0) // R102, unbypassed
        .supply("v5b_p", 220_000.0, SUPPLY) // R96
        .triode("v5b_p", "v5b_g", "v5b_k", ECC83);

    // --- V5A, with feedback -------------------------------------------------
    // R88 1 M from the output back to this grid is the only feedback anywhere
    // in the preamplifier. It cannot be reaching the plate: the plate sits a
    // couple of hundred volts up and 1 M from there through R91 would put a
    // hundred of them on the grid. It reaches the far side of C58, which is
    // blocked for direct voltage and free for signal.
    net.resistor("v5b_p", "n5", 1_000_000.0) // R87
        .capacitor("n5", "v5a_g", 0.022e-6) // C56
        .resistor("v5a_g", "gnd", 1_000_000.0) // R91
        .resistor("v5a_g", "out", 1_000_000.0) // R88, the feedback
        .resistor("v5a_k", "gnd", 2_200.0) // R93, unbypassed
        .supply("v5a_p", 100_000.0, SUPPLY) // R86
        .triode("v5a_p", "v5a_g", "v5a_k", ECC83);

    // --- out, and the tone stack -------------------------------------------
    // R89 470 k into R24 33 k: twenty four decibels thrown away before the
    // stack does anything, which is why this amplifier has a post gain. C59
    // and R94 47 k from the same node to ground take a little more off the top.
    net.capacitor("v5a_p", "out", 0.022e-6) // C58
        .resistor("out", "stack", 470_000.0) // R89
        .resistor("stack", "gnd", 33_000.0) // R24
        .capacitor("stack", "c59", 0.001e-6) // C59
        .resistor("c59", "gnd", 47_000.0); // R94

    // C14 470 pF and C60 100 pF side by side to the top of HIGH, R28 47 k to the
    // slope node, C18 .022 from there to the bottom of HIGH and the top of LOW,
    // C21 .022 to MID's wiper. LOW is a variable resistor (wiper to its top)
    // and turned up puts resistance in: the reverse law, as on every stack of
    // this family here. MID is a real divider to ground with R95 47 k across
    // its track. Pin numbers and the clockwise arrows are the drawing's.
    net.capacitor("stack", "hi_top", 470e-12) // C14
        .capacitor("stack", "hi_top", 100e-12) // C60
        .resistor("stack", "slope", 47_000.0) // R28
        .pot("hi_top", "tone", "hi_bot", 250_000.0, Taper::Linear, TREBLE) // VR5
        .capacitor("slope", "hi_bot", 0.022e-6) // C18
        .pot(
            "hi_bot",
            "lo_bot",
            "lo_bot",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        ) // VR3
        .pot("lo_bot", "mid_w", "gnd", 50_000.0, Taper::Linear, MIDDLE) // VR4
        .resistor("lo_bot", "gnd", 47_000.0) // R95
        .capacitor("slope", "mid_w", 0.022e-6) // C21
        // VR6 CLEAN POST, 1 M, across the stack's output whichever channel is on.
        .resistor("tone", "gnd", 1_000_000.0)
        .resistor("tone", "gnd", load);

    net.build(at)
}
