//! The Mesa Boogie Mark IIC+ lead-channel preamplifier, from the schematic.
//!
//! Four triode stages and a Fender tone stack between the first two of them.
//! `INPUT` through V1A, the stack, Volume 1, V1B, then the lead drive control
//! into V3B and V4A, ending where the drawing marks `LEAD OUTPUT`. What comes
//! after that on the real amp is reverb, the graphic equaliser and the power
//! stage, none of which is preamplifier.
//!
//! One number here is not off the drawing: the supply. Only capacitor voltage
//! ratings are printed, never the rail, so `SUPPLY` is a stated assumption
//! rather than a measurement, and it is the first thing to change if this
//! turns out not to bias where the real amp does.

use crate::dsp::netlist::{Circuit, Fault, Netlist, Taper, TriodeSpec};

pub const TREBLE: usize = 0;
pub const BASS: usize = 1;
pub const MIDDLE: usize = 2;
pub const VOLUME: usize = 3;
pub const LEAD_DRIVE: usize = 4;

/// The plate supply. Not printed on the schematic -- see the note above.
pub const SUPPLY: f64 = 410.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "lead_out")
}

/// The same preamplifier brought out at a chosen node, for measuring one
/// stage at a time.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Mark IIC+ preamp");

    // --- V1A --------------------------------------------------------------
    // R1 grid leak, and a cathode with two capacitors on it rather than one.
    // C1 is .47 uF straight to ground, which against 1.5 k bypasses from about
    // 226 Hz upwards and leaves the bottom of the band its degeneration --
    // that is where this amplifier's tightness starts. C2 is 22 uF through
    // R3 15 k, so below C1's corner the cathode still sees 1.5 k in parallel
    // with 15 k rather than 1.5 k alone.
    //
    // The PULL SHIFT BASS switch shorts R3, which puts 22 uF straight across
    // the cathode and turns the whole stage into a fully bypassed one. It is
    // a front-panel control and it is not modelled: the switch is out, which
    // is the stock position and the one the amplifier is known for.
    net.input("in", source)
        .resistor("in", "gnd", 1_000_000.0) // R1
        .resistor("v1a_k", "gnd", 1_500.0) // R2
        .capacitor("v1a_k", "gnd", 0.47e-6) // C1
        .capacitor("v1a_k", "v1a_shift", 22e-6) // C2
        .resistor("v1a_shift", "gnd", 15_000.0) // R3, shorted by PULL SHIFT BASS
        .supply("v1a_p", 150_000.0, SUPPLY) // R4
        .triode("v1a_p", "in", "v1a_k", ECC83);

    // --- the tone stack ---------------------------------------------------
    // A Fender stack, read off the drawing. R5 is the slope resistor, C4 and
    // C3 feed the bass and middle legs, the output is the treble wiper, and
    // the bass reaches that wiper through the lower half of the same pot --
    // which is what makes the three controls interact the way they famously
    // do.
    //
    // The treble capacitor is the part worth getting right. C5 250 pF goes
    // straight to the top of the treble pot; C6 750 pF gets there only
    // through R6, which is **10 M**, so with the TREBLE SHIFT light-dependent
    // resistor dark C6 contributes nothing and the treble cap is 250 pF.
    // LDR1A shorts R6 and makes it 1000 pF. This was built with R6 across the
    // pair instead of in series with C6, which is the treble shift jammed
    // permanently on -- a four-to-one error in the capacitor that decides
    // where the treble control works.
    net.capacitor("v1a_p", "c6", 750e-12) // C6
        .resistor("c6", "t_top", 10_000_000.0) // R6, shorted by TREBLE SHIFT
        .capacitor("v1a_p", "t_top", 250e-12) // C5
        .pot("t_top", "ts_out", "t_bot", 250_000.0, Taper::Linear, TREBLE)
        .resistor("v1a_p", "slope", 100_000.0) // R5
        .capacitor("slope", "t_bot", 0.1e-6) // C4
        .capacitor("slope", "b_bot", 0.047e-6) // C3
        // Both of these are wired as variable resistors, and a variable
        // resistor keeps `1 - fraction` of its track -- so a forward taper
        // *shorts them out* at the top of the travel and both controls run
        // backwards. Turned up they have to have more resistance in circuit,
        // not less.
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
        );

    // --- Volume 1 and V1B --------------------------------------------------
    // Volume 1 is not a level control and the manual is emphatic about it:
    // "Generally you will want to run the Volume 1 control as high as possible
    // without causing unwanted distortion, in order to have available the most
    // possible sustain when switching into the Lead mode... The VOLUME 1
    // control determines much about the sound and feel of both the Modes, and
    // is called GAIN in many amplifiers as that is what it meters."
    //
    // The plugin has one Drive knob and it turns the Lead Drive, so Volume 1
    // is a control nobody turns -- and a control nobody turns was sitting at
    // the middle of its travel, which on an audio track is twenty decibels
    // down and is the last place the manual would put it. Eight tenths: high,
    // as asked, and short of the stop so the stage in front of the lead
    // section is not itself the thing distorting. This is a stated playing
    // position, not a value off the drawing.
    net.rest(VOLUME, 0.8)
        .pot("ts_out", "v1b_g", "gnd", 1_000_000.0, Taper::Audio, VOLUME)
        .resistor("v1b_k", "gnd", 1_500.0) // R7
        .capacitor("v1b_k", "gnd", 22e-6) // C13
        .supply("v1b_p", 100_000.0, SUPPLY) // R8
        .triode("v1b_p", "v1b_g", "v1b_k", ECC83)
        // C7 lands on a node of its own, not on the lead chain. R9 is 91 k
        // from there to ground -- 100 k on an RP11A, and this is an RP10A --
        // and it is most of what V1B's plate actually works into: 100 k of
        // plate load against 91 k of shunt rather than against the megohm the
        // lead drive presents. C21 then takes the signal on to the lead
        // section. Both were missing, and V1B was about three decibels louder
        // than the drawing for it.
        //
        // R10 3.3 M with C10 10 pF across it also leaves this node, feeding
        // the pre-lead signal forward to the lead *return*. That return is
        // past where this model stops, so the network is not built; it is not
        // an omission from the lead path.
        .capacitor("v1b_p", "v1b_out", 0.1e-6) // C7
        .resistor("v1b_out", "gnd", 91_000.0) // R9, 91 k on RP10A
        .capacitor("v1b_out", "lead_in", 0.02e-6); // C21

    // --- the lead drive control -------------------------------------------
    // R21 is marked "680K, 1M on ++", and the 330 k drawn beside the pot is
    // inside the dashed box marked "++ MOD" -- both belong to the
    // modification, not to a stock C+. The 330 k is across the pot rather
    // than under it, so leaving it in would load the control rather than lift
    // its bottom off ground; either way it is not stock.
    net.resistor("lead_in", "ld_top", 680_000.0) // R21
        .pot(
            "ld_top",
            "v3b_g",
            "gnd",
            1_000_000.0,
            Taper::Audio,
            LEAD_DRIVE,
        );

    // --- V3B ---------------------------------------------------------------
    net.resistor("v3b_g", "gnd", 475_000.0) // R22
        .capacitor("v3b_g", "v3b_k", 120e-12) // C22
        .resistor("v3b_k", "gnd", 1_500.0) // R23
        .capacitor("v3b_k", "gnd", 2.2e-6) // C23
        .supply("v3b_p", 82_000.0, SUPPLY) // R26
        .triode("v3b_p", "v3b_g", "v3b_k", ECC83);

    // --- into V4A ----------------------------------------------------------
    // R25 in series and R24 to ground make a divider between the two stages,
    // with C24 across R24 taking the top off what gets through. Two triodes
    // in a row with an attenuator between them is how a lead channel gets its
    // gain without simply oscillating.
    net.capacitor("v3b_p", "c25", 0.022e-6) // C25
        .resistor("c25", "v4a_g", 270_000.0) // R25
        .resistor("v4a_g", "gnd", 68_000.0) // R24
        .capacitor("v4a_g", "gnd", 1000e-12) // C24
        // The cathode network, and it is not the usual resistor-with-a-cap.
        // R30 3.3 k is the cathode resistor. C29 .22 uF and R29 6.8 k are a
        // *series* leg in parallel with it, so above C29's corner near 106 Hz
        // the cathode sees 3.3 k in parallel with 6.8 k -- 2.2 k, not nothing.
        // This stage keeps its degeneration across the audio band, which is
        // six decibels of gain and a different way of clipping from a stage
        // that is simply bypassed. It was built as R29 straight to ground with
        // C29 across it, which is a fully bypassed stage and neither of those
        // things. PULL BRIGHT LEAD MASTER shorts R29; the switch is out here.
        .resistor("v4a_k", "gnd", 3_300.0) // R30
        .capacitor("v4a_k", "v4a_bright", 0.22e-6) // C29
        .resistor("v4a_bright", "gnd", 6_800.0) // R29, shorted by PULL BRIGHT
        .supply("v4a_p", 274_000.0, SUPPLY) // R27
        .capacitor("v4a_p", "gnd", 1000e-12) // C27, across R27 to the rail
        .triode("v4a_p", "v4a_g", "v4a_k", ECC83);

    // --- out ---------------------------------------------------------------
    net.capacitor("v4a_p", "c30", 0.047e-6) // C30
        .resistor("c30", "lead_out", 220_000.0) // R31
        .capacitor("c30", "lead_out", 250e-12) // C31, across R31
        .resistor("lead_out", "gnd", 100_000.0) // R32
        .capacitor("lead_out", "gnd", 500e-12) // C32
        .resistor("lead_out", "gnd", load);

    net.build(at)
}

