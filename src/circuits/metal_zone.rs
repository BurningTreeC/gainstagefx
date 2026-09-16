//! The Metal Zone: two gain stages, a diode pair, and seven filters.
//!
//! Engineering reference (developer documentation, not panel text): Boss MT-2
//! Metal Zone, built from Boss's own drawing and cross-checked against Electric
//! Druid's analysis of the same sheet. See `docs/models/metal_zone.md`.
//!
//! This pedal is made of **frequency response**, not of diodes. Its own analyst
//! puts it better than a comment can: "It's really all about frequency
//! response. It's heavily shaped every step of the way, and those frequency
//! responses determine the sound of the pedal, much more than diode choice ever
//! would." Before the clipper there is a hump at a kilohertz that decides what
//! gets distorted; after it, two resonances at the ends of the band with a
//! scoop between them; and then a three-band equaliser with a **swept middle**,
//! which is a control nothing else in this plugin has.
//!
//! Every one of those is a **gyrator**: an op-amp wired so the current into it
//! lags the voltage, which is an inductor made without winding one. They are
//! built here as the inductances they stand for, `L = R1 x R2 x C`, the same
//! way `heavy_metal` and the Mark IIC+'s graphic are. Working them out from the
//! drawing's values and pairing each with its series capacitor gives 953 Hz,
//! 4894 Hz, 105 Hz and 106 Hz -- against the published 1 kHz, 4,898 Hz, 105 Hz
//! and 105 Hz. That agreement is the check this model rests on.

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, JfetSpec, Netlist, Taper};

/// VR01, the Dist control, 250 k audio.
pub const DIST: usize = 0;
/// VR03a, Low, 100 k.
pub const LOW: usize = 1;
/// VR02b, Middle, 100 k.
pub const MIDDLE: usize = 2;
/// VR02a, the Mid Freq control: a 50 k dual gang, both sections of the one knob,
/// which is what lets the centre move without the shape moving with it.
pub const MID_FREQ: usize = 3;
/// VR03b, High, 100 k.
pub const HIGH: usize = 4;
/// VR04, the Level control, 50 k audio.
pub const LEVEL: usize = 5;

/// Where the level control rests: **unity through the pedal**, which is what
/// the panel's Level knob means at noon. See `rodent::VOLUME_REST`.
///
/// Measured *broadband* (`examples/pedallevel.rs`) rather than on one tone, and
/// for this pedal the two are eight decibels apart: it is shaped hard enough --
/// a hump at 950 Hz, a scoop, then a three-band equaliser -- that its level at
/// 220 Hz says very little about how loud it is. The other pedals are flat
/// enough for the distinction not to arise. At this position it sits 1.6 dB
/// above no pedal at all, in the middle of the list.
pub const LEVEL_REST: f64 = 0.45;

/// M5218AL: a 10 MHz, 5 V/us dual. The op-amps are built as ideal amplifiers
/// with a rail, as the Green 808's are; what that leaves out is the slew limit,
/// which matters far less here than in a Rodent because nothing in this pedal
/// asks an amplifier to follow an edge it cannot.
const SWING: f64 = 4.0;

/// 1SS133, a small-signal silicon switching diode. No SPICE model located, so
/// it takes 1N4148-class constants. APPROXIMATED.
const CLIPPER: DiodeSpec = DiodeSpec::SILICON;

/// 2SK184GR / 2SK118Y, the JFETs either end of the pedal.
const JFET: JfetSpec = JfetSpec::J201;
/// 2SC3378GR, which the catalogue already carries.
const NPN: BipolarSpec = BipolarSpec::NPN_2SC3378;

/// One resonant leg: a gyrator's inductance, its series resistance and the
/// capacitor in front of it.
///
/// | | from the drawing | L | with | resonance |
/// |---|---|---|---|---|
/// | the hump | 47k, 2.2k, .01 | 1.034 H | .027 uF | **953 Hz** |
/// | post high | 47k, 1k, .015 | 0.705 H | .0015 uF | **4894 Hz** |
/// | post low | 470k, 470, .22 | 48.6 H | .047 uF | **105 Hz** |
/// | Low control | 100k, 2.2k, .22 | 48.4 H | .047 uF | **106 Hz** |
type Leg = (f64, f64, f64);

const HUMP: Leg = (1.034, 2_200.0, 0.027e-6);
const POST_HIGH: Leg = (0.705, 1_000.0, 0.0015e-6);
const POST_LOW: Leg = (48.598, 470.0, 0.047e-6);
const LOW_BAND: Leg = (48.4, 2_200.0, 0.047e-6);

