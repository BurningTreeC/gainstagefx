//! The Mesa Boogie Mark IIC+ lead-channel preamplifier, from the schematic.
//!
//! Six triode stages and a Fender tone stack between the first two of them.
//! `INPUT` through V1A, the stack, Volume 1, V1B, then the lead drive control
//! into V3B and V4A to `LEAD OUTPUT`, back through the lead return into V2B,
//! the Lead Master, and V2A, ending where the drawing marks `TO EQ`. What comes
//! after that on the real amp is the graphic equaliser and the power stage.
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
/// The Lead Master, between V2B and V2A. See `tap`.
pub const LEAD_MASTER: usize = 5;

/// Where the Lead Master rests: five on the dial. A stated playing position,
/// like Volume 1's, not a value off the drawing.
pub const LEAD_MASTER_REST: f64 = 0.5;

/// The plate supply. Not printed on the schematic -- see the note above.
pub const SUPPLY: f64 = 410.0;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The whole lead channel, from the input jack to `TO EQ`.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "to_eq")
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
        //
        // The Bass pot is an **audio** track: "250k A" on the SLOCLONE trace
        // (Mesa_MK2C+_SLOCLONE_v2, rev 2.0, 2020), and Mesa's replacement part
        // A250K LOG is listed for "MK II/III Bass, Treble, Master". It was
        // built linear, which put 30 % of the track -- all but 1.5 dB of the
        // bass there is -- in circuit at a setting of 3. That is the setting
        // Mesa's manual gives for a high Volume 1 precisely *because* it is
        // most of the way down an audio track. The middle is "10k B" on the
        // same trace. The treble is marked "250k B" there against the parts
        // listing's A; the trace, which read this amplifier's own pots, is
        // followed. See `docs/models/cali_iic_plus.md`.
        .pot(
            "t_bot",
            "b_bot",
            "b_bot",
            250_000.0,
            Taper::ReverseAudio,
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
        // the pre-lead signal forward to the lead return at V2B's grid. It is
        // built with the recovery stages below.
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

    // --- the lead output ------------------------------------------------------
    net.capacitor("v4a_p", "c30", 0.047e-6) // C30
        .resistor("c30", "lead_out", 220_000.0) // R31
        .capacitor("c30", "lead_out", 250e-12) // C31, across R31
        .resistor("lead_out", "gnd", 100_000.0) // R32
        .capacitor("lead_out", "gnd", 500e-12); // C32

    // --- the lead return and V2B ---------------------------------------------
    // This model used to stop at `LEAD OUTPUT` and hand that straight to the
    // graphic equaliser, which left out two triodes. On both drawings
    // (`RP10 IIC+ Schematic.pdf` and `Mark IIC+ Schematic FINAL.pdf`) LDR3A
    // takes `LEAD OUTPUT` back to V2B's grid, where the rhythm signal also
    // arrives from V1B through R10 3.3 M with C10 10 pF across it, and R11 680 k
    // with C11 47 pF shunts the node. V2B has a 100 k plate and a 1.5 k cathode
    // with nothing across it (the 15 uF there is marked "++ MOD"). At a
    // guitar's level the lead output is several volts, so V2B clips: it is the
    // fifth gain stage of the lead channel, and without it the amplifier was
    // audibly short of the saturation it is known for.
    //
    // LDR3A is lit in lead mode and is modelled as a short.
    net.resistor("v1b_out", "lead_out", 3_300_000.0) // R10
        .capacitor("v1b_out", "lead_out", 10e-12) // C10
        .resistor("lead_out", "gnd", 680_000.0) // R11
        .capacitor("lead_out", "gnd", 47e-12) // C11
        .resistor("v2b_k", "gnd", 1_500.0) // R16, unbypassed
        .supply("v2b_p", 100_000.0, SUPPLY) // R19 on RP10, R13 on FINAL
        .triode("v2b_p", "lead_out", "v2b_k", ECC83);

    // --- the Lead Master ------------------------------------------------------
    // R105 47 k and C9 .047 uF, then the Lead Master: a 250 k track wired as a
    // rheostat to ground through LDR4A, so it shunts the node rather than
    // dividing it, and turned up it takes resistance *out* of the shunt.
    // Behind it R102 150 k into R101 4.7 k is a thirty decibel pad, which is
    // where the reverb return joins and the effects send leaves.
    net.resistor("v2b_p", "r105", 47_000.0) // R105
        .capacitor("r105", "lead_master", 0.047e-6) // C9
        .rest(LEAD_MASTER, LEAD_MASTER_REST)
        .pot(
            "lead_master",
            "gnd",
            "gnd",
            250_000.0,
            Taper::ReverseAudio,
            LEAD_MASTER,
        )
        .resistor("lead_master", "send", 150_000.0) // R102
        .resistor("send", "gnd", 4_700.0) // R101
        // The RETURN jack is normalled to SEND with no cable in the loop. R103
        // 47 k is at the return. R12 2.2 k in the send is marked "a later edition
        // factory mod" and is left out.
        .resistor("send", "gnd", 47_000.0); // R103

    // --- V2A, the recovery stage ---------------------------------------------
    // R104 1 k at the cathode with C16 .47 uF straight across it, so the stage
    // is bypassed from about 340 Hz up. C15 22 uF reaches ground only through
    // R20 68 k unless the GAIN BOOST (pull) switch shorts R20; the switch is in,
    // which is stock. The 120 pF from grid to cathode is marked "removed after
    // August 1984" and "don't think present on RP10A", and is not built.
    net.resistor("v2a_k", "gnd", 1_000.0) // R104
        .capacitor("v2a_k", "gnd", 0.47e-6) // C16
        .capacitor("v2a_k", "boost", 22e-6) // C15
        .resistor("boost", "gnd", 68_000.0) // R20, shorted by GAIN BOOST
        .supply("v2a_p", 120_000.0, SUPPLY) // R13 on RP10, R19 on FINAL
        .triode("v2a_p", "send", "v2a_k", ECC83);

    // --- to the equaliser -----------------------------------------------------
    // C12 .047 uF to `TO EQ`. The MASTER control (a 1 M rheostat here) is what
    // the power stage's master pot stands in for: the equaliser between them is
    // linear, so the order of the two changes loading and not the sound. R106
    // 15 k is a later mod and is left out. `load` is the equaliser's 1 M input.
    net.capacitor("v2a_p", "to_eq", 0.047e-6) // C12
        .resistor("to_eq", "gnd", load);

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

/// The slider track, 50 k. **Not on either drawing**: the value is the one Mesa's
/// replacement slide pot for the Mark I-IV equaliser is sold as (Tube Amp Doctor
/// part Z-MB-S50K). Its law is `TAPER`.
const SLIDER: f64 = 50_000.0;

/// The equaliser's amplifier, Q2-Q4, as an op-amp with a rail it never meets.
///
/// On the drawings the equaliser is fed from V2A through the MASTER control, so it
/// never sees more than a few volts. Here the power stage's master pot stands in
/// for MASTER and sits *after* the equaliser, so the equaliser is handed the
/// recovery stage's full swing. A realistic rail would clip it there, which the
/// amplifier does not do; so the transistors' own limits are not modelled.
const EQ_RAIL: f64 = 1_000.0;

/// The slider's law. ESTIMATED: neither the drawings nor the replacement part
/// give it. A linear track moves each band by about 1 dB over the middle half of
/// its travel and does everything in the last few millimetres, because a 470 ohm
/// branch against 3.32 k only acts with the wiper close to an end. A dual-slope
/// track with a span of 150 puts about half of each band's range at a quarter of
/// the travel either side of a flat centre (measured: -14 to +14 dB at 60 Hz,
/// ±6 at a quarter), which is how the sliders behave in use.
const TAPER: Taper = Taper::Symmetric { span: 150.0 };

/// One band: its series resistor, inductor and capacitor.
///
/// Read off `docs/schematics/RP10 IIC+ Schematic.pdf`, which the FINAL sheet
/// carries identically. The frequencies are the drawing's own labels for the
/// sliders; the series resonances land at 88, 372, 723, 1576 and 4823 Hz, and
/// where the assembled network peaks is decided by the feedback loop around it.
const BANDS: [(f64, f64, f64); 5] = [
    (470.0, 1.0, 3.3e-6),       // R51, L1, C51 -- 60 Hz
    (470.0, 0.39, 0.47e-6),     // R52, L2, C52 -- 240 Hz
    (470.0, 0.22, 0.22e-6),     // R53, L3, C53 -- 750 Hz
    (1_000.0, 0.068, 0.15e-6),  // R54, L4, C54 -- 2200 Hz
    (1_000.0, 0.033, 0.033e-6), // R55, L5, C55 -- 6600 Hz
];

/// The graphic equaliser, from `EQ INPUT` to `EQ OUTPUT`.
///
/// **What it is.** A feedback equaliser, not a passive network. Q1 is an emitter
/// follower (C41 .1 uF in, R42/R43 470 k bias, R41 1 M at the input). From its
/// emitter R46 3.32 k feeds Q2's base; Q2 and Q3 are a long-tailed pair on R47
/// 22.1 k, Q4 follows Q3's collector, and R48 3.32 k (with C47 10 pF) brings the
/// output back to Q3's base. So with nothing else connected it is a unity-gain
/// amplifier whose two inputs are fed through equal resistors.
///
/// **Where the sliders go.** Every slider's track runs between those two inputs --
/// the drawings join the bottoms to Q2's base and the tops to Q3's base -- and its
/// wiper drives a series R-L-C to the switched ground (LDR5A lit). Toward Q2's
/// side the branch shunts the input and cuts its band; toward Q3's side it shunts
/// the feedback and boosts it; centred, the two cancel. That is the ±12 dB
/// equaliser Mesa describes.
///
/// **The mistake this replaces.** It was built with every track from the signal
/// to ground and a fixed 16.6 dB "driver" make-up after it: the network could
/// cut 12 dB but boost less than 3, so every V that player's set came out as a
/// cut in the middle with almost nothing added at the ends. Measured and
/// recorded in `docs/models/cali_iic_plus.md`.
pub fn graphic(source: f64, load: f64) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Mark IIC+ graphic EQ");
    net.input("eq", source)
        .resistor("eq", "gnd", 1_000_000.0) // R41
        .capacitor("eq", "q1b", 0.1e-6) // C41
        .resistor("q1b", "gnd", 235_000.0) // R42 || R43, to AC ground
        .opamp("q1e", "q1b", "q1e", EQ_RAIL) // Q1, the follower
        .resistor("q1e", "cut", 3_320.0) // R46, into Q2's base
        .opamp("q4e", "cut", "boost", EQ_RAIL) // Q2/Q3/Q4
        .resistor("q4e", "boost", 3_320.0) // R48, into Q3's base
        .capacitor("q4e", "boost", 10e-12); // C47
    for (band, &(r, l, c)) in BANDS.iter().enumerate() {
        let wiper = format!("w{band}");
        let after_r = format!("m{band}");
        let after_l = format!("n{band}");
        // Fraction 1 puts the wiper at the boost end: a slider pushed up boosts.
        net.pot("boost", &wiper, "cut", SLIDER, TAPER, band)
            .resistor(&wiper, &after_r, r)
            .inductor(&after_r, &after_l, l)
            .capacitor(&after_l, "gnd", c);
    }
    net.capacitor("q4e", "eqout", 10e-6) // C48
        .resistor("eqout", "gnd", load);
    net.build("eqout")
}