// ---------------------------------------------------------------------------
// The five band graphic equaliser
// ---------------------------------------------------------------------------

/// The five sliders, in the order the front panel has them.
pub const BAND_60: usize = 0;
pub const BAND_240: usize = 1;
pub const BAND_750: usize = 2;
pub const BAND_2200: usize = 3;
pub const BAND_6600: usize = 4;

/// The slider track. **Not on the drawing** -- neither sheet prints a value
/// beside the five sliders -- so this is the figure the Mesa graphic is
/// commonly built with, and it is the one number here that is an assumption
/// rather than a reading. It sets how deep the bands go: the ratio between it
/// and each band's series resistor is the band's range.
const SLIDER: f64 = 100_000.0;

/// What the driver puts back.
///
/// The network is five shunts across one node, so even with every slider
/// centred it throws away **16.6 dB** at 1 kHz into the impedance this circuit
/// is driven from. That is what the four transistors are there for: Q1 to Q4
/// are a recovery amplifier making up the equaliser's insertion loss, and this
/// is the one number of theirs that reaches the sound.
///
/// Measured at the sliders' centres, at 1 kHz, into `voice::SOURCE` and
/// `voice::LOAD` -- the same place the chain builds it. Not flat across the
/// band: the network tilts about two and a half decibels either side of this,
/// because five resonant shunts in parallel do not add up to a flat load. That
/// tilt is the network's own and is left in.
pub const DRIVER_GAIN_DB: f64 = 16.64;

