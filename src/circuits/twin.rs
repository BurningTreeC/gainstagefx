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
/// The reverb driver, whose two halves the drawing wires in parallel.
const V12AT7: TriodeSpec = TriodeSpec::ECC81;

/// Where the reverb transformer's secondary drives the tank.
pub const SEND: &str = "tank_in";
/// Where the tank comes back into the recovery stage's grid.
pub const RETURN: &str = "tank_out";

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

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

    // --- reverb driver ----------------------------------------------------
    // A 12AT7 with both halves in parallel, which is a lower plate resistance
    // and the current the transformer's primary needs. Two triodes in parallel
    // are one device passing twice the current, so that is how it is stamped.
    //
    // The driver is fed from V1's plate, ahead of the tone stack: the tank
    // gets the channel's full bandwidth and the stack shapes only the dry
    // path. That is what gives a blackface reverb its brightness against a
    // rolled-off dry signal.
    net.capacitor("v1_p", "rv_g", 0.1e-6)
        .resistor("rv_g", "gnd", 1_000_000.0)
        .resistor("rv_k", "gnd", 2_800.0)
        .capacitor("rv_k", "gnd", 25e-6)
        .triode("rv_p", "rv_g", "rv_k", V12AT7)
        // TR4, the reverb transformer.
        //
        // The plate's direct current comes **through the primary winding**
        // from the supply -- that is what a transformer-coupled plate is. The
        // first build put the supply on the plate and the primary across
        // plate-to-ground beside it, which is an ideal ratio in parallel with
        // the stage: a short at direct current. Measured, it put the driver's
        // plate and cathode both at 0.0 V against the 440 V and 8.6 V the
        // drawing prints beside that tube. `power.rs` has the pattern -- the
        // winding is a resistance and a magnetising inductance in series from
        // the supply to the plate, with the ideal ratio across it.
        //
        // The drawing gives no turns ratio and no winding figures. A Fender
        // reverb driver transformer steps down hard into a tank of a few tens
        // of ohms; 22:1, a 250 ohm primary and 1.5 H of magnetising inductance
        // are **estimates of the part, not values read off the schematic**.
        .supply("rv_ht", 1_000.0, 440.0)
        .resistor("rv_ht", "rv_mag", 250.0)
        .inductor("rv_mag", "rv_p", 1.5)
        .transformer("rv_mag", "rv_p", SEND, "gnd", 1.0 / 22.0)
        .resistor(SEND, "gnd", 8.0);

    // --- reverb recovery and the Reverb control ---------------------------
    // Half a 7025 with a 220 k plate load -- higher than the 100 k the channel
    // stages use, because what comes back off a tank is tiny.
    //
    // **The mixing network is the part of this drawing that is hardest to
    // read.** What is legible: a .003 out of the recovery plate, a 100 k, the
    // 100 k linear REVERB pot, and a 3.3 M with 10 pF across it going to the
    // channel's second grid, with 470 k and 220 k in the divider. The exact
    // order of the 470 k and 220 k against the pot is an inference from the
    // usual AB763 arrangement rather than something read off this render, and
    // it is marked here rather than presented as certain. See `CLAUDE.md`
    // §53.13.
    net.resistor(RETURN, "gnd", 220_000.0)
        .resistor("rc_k", "gnd", 820.0)
        .capacitor("rc_k", "gnd", 25e-6)
        .supply("rc_p", 220_000.0, SUPPLY)
        .triode("rc_p", RETURN, "rc_k", V7025)
        .capacitor("rc_p", "rv_send", 0.003e-6)
        .resistor("rv_send", "rv_top", 100_000.0)
        .pot("rv_top", "rv_w", "gnd", 100_000.0, Taper::Linear, REVERB)
        .resistor("rv_w", "rv_mix", 470_000.0)
        .resistor("rv_mix", "gnd", 220_000.0)
        // Into the same grid the dry signal arrives at. The 10 pF across the
        // 3.3 M is what lets the top of the reverb through a resistance that
        // would otherwise be far too high for it.
        .resistor("rv_mix", "v2_g", 3_300_000.0)
        .capacitor("rv_mix", "v2_g", 10e-12);

    net.build(at)
}
