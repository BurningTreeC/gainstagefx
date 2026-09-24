//! The Brit 2205 preamplifier: the boost channel of a 50 W British
//! split-channel head, in its later circuit, from the manufacturer's drawing.
//!
//! Engineering reference (developer documentation, not panel text): Marshall
//! JCM800 2205 Split Channel Reverb, "2205 Preamp", issue 2 of 18-5-88 (issue 1
//! 6/3/87), PCB JM83. See `docs/models/brit_2205.md`, which also records why
//! this is the later of the model's two circuits.
//!
//! The same family name as the Brit 800 and a different preamplifier. The
//! first stage is shared by both channels; the boost channel then has two
//! things the 2203 does not:
//!
//! - **V1B is diode-biased.** D1, a 1N4007, sits across its 10 k cathode
//!   resistor. The valve would idle with three volts or so on that resistor;
//!   the diode holds the cathode at about 0.6 V instead and, conducting, is a
//!   few tens of ohms in parallel with it. So the stage runs hot and fully
//!   bypassed on a 470 k plate load -- as much gain as an ECC83 gives.
//! - **A diode clipper.** Past the second gang of the Gain pot the signal goes
//!   through R20 27 k, with C16 2 nF across it to let the top through, into a
//!   W005 bridge rectifier with a 1N4007 across its DC terminals. Whichever
//!   way the current flows it crosses three junctions, so the node is held to
//!   about ±1.4 V -- three junctions at the small currents 27 k lets through, and
//!   softly, because it is held through that 27 k.
//!
//! The Gain knob turns both gangs of one dual pot, VR4A before V1B and VR4B
//! after it, so the knob sets how hard V1B is driven *and* how hard the clipper
//! is. Then comes the channel's Volume, V3A, the stack, the master, a passive
//! mix with the reverb and the loop send, and V4A, which makes up that mix's
//! loss and drives the phase inverter. This netlist ends at V4A's plate.
//!
//! The Normal channel is muted when the boost channel is selected -- its second
//! stage's grid is shorted by the switching transistor through C40 -- but its
//! passive network still hangs on V1A's plate and its output still hangs on the
//! master node, so both are built as the loads they are.

use crate::dsp::netlist::{Circuit, DiodeSpec, Fault, Netlist, Taper, TriodeSpec};

/// VR4A and VR4B, the boost channel's Gain: both gangs of one 1 M log pot.
/// The Drive knob turns it.
pub const GAIN: usize = 0;
/// VR6, boost Treble, 220 k linear.
pub const TREBLE: usize = 1;
/// VR8, boost Bass, 1 M, wired as a variable resistor to ground.
pub const BASS: usize = 2;
/// VR7, boost Middle, 100 k, wired as a variable resistor to ground.
pub const MIDDLE: usize = 3;
/// VR10, the Master Volume, 1 M, wired as a variable resistor to ground. The
/// panel's Master knob turns it.
pub const MASTER: usize = 4;
/// VR5, the boost channel's own Volume, 1 M log. No panel knob; it rests at
/// [`VOLUME_REST`].
pub const VOLUME: usize = 5;
/// VR9, Master Reverb, 100 k. The tank is not modelled, so it rests at zero.
pub const REVERB: usize = 6;
/// The muted Normal channel's Volume, Treble and Bass (VR1, VR2, VR3). They
/// matter only as the load their network is on V1A, and rest where
/// [`NORMAL_VOLUME_REST`] and the middle put them.
pub const NORMAL_VOLUME: usize = 7;
pub const NORMAL_TREBLE: usize = 8;
pub const NORMAL_BASS: usize = 9;

/// Where the Master rests, which is where the voice is calibrated. The same
/// half-way as the other amplifiers whose master is in the preamplifier.
pub const MASTER_REST: f64 = 0.5;
/// The boost Volume, at the 6 of 10 Morello is reported to use. See the
/// research log's settings table.
pub const VOLUME_REST: f64 = 0.6;
/// The Normal channel's Volume, at zero, as it is on the amplifier this model
/// was voiced after.
pub const NORMAL_VOLUME_REST: f64 = 0.0;

/// The inverter's rail, X on the drawing, which the preamplifier's own rail A
/// hangs from through R58. ESTIMATED: the 2205 sheets carry no voltages. X is
/// the screen node less R59 10 k's drop, and that drop is the inverter's and
/// the preamplifier's current together -- about 2.5 mA and 6 mA -- so from the
/// 450 V screens measured on the 50 W JCM800s (see `power::BRIT_2205_EL34`)
/// it settles near 365 V. The preamplifier then sets A from it through R58.
pub const RAIL: f64 = 365.0;