/// The slider's law, chosen by measurement -- see `examples/graphic.rs`.
const TAPER: Taper = Taper::ReverseLog { span: 200.0 };

/// One band: its series resistor, inductor and capacitor.
///
/// Read off `docs/schematics/RP10 IIC+ Schematic.pdf`, which the FINAL sheet
/// carries identically. The frequencies are the drawing's own labels for the
/// sliders; they are **not** the series resonance of L and C, which lands at
/// 88, 372, 723, 1576 and 4823 Hz. That is not an error in either place -- a
/// band's branch is loaded by the slider it hangs off and by the four bands
/// beside it, and where the assembled network peaks is not where an isolated
/// LC would.
const BANDS: [(f64, f64, f64); 5] = [
    (470.0, 1.0, 3.3e-6),       // R51, L1, C51 -- 60 Hz
    (470.0, 0.39, 0.47e-6),     // R52, L2, C52 -- 240 Hz
    (470.0, 0.22, 0.22e-6),     // R53, L3, C53 -- 750 Hz
    (1_000.0, 0.068, 0.15e-6),  // R54, L4, C54 -- 2200 Hz
    (1_000.0, 0.033, 0.033e-6), // R55, L5, C55 -- 6600 Hz
];

/// The graphic equaliser, from `EQ INPUT` to `EQ OUTPUT`.
///
/// **Where it sits.** The drawing takes `EQ INPUT` from `LEAD OUTPUT`, which is
/// exactly where `build` above stops, and returns `EQ OUTPUT` to the phase
/// inverter. So this goes between the preamplifier and the power amplifier,
/// which is the whole reason it matters: §9.8 -- a low band boosted here is
/// boosted *after* four gain stages and lands on the power stage, and moving
/// it to the end of the chain would be a different amplifier.
///
/// **The topology, traced.** Two rails. Each slider sits across them, and its
/// wiper drives a series R-L-C back to the lower rail. The lower rail is
/// **ground**, switched: it runs to the LDR5A, which is the equaliser's
/// footswitch, with the 68 k beside it marked "EQ POP FIX. FACTORY MOD" --
/// with the cell dark the rail floats and every branch is an open circuit,
/// which is the equaliser out of circuit and flat.
///
/// So each band is a series-resonant branch shunting the signal to ground
/// through its own slider. At the band's frequency the branch is about its
/// series resistor -- 470 ohms against a 100 k track -- and away from it the
/// branch is an open circuit and the track stands alone. Sliding toward the
/// signal end brings that low impedance across the node and takes the band
/// out; sliding the other way shorts the branch and leaves it. With the
/// sliders at their centres as the reference, that is the boost and cut either
/// side of flat that Mesa's own figure of +/- 12 dB describes.
///
/// Getting this the other way round is the mistake worth recording: read as a
/// series network between two live rails, with the next stage's megohm across
/// the output, the whole equaliser moved the response by **0.1 dB** at every
/// slider and every frequency. A series impedance into a load a hundred times
/// larger does nothing, and that is what says the lower rail is ground.
///
pub fn graphic(source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Mark IIC+ graphic EQ");
    net.input("eq", source);
    for (band, &(r, l, c)) in BANDS.iter().enumerate() {
        let wiper = format!("w{band}");
        let after_r = format!("m{band}");
        let after_l = format!("n{band}");
        net.pot("eq", &wiper, "gnd", SLIDER, TAPER, band)
            .resistor(&wiper, &after_r, r)
            .inductor(&after_r, &after_l, l)
            .capacitor(&after_l, "gnd", c);
    }
    net.resistor("eq", "gnd", load);
    net.build("eq")
}