/// Hangs one resonant leg between a node and ground.
fn leg(net: &mut Netlist, from: &str, prefix: &str, (l, r, c): Leg) {
    let a = format!("{prefix}_a");
    let b = format!("{prefix}_b");
    net.capacitor(from, &a, c)
        .inductor(&a, &b, l)
        .resistor(&b, "gnd", r);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Metal Zone");

    // --- supply and bias --------------------------------------------------------------
    // R056/R057 10 k each make the 4.5 V the input buffer sits on.
    net.supply("v9", 47.0, 9.0)
        .capacitor("v9", "gnd", 100e-6) // C040
        .resistor("v9", "vref", 10_000.0) // R056
        .resistor("vref", "gnd", 10_000.0) // R057
        .capacitor("vref", "gnd", 100e-6); // C019

    // --- input buffer, Q011 -------------------------------------------------------------
    net.input("in", source)
        .resistor("in", "c042", 10_000.0) // R059
        .capacitor("c042", "g1", 0.047e-6) // C042
        .resistor("g1", "vref", 1_000_000.0) // R058
        .jfet("v9", "g1", "s1", JFET)
        .resistor("s1", "gnd", 10_000.0); // R060

    // Ground-referenced from here on, with the buffer coupled in. The pedal
    // itself runs everything on the 4.5 V bias; an amplifier clipping at
    // 4.5 V +- 4 V and one clipping at 0 V +- 4 V do the same thing to a
    // signal, and the solver is far happier with the second. The same
    // construction `heavy_metal` and `rodent` use, and for the same reason.
    net.capacitor("s1", "pre", 1e-6).resistor("pre", "gnd", 1_000_000.0);

    // --- the hump, op-amp 3b ------------------------------------------------------------
    // A non-inverting stage whose gain leg is a resonant shunt: at 953 Hz the
    // leg is only its own resistance, so the gain rises to about 1 + 220k/2.2k
    // there and sits near unity everywhere else. That is the mid hump, and it
    // is *before* the clipper -- it decides which part of the guitar gets
    // distorted, which is most of why this pedal sounds the way it does.
    net.capacitor("pre", "u3b_p", 0.015e-6) // C033
        .resistor("u3b_p", "gnd", 100_000.0) // R043, 106 Hz with C033
        .opamp("u3b", "u3b_p", "u3b_m", SWING)
        .resistor("u3b", "u3b_m", 220_000.0) // R044
        .capacitor("u3b", "u3b_m", 100e-12); // C032, 7.2 kHz
    leg(&mut net, "u3b_m", "hump", HUMP);

    // Out through the divider and its lowpass: R045/R042 10 k each, and R045
    // against C031 turns the top over at about 340 Hz.
    net.resistor("u3b", "mid", 10_000.0) // R045
        .resistor("mid", "gnd", 10_000.0) // R042
        .capacitor("mid", "gnd", 0.047e-6); // C031

    // --- the gain stage, op-amp 3a -------------------------------------------------------
    // Non-inverting, with the Dist control in the feedback: R041 1 k and VR01
    // 250 k against R051 1 k, so the gain runs from 1 + 1k/1k = 2 to
    // 1 + 251k/1k = 252. C028 47 p holds the top down above 13 kHz.
    net.capacitor("mid", "u3a_p", 0.033e-6) // C029
        .resistor("u3a_p", "gnd", 100_000.0) // R040, 48 Hz
        .opamp("u3a", "u3a_p", "u3a_m", SWING)
        .resistor("u3a", "dist_w", 1_000.0) // R041
        // A rheostat with its wiper tied to `b`, so what is in circuit is the
        // a-to-wiper leg, `R x (1 - f)`. A forward taper would run the control
        // backwards; the reverse one puts the gain up as the knob goes up.
        .pot("dist_w", "u3a_m", "u3a_m", 250_000.0, Taper::ReverseAudio, DIST) // VR01
        .capacitor("u3a", "u3a_m", 47e-12) // C028
        .resistor("u3a_m", "gnd", 1_000.0); // R051

    // --- the clipper ----------------------------------------------------------------------
    // R033 2.2 k into a 1SS133 pair to ground, then the divider that follows it.
    net.resistor("u3a", "clip", 2_200.0) // R033
        .diode("clip", "gnd", CLIPPER) // D003
        .diode("gnd", "clip", CLIPPER) // D004
        .resistor("clip", "post", 10_000.0) // R032
        .resistor("post", "gnd", 4_700.0) // R031
        .capacitor("post", "gnd", 0.015e-6); // C023

    // --- the scoop, op-amp 4b -------------------------------------------------------------
    // Two resonant legs in the gain leg at once: the stage lifts 105 Hz and
    // 4894 Hz and leaves the middle where it was, which is the double peak --
    // and the scoop between them -- this pedal is bought for.
    // R030 sits *in series* with the legs rather than across them. Across, it
    // is a gain leg of its own and the stage lifts everything by thirty-one
    // times, resonance or not, which is not a scoop but a gain stage; in
    // series, the legs are a high impedance away from their resonances and the
    // stage sits at unity everywhere except the two ends of the band.
    net.opamp("u4b", "post", "u4b_m", SWING)
        .resistor("u4b", "u4b_m", 100_000.0) // R029
        .capacitor("u4b", "u4b_m", 47e-12) // C022
        .resistor("u4b_m", "u4b_legs", 3_300.0); // R030
    leg(&mut net, "u4b_legs", "plow", POST_LOW);
    leg(&mut net, "u4b_legs", "phigh", POST_HIGH);

    // --- the equaliser ---------------------------------------------------------------------
    // The three tone controls, built the way the Mark IIC+'s graphic is: each
    // track runs between the two inputs of one amplifier with its own leg on
    // the wiper, so it boosts toward one end, cuts toward the other and is flat
    // in the middle. Published range: +-20 dB on Low and High, +-15 dB on the
    // middle.
    net.resistor("u4b", "cut", 22_000.0) // R028
        .opamp("u4a", "cut", "boost", SWING)
        .resistor("u4a", "boost", 22_000.0) // R026
        .capacitor("u4a", "boost", 10e-12); // C018

    // VR03a Low: the 106 Hz resonance.
    net.pot("boost", "w_low", "cut", 100_000.0, Taper::Symmetric { span: 150.0 }, LOW);
    leg(&mut net, "w_low", "blow", LOW_BAND);

    // VR03b High: a shelf rather than a resonance -- R061 2.2 k and C044 .01
    // let everything above about 7 kHz through the track and nothing below it.
    net.pot("boost", "w_high", "cut", 100_000.0, Taper::Symmetric { span: 150.0 }, HIGH)
        .capacitor("w_high", "bhigh", 0.01e-6) // C044
        .resistor("bhigh", "gnd", 2_200.0); // R061

    // VR02b Middle, over a Wien network whose corner the Mid Freq knob sweeps.
    //
    // A Wien bandpass is a series C-R into a parallel R-C, and its centre is
    // 1 / (2 pi R sqrt(C1 C2)) -- so moving *both* resistances together moves
    // the centre and leaves the shape alone. That is what the dual-gang VR02a
    // is for, and here both halves carry the same control index, which is what
    // ganged means. Measured from the values: 5.4 kHz with the knob down and
    // 227 Hz with it up, against the published 4.7 kHz and 240 Hz.
    // VR02b Middle, over a gyrator whose **both** resistances the Mid Freq gang
    // sweeps at once. This is the one control in the plugin that moves where it
    // works rather than how much.
    //
    // The original sweeps with a Wien bridge, and what makes that a *parametric*
    // mid rather than merely a moving one is two properties together: its centre
    // goes as `1/R`, and its Q does not move with it. A gyrator gives
    // `L = R1 x R2 x C` and a leg of `Rs + sL` in series with `Cs`, so sweeping
    // one resistance gives `f0` proportional to `1/sqrt(R)` -- a fifth of the
    // span -- and a Q that slides with it. That was built, measured and thrown
    // away.
    //
    // Sweeping **both** gyrator resistances together is the answer, and it is
    // what the dual gang is for:
    //
    // - `L = R^2 C_gyr`, so `f0 = 1 / (2 pi R sqrt(C_gyr Cs))` -- proportional
    //   to `1/R`, exactly as the Wien bridge;
    // - the leg's series resistance is `R1 = R` and it moves with it, so
    //   `Q = sqrt(C_gyr / Cs)` -- **a constant, set by the two capacitors and
    //   nothing else.**
    //
    // With the drawing's own C036 .022 against a .01 series capacitor that is a
    // Q of 1.48 held across the whole sweep, and 2.2 k to 52.2 k of gang gives
    // **4877 Hz down to 206 Hz**, against a published 4.7 kHz to 240 Hz.
    // Where the sweep sits when nothing turns it. The pedal slot gives it a
    // knob; selected as a circuit it has bass, middle and treble and no fourth
    // control, so it rests at noon -- which for a frequency sweep on a linear
    // track is the middle of its range and not, as it would be on an audio
    // track, twenty decibels down.
    net.rest(MID_FREQ, 0.5)
        .pot("boost", "w_mid", "cut", 100_000.0, Taper::Symmetric { span: 150.0 }, MIDDLE)
        .capacitor("w_mid", "mid_leg", 0.01e-6) // the series capacitor, Cs
        // R1: the leg's series resistance and half of the inductance.
        .resistor("mid_leg", "mid_r1", 2_200.0) // R048
        .pot("mid_r1", "gyr_out", "gyr_out", 50_000.0, Taper::Linear, MID_FREQ)
        // C_gyr and R2: the other half.
        .capacitor("mid_leg", "gyr_in", 0.022e-6) // C036
        .resistor("gyr_in", "mid_r2", 2_200.0) // R062
        .pot("mid_r2", "gnd", "gnd", 50_000.0, Taper::Linear, MID_FREQ)
        // op-amp 2b, the follower the drawing puts here.
        .opamp("gyr_out", "gyr_in", "gyr_out", SWING);

    // --- Level and the output buffer --------------------------------------------------------
    net.resistor("u4a", "lvl_top", 22_000.0) // R014
        .rest(LEVEL, LEVEL_REST)
        .pot("lvl_top", "lvl", "gnd", 50_000.0, Taper::Audio, LEVEL) // VR04
        .capacitor("lvl", "b1", 10e-6) // C005
        .resistor("b1", "vref", 1_000_000.0) // R005
        .bipolar("v9", "b1", "e1", NPN)
        .resistor("e1", "gnd", 10_000.0) // R002
        .capacitor("e1", "out", 10e-6) // C001
        .resistor("out", "gnd", 100_000.0) // R003
        .resistor("out", "gnd", load);

    net.build(at)
}