/// A 1N4007 rectifier diode, from the widely distributed SPICE model (IS
/// 7.03 nA, N 1.808). Its 34 milliohm series resistance and its junction
/// capacitance are left out, as for every diode in the catalogue.
pub const R1N4007: DiodeSpec = DiodeSpec {
    saturation: 7.027_67e-9,
    emission: 1.808_03,
};

/// Three 1N4007-class junctions in series: DB1's two conducting cells and D6.
///
/// For identical Shockley junctions carrying one current, three in series drop
/// three times what one does at every current, which is one junction with three
/// times the emission coefficient. So this is exact for the bridge as drawn,
/// given that its cells are the same junction as D6 -- which is the
/// approximation, and `tests/brit2205.rs` builds the full bridge beside it.
pub const CLIPPER: DiodeSpec = DiodeSpec {
    saturation: R1N4007.saturation,
    emission: 3.0 * R1N4007.emission,
};

const ECC83: TriodeSpec = TriodeSpec::ECC83;

/// The idle current of the valves on rail A that are not built -- the muted
/// Normal channel's V2 pair and the reverb recovery V4B, about 2 mA together
/// -- as the resistance that draws it, so the rail sits where they would pull
/// it. ESTIMATED from their cathode resistors.
const UNBUILT_IDLE: f64 = 120_000.0;

/// Blocks V4A's plate voltage from the hand-off. Not a part on the drawing:
/// the drawing's coupling capacitor, C25, is the power stage's
/// `pi_couple`, and this is large enough to be invisible in front of it.
const HANDOFF_BLOCK: f64 = 10e-6;

/// The boost channel from the input jack to V4A's plate.
pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

/// The same preamplifier brought out at a chosen node, for measuring one stage
/// at a time. Reference designators are the 1988 drawing's.
pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit 2205 preamp");
    build_into(&mut net, source, load, Clipper::Folded);
    net.build(at)
}

/// How the bridge clipper is built. `Folded` is what ships; `Bridge` is the
/// drawing part for part, kept so a test can hold the two together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clipper {
    Folded,
    Bridge,
}

/// The same preamplifier with the clipper built as a chosen [`Clipper`].
pub fn tap_with(source: f64, load: f64, at: &str, clipper: Clipper) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Brit 2205 preamp");
    build_into(&mut net, source, load, clipper);
    net.build(at)
}

fn build_into(net: &mut Netlist, source: f64, load: f64, clipper: Clipper) {
    // --- the supply ------------------------------------------------------------
    // A hangs off X through R58 10 k 1 W with C44 50 uF; every preamp valve's
    // plate load comes off A.
    net.supply("a", 10_000.0, RAIL) // R58
        .capacitor("a", "gnd", 50e-6) // C44
        .resistor("a", "gnd", UNBUILT_IDLE);

    // --- V1A, shared by both channels -------------------------------------------
    // C1 .047 in, R1 1 M from the grid. R2 2k7 with C2 .47 uF: bypassed from
    // about 125 Hz up.
    net.input("in", source)
        .capacitor("in", "v1a_g", 0.047e-6) // C1
        .resistor("v1a_g", "gnd", 1_000_000.0) // R1
        .resistor("a", "v1a_p", 100_000.0) // R4
        .resistor("v1a_k", "gnd", 2_700.0) // R2
        .capacitor("v1a_k", "gnd", 0.47e-6) // C2
        .triode("v1a_p", "v1a_g", "v1a_k", ECC83);

    // --- the muted Normal channel, as the load it is ----------------------------
    // C6 .022 to the top of VR3 (Bass), whose other end returns through R26
    // 100 k; C3 .005 to VR3's wiper; R5 100 k from there to VR2's (Treble)
    // wiper, which R6 330 k joins to the top of VR1 (Volume); C5 .01 from VR1's
    // top to VR2's, and C23 .002 under VR2. C4 100 pF across the top of VR1 to
    // V2's grid, and C40 .22 from that grid to the switching transistor, which
    // conducts with the boost channel selected: the Normal channel's second
    // stage is shorted out there, and is not built.
    net.rest(NORMAL_BASS, 0.5)
        .rest(NORMAL_TREBLE, 0.5)
        .rest(NORMAL_VOLUME, NORMAL_VOLUME_REST)
        .capacitor("v1a_p", "n_b_top", 0.022e-6) // C6
        .pot(
            "n_b_top",
            "n_mid",
            "n_b_bot",
            1_000_000.0,
            Taper::Audio,
            NORMAL_BASS,
        ) // VR3
        .resistor("n_b_bot", "gnd", 100_000.0) // R26
        .capacitor("v1a_p", "n_mid", 0.005e-6) // C3
        .resistor("n_mid", "n_tw", 100_000.0) // R5
        .resistor("n_tw", "n_z", 330_000.0) // R6
        .capacitor("n_z", "n_t_top", 0.01e-6) // C5
        .pot(
            "n_t_top",
            "n_tw",
            "n_t_bot",
            1_000_000.0,
            Taper::Audio,
            NORMAL_TREBLE,
        ) // VR2
        .capacitor("n_t_bot", "gnd", 0.002e-6) // C23
        .pot(
            "n_z",
            "v2_g",
            "gnd",
            1_000_000.0,
            Taper::Audio,
            NORMAL_VOLUME,
        ) // VR1
        .capacitor("n_z", "v2_g", 100e-12) // C4
        .capacitor("v2_g", "gnd", 0.22e-6); // C40, through the conducting mute

    // --- into V1B ---------------------------------------------------------------------
    // C7 2 nF and R11 100 k into the top of VR4A: the boost channel takes V1A's
    // signal through a small capacitor, which is where it loses its bottom end
    // before any gain -- 72 Hz against the pot and R11. VR4A's wiper through
    // C39 .047 to V1B's grid, R12 220 k to ground. C11 .22 from that grid goes
    // to the switching bus, which is open with the boost channel on.
    net.capacitor("v1a_p", "r11", 0.002e-6) // C7
        .resistor("r11", "g1_top", 100_000.0) // R11
        .pot("g1_top", "g1_w", "gnd", 1_000_000.0, Taper::Audio, GAIN) // VR4A
        .capacitor("g1_w", "v1b_g", 0.047e-6) // C39
        .resistor("v1b_g", "gnd", 220_000.0); // R12

    // --- V1B, diode-biased ---------------------------------------------------------
    // R7 470 k plate load. R3 10 k to ground with D1 across it, anode at the
    // cathode.
    net.resistor("a", "v1b_p", 470_000.0) // R7
        .resistor("v1b_k", "gnd", 10_000.0) // R3
        .diode("v1b_k", "gnd", R1N4007) // D1
        .triode("v1b_p", "v1b_g", "v1b_k", ECC83);

    // --- VR4B and the clipper ---------------------------------------------------------
    // C8 .022 into the top of VR4B, the second gang of the Gain knob. Its wiper
    // through R20 27 k, with C16 2 nF across it, to X; DB1 and D6 from X to
    // ground; VR5, the boost Volume, from X to ground.
    net.capacitor("v1b_p", "g2_top", 0.022e-6) // C8
        .pot("g2_top", "g2_w", "gnd", 1_000_000.0, Taper::Audio, GAIN) // VR4B
        .resistor("g2_w", "x", 27_000.0) // R20
        .capacitor("g2_w", "x", 0.002e-6); // C16
    match clipper {
        Clipper::Folded => {
            net.diode("x", "gnd", CLIPPER).diode("gnd", "x", CLIPPER);
        }
        Clipper::Bridge => {
            // DB1's four cells, AC terminals at X and ground, with D6 from its
            // positive terminal to its negative.
            net.diode("x", "db_pos", R1N4007)
                .diode("gnd", "db_pos", R1N4007)
                .diode("db_neg", "x", R1N4007)
                .diode("db_neg", "gnd", R1N4007)
                .diode("db_pos", "db_neg", R1N4007); // D6
        }
    }

    // --- the boost Volume and V3A -----------------------------------------------------
    // VR5's wiper is V3A's grid; C48 47 pF from it to the cathode, which C18
    // 22 uF bypasses, so it is a treble shunt at the grid. C12 .22 from the
    // wiper goes to the switching bus, open with the boost channel on. V3A:
    // R27 100 k plate load with C22 2 nF from the rail to the plate -- a
    // treble shunt at the plate, since the rail is decoupled -- and R24 1k5 with
    // C18 at the cathode.
    net.rest(VOLUME, VOLUME_REST)
        .pot("x", "v3a_g", "gnd", 1_000_000.0, Taper::Audio, VOLUME) // VR5
        .capacitor("v3a_g", "v3a_k", 47e-12) // C48
        .resistor("a", "v3a_p", 100_000.0) // R27
        .capacitor("a", "v3a_p", 0.002e-6) // C22
        .resistor("v3a_k", "gnd", 1_500.0) // R24
        .capacitor("v3a_k", "gnd", 22e-6) // C18
        .triode("v3a_p", "v3a_g", "v3a_k", ECC83);

    // --- the tone stack -------------------------------------------------------------
    // Driven from V3A's plate through R28 10 k -- no cathode follower here,
    // unlike the 2203. C17 470 pF to the top of Treble; R23 56 k to the slope
    // node S; C19 .022 from S to the bottom of Treble, T; C21 22 nF from S to
    // Middle, a rheostat to ground; R33 4k7 from T to Bass, a rheostat to
    // ground; C20 2 nF from T to ground. The Treble wiper is the output.
    //
    // Both rheostats put more resistance in as they turn up, which lifts their
    // band, so they take the reverse law (see `Taper::ReverseAudio`).
    net.resistor("v3a_p", "b", 10_000.0) // R28
        .capacitor("b", "t_top", 470e-12) // C17
        .resistor("b", "s", 56_000.0) // R23
        .pot("t_top", "t_w", "t_bot", 220_000.0, Taper::Linear, TREBLE) // VR6
        .capacitor("s", "t_bot", 0.022e-6) // C19
        .capacitor("s", "m_top", 22e-9) // C21
        .pot(
            "m_top",
            "gnd",
            "gnd",
            100_000.0,
            Taper::ReverseAudio,
            MIDDLE,
        ) // VR7
        .resistor("t_bot", "b_top", 4_700.0) // R33
        .pot(
            "b_top",
            "gnd",
            "gnd",
            1_000_000.0,
            Taper::ReverseAudio,
            BASS,
        ) // VR8
        .capacitor("t_bot", "gnd", 0.002e-6); // C20

    // --- the master, and what else hangs on it ---------------------------------------
    // R22 100 k from the Treble wiper to G; VR10, the Master, a rheostat from G
    // to ground. The muted Normal channel's output arrives at G through R21
    // 470 k from R19 100 k || C15 470 pF and R18 150 k, which C14 .022 couples
    // to V2's plate load R17 100 k (the valve is not built). The reverb send
    // leaves through R35 220 k and C27 .01 to V3B's grid, R36 1 M (V3B not
    // built).
    net.rest(MASTER, MASTER_REST)
        .resistor("t_w", "g", 100_000.0) // R22
        .pot("g", "gnd", "gnd", 1_000_000.0, Taper::ReverseAudio, MASTER) // VR10
        .resistor("g", "n_out", 470_000.0) // R21
        .resistor("n_out", "gnd", 100_000.0) // R19
        .capacitor("n_out", "gnd", 470e-12) // C15
        .resistor("n_out", "n_r18", 150_000.0) // R18
        .capacitor("n_r18", "v2b_p", 0.022e-6) // C14
        .resistor("a", "v2b_p", 100_000.0) // R17
        .resistor("g", "rv_send", 220_000.0) // R35
        .capacitor("rv_send", "v3b_g", 0.01e-6) // C27
        .resistor("v3b_g", "gnd", 1_000_000.0); // R36

    // --- the mix, and V4A ----------------------------------------------------------------
    // R32 470 k with C24 100 pF across it from G to M; R29 10 k from M to
    // ground, which is the loop's send; R31 56 k from M to the top of VR9, the
    // reverb pot, whose wiper the (unbuilt) recovery stage drives -- at zero it
    // sits on ground. M through the return jack's closed contact and C26 .22 to
    // V4A's grid, R34 1 M. V4A: R30 100 k plate load, R25 1k5 unbypassed.
    net.rest(REVERB, 0.0)
        .resistor("g", "m", 470_000.0) // R32
        .capacitor("g", "m", 100e-12) // C24
        .resistor("m", "gnd", 10_000.0) // R29
        .resistor("m", "rv_top", 56_000.0) // R31
        .pot("rv_top", "gnd", "gnd", 100_000.0, Taper::Audio, REVERB) // VR9
        .capacitor("m", "v4a_g", 0.22e-6) // C26
        .resistor("v4a_g", "gnd", 1_000_000.0) // R34
        .resistor("a", "v4a_p", 100_000.0) // R30
        .resistor("v4a_k", "gnd", 1_500.0) // R25
        .triode("v4a_p", "v4a_g", "v4a_k", ECC83)
        .capacitor("v4a_p", "out", HANDOFF_BLOCK)
        .resistor("out", "gnd", load);
}
