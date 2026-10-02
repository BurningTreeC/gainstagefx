//! The American SVT: the 300 W valve bass head's channel 1, from its NORMAL
//! input to the 6C4 that feeds the power amplifier.
//!
//! Engineering reference (developer documentation, not panel text): Ampeg
//! SVT, "SVT PREAMP" D 591719 revision D (ECN-P9517, 12/5/75). The matched
//! power stage is `power::PowerSpec::SVT_6550`. See
//! `docs/models/american_svt.md`.
//!
//! ```text
//! NORMAL -- R2 47k -- V1a (R3 220k / R5 3k3) -- V1b follower (R6 220k)
//!   -- C2 .1 -- BASS SELECT (at OFF) -- VR1 VOLUME, C6 ULTRA HI across it
//!   -- V3a (R24 47k / R25 4k7) -- C7 .1 -- James stack (BASS, TREBLE)
//!   -- C14 .01 -- V3b (R28 220k; R29 560 + R30 7k5) -- C15 .01
//!   -- V4a (R32 470k / R33 3k3) -- V4b follower (R37 47k + R36 6k8)
//!        |__ R35 56k back to V3b's cathode ______________|
//!   MIDRANGE between V3b's cathode and the R37/R36 tap, L1 to ground
//!   -- C20 .1 -- R42 100k -- mix (channel 2's R23 100k) -- C21 .02 -- V5 6C4
//! ```
//!
//! **The midrange is inside a feedback loop.** V3b, V4a and V4b are two
//! inversions and a follower, and R35 takes V4b's cathode back to V3b's: the
//! three are one amplifier with its gain set by the loop. VR7 sits between two
//! points of that loop -- V3b's cathode through C16 and 470 ohm, and the
//! follower's tap through C19 and 620 ohm -- with its wiper on a series
//! resonance to ground, the toroid and a capacitor. Turned one way the
//! resonance shorts the loop's return at V3b's cathode and the gain rises at
//! that frequency; turned the other it shunts the follower's output and the
//! band falls. Hence +/-20 dB with no midrange pot in the signal path.
//!
//! **The Volume is in the middle.** Two triodes ahead of it, five behind it,
//! and the stack between V3a and V3b: turned up it drives V3a, the stack's
//! output and the loop harder, which is where an SVT's grind comes from before
//! the power amplifier adds its own.

use crate::dsp::netlist::{Adjust, Circuit, Fault, Netlist, Taper, TriodeSpec};

/// VR1, 1 M linear, channel 1's VOLUME. The Drive knob turns it.
pub const VOLUME: usize = 0;
/// VR5, 1 M log, BASS.
pub const BASS: usize = 1;
/// VR7, 50 k linear, MIDRANGE.
pub const MIDDLE: usize = 2;
/// VR6, 1 M log, TREBLE.
pub const TREBLE: usize = 3;

/// SW2 ULTRA HI: C6, 500 pF from VR1's top to its wiper, as an adjustable
/// capacitor -- in at 500 pF, out at nothing. Ampeg: "the amount of boost is
/// dependent on the setting of the volume control", which a bright capacitor
/// across a volume pot is.
pub const ULTRA_HI_SLOT: usize = 0;
pub const ULTRA_HI_FARADS: f64 = 500e-12;
pub const ULTRA_HI_OFF_FARADS: f64 = 1.0e-18;

/// The two slide switches, as the contacts they close: each contact is an
/// adjustable resistor, closed at `SWITCH_CLOSED` and open at `SWITCH_OPEN`.
/// One ohm against the network's hundreds of kilohms, and a gigohm against a
/// nanofarad's megohms at 100 Hz, so neither state is anything but a switch.
pub const SWITCH_CLOSED: f64 = 1.0;
pub const SWITCH_OPEN: f64 = 1.0e9;
const C: f64 = SWITCH_CLOSED;
const O: f64 = SWITCH_OPEN;

/// SW1 BASS SELECT, a two-row slide switch whose slider bridges neighbouring
/// contacts in each row: four contacts that matter, `A-B` and `B-N` in the
/// OFF position, `B-T` and `N-P` in ULTRA LO, none at all in BASS CUT (the
/// slider's left-hand pair carries nothing). Where A is C2's far side, B is
/// VR1's top, N the C4/C5 junction, P the R10/R11 junction and T R9's far end.
/// Left, centre, right on the panel: Ampeg's "engaging the left side of this
/// switch decreases the low frequency output ... the right side enhances" it.
pub const BASS_SELECT_SLOTS: [usize; 4] = [1, 2, 3, 4];
pub const BASS_SELECT: [[f64; 4]; 3] = [
    [O, O, O, O], // BASS CUT: through C4 and C5 alone
    [C, C, O, O], // OFF: straight through
    [O, O, C, C], // ULTRA LO: the twin-T
];

/// SW5 MIDRANGE SELECT: which section of the toroid VR7's wiper reaches,
/// with which capacitor. Ampeg's SVT-VR manual: 220 Hz with the left side
/// engaged, 800 Hz in the centre, 3 kHz with the right.
pub const MID_SELECT_SLOTS: [usize; 3] = [5, 6, 7];
pub const MID_SELECT: [[f64; 3]; 3] = [
    [C, O, O], // 1: tap 1, the whole toroid straight to ground
    [O, C, O], // 2: tap 2 through C18 .15
    [O, O, C], // 3: tap 3 through C17 .033
];

/// L1's three sections, which the drawing does not give (toroid 320821-1).
/// ESTIMATED, each from its published frequency and the capacitance in series
/// with it -- C16's .68 alone, then with C18's .15, then with C17's .033 --
/// so tap 1 resonates at 220 Hz, tap 2 at 800 Hz, tap 3 at 3 kHz. As turns
/// they are 100, 65 and 34 % of the winding: a plausibly tapped toroid.
pub const L1_TAP1: f64 = 0.7697;
pub const L1_TAP2: f64 = 0.3221;
pub const L1_TAP3: f64 = 0.0894;

/// The preamplifier's rail: P1 pin 5, 430 V on the drawing, through R47 to
/// the "+300 V" node and C23.
///
/// R47 reads "82K 5W". The sheet's own voltages ask for 16 mA through it,
/// which is 8.2 k, and so does its 5 W rating; the 1969 SVT drawing, 669-0580,
/// prints it 8.2 K. PLAUSIBLE.
pub const RAIL_UPSTREAM: f64 = 430.0;
pub const RAIL_DROPPER: f64 = 8_200.0;
pub const RAIL_RESERVOIR: f64 = 40e-6;

/// Channel 2's two triodes, which are not built, as the 1.3 mA they draw from
/// the rail: V2a 1.75 V across 3.3 k and V2b 2.2 V across 3.3 k on the sheet.
/// APPROXIMATED.
const CHANNEL_2_DRAW: f64 = 300.0 / 1.3e-3;

/// V2b's plate as channel 2's stack sees it: R20's 100 k against a 12AX7's
/// plate resistance near 65 k at the 0.7 mA the sheet gives it. APPROXIMATED.
const CHANNEL_2_SOURCE: f64 = 39_000.0;

/// Where channel 2's knobs are, since it is not played: noon. ESTIMATED.
const CHANNEL_2_KNOBS: f64 = 0.5;

const ECC83: TriodeSpec = TriodeSpec::ECC83;

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("American SVT");

    net.supply("bp", RAIL_DROPPER, RAIL_UPSTREAM)
        .capacitor("bp", "gnd", RAIL_RESERVOIR)
        .resistor("bp", "gnd", CHANNEL_2_DRAW);

    // --- V1: a gain stage and a follower ---------------------------------------
    net.input("in", source)
        .resistor("in", "g1", 47_000.0) // R2
        .resistor("g1", "gnd", 5_600_000.0) // R4
        .resistor("bp", "p1", 220_000.0) // R3
        .triode("p1", "g1", "k1", ECC83)
        .resistor("k1", "gnd", 3_300.0) // R5
        .triode("bp", "p1", "k1b", ECC83)
        .resistor("k1b", "gnd", 220_000.0); // R6

    // --- BASS SELECT, and the Volume -----------------------------------------------
    // Every part of the network, and the four switch contacts between its
    // nodes (see `BASS_SELECT`). At OFF, A, B and N are one node; at ULTRA LO
    // R8-R9 with C3 and C4-C5 with R11 are a twin-T; at BASS CUT only C4 and
    // C5 carry the signal.
    net.capacitor("k1b", "bsa", 0.1e-6) // C2
        .resistor("bsa", "r8b", 150_000.0) // R8
        .capacitor("r8b", "gnd", 0.002e-6) // C3
        .resistor("r8b", "r9b", 150_000.0) // R9
        .capacitor("bsa", "bsn", 0.002e-6) // C4
        .capacitor("bsn", "vol", 0.002e-6) // C5
        .resistor("bsn", "bsp", 820_000.0) // R10
        .resistor("bsp", "gnd", 68_000.0) // R11
        .pot("vol", "g3a", "gnd", 1_000_000.0, Taper::Linear, VOLUME); // VR1
    let ultra_hi = net.adjustable("vol", "g3a", Adjust::Capacitor, ULTRA_HI_FARADS);
    debug_assert_eq!(ultra_hi, ULTRA_HI_SLOT);
    let off = BASS_SELECT[1];
    for ((a, b), (slot, value)) in [
        ("bsa", "vol"),
        ("vol", "bsn"),
        ("vol", "r9b"),
        ("bsn", "bsp"),
    ]
    .into_iter()
    .zip(BASS_SELECT_SLOTS.into_iter().zip(off))
    {
        let made = net.adjustable(a, b, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }

    // --- V3a ----------------------------------------------------------------------
    // R25 is 4.7 k here; the 1969 drawing's 1.5 k and its "200 V / 3.2 V"
    // belong to the 12DW7 that position held then, and the voltages were
    // carried over when the valve became a 12AX7.
    net.resistor("bp", "p3a", 47_000.0) // R24
        .triode("p3a", "g3a", "k3a", ECC83)
        .resistor("k3a", "gnd", 4_700.0) // R25
        .capacitor("p3a", "s1", 0.1e-6); // C7

    // --- the James stack (P.E.C. 250762-1) --------------------------------------
    james_stack(&mut net, "s1", "x1", Some((BASS, TREBLE)), "1");

    // --- V3b, V4 and the loop -----------------------------------------------------
    net.capacitor("x1", "g3b", 0.01e-6) // C14
        .resistor("g3b", "k3j", 1_000_000.0) // R27
        .resistor("bp", "p3b", 220_000.0) // R28
        .triode("p3b", "g3b", "k3b", ECC83)
        .resistor("k3b", "k3j", 560.0) // R29
        .resistor("k3j", "gnd", 7_500.0) // R30
        .capacitor("p3b", "g4a", 0.01e-6) // C15
        .resistor("g4a", "gnd", 1_000_000.0) // R31
        .resistor("bp", "p4a", 470_000.0) // R32
        .triode("p4a", "g4a", "k4a", ECC83)
        .resistor("k4a", "gnd", 3_300.0) // R33
        .triode("bp", "p4a", "k4b", ECC83)
        .resistor("k4b", "cf", 47_000.0) // R37
        .resistor("cf", "gnd", 6_800.0) // R36
        .resistor("k4b", "k3b", 56_000.0); // R35

    // --- MIDRANGE -----------------------------------------------------------------
    // `pot(a, wiper, b)` puts `R f(p)` between the wiper and `b`, so turned up
    // the wiper is at `a`: V3b's cathode end, where the resonance shorts the
    // loop's return and the band rises. The sheet's arrow puts clockwise there
    // too, and Ampeg's manual has clockwise "a sound which really cuts
    // through".
    net.capacitor("k3b", "c16b", 0.68e-6) // C16
        .resistor("c16b", "mid_l", 470.0) // R34
        .capacitor("cf", "c19b", 0.68e-6) // C19
        .resistor("c19b", "mid_r", 620.0) // R39
        .resistor("mid_r", "gnd", 220_000.0) // R38
        .pot("mid_l", "mid_w", "mid_r", 50_000.0, Taper::Linear, MIDDLE); // VR7
                                                                          // The three branches SW5 chooses between, each behind its own contact. The
                                                                          // sections are built as three inductors rather than one tapped winding:
                                                                          // only the section in circuit carries current, the rest of the toroid
                                                                          // hangs off it through R40 / R41's 220 k and an open contact. APPROXIMATED.
    net.inductor("ml", "gnd", L1_TAP1)
        .inductor("mc", "l1_2", L1_TAP2)
        .capacitor("l1_2", "gnd", 0.15e-6) // C18
        .resistor("l1_2", "gnd", 220_000.0) // R41
        .inductor("mh", "l1_3", L1_TAP3)
        .capacitor("l1_3", "gnd", 0.033e-6) // C17
        .resistor("l1_3", "gnd", 220_000.0); // R40
    let centre = MID_SELECT[1];
    for (to, (slot, value)) in ["ml", "mc", "mh"]
        .into_iter()
        .zip(MID_SELECT_SLOTS.into_iter().zip(centre))
    {
        let made = net.adjustable("mid_w", to, Adjust::Resistor, value);
        debug_assert_eq!(made, slot);
    }

    // --- the mix, with channel 2 not played -------------------------------------
    net.capacitor("cf", "c20b", 0.1e-6) // C20
        .resistor("c20b", "mix", 100_000.0) // R42
        .resistor("ch2_p", "gnd", CHANNEL_2_SOURCE)
        .capacitor("ch2_p", "s2", 0.1e-6); // C13
    james_stack(&mut net, "s2", "x2", None, "2");
    net.resistor("x2", "mix", 100_000.0); // R23

    // --- V5, a bootstrapped 6C4 follower --------------------------------------------
    net.capacitor("mix", "g5", 0.02e-6) // C21
        .resistor("g5", "k5j", 1_000_000.0) // R43
        .triode("bp", "g5", "k5", TriodeSpec::ECC82)
        .resistor("k5", "k5j", 1_000.0) // R44
        .resistor("k5j", "gnd", 47_000.0) // R45
        .capacitor("k5", "out", 0.1e-6) // C22
        .resistor("out", "gnd", 100_000.0) // R46
        .resistor("out", "gnd", load);

    net.build(at)
}

/// The P.E.C. 250762-1, both channels' stack: 220 k / 1 M log BASS / 22 k,
/// .001 across the bass pot's upper half and .01 across its lower, 120 k from
/// its wiper to the TREBLE's; .00047 / 1 M log TREBLE / .0047; out from the
/// treble's wiper. `controls` is the two pots' control numbers, or `None` for
/// fixed parts at `CHANNEL_2_KNOBS`. `pot(a, wiper, b)` puts `R f(p)` between
/// the wiper and `b`, so turned up each wiper is at `a`, its input end, which
/// is where the sheet's arrows put clockwise; the log track's short end is
/// `b`, the grounded end. The VT-40's P.E.C. 6470000 is the same network
/// (`circuits::american_vt40`).
pub(crate) fn james_stack(
    net: &mut Netlist,
    input: &str,
    output: &str,
    controls: Option<(usize, usize)>,
    tag: &str,
) {
    let n = |s: &str| format!("{s}{tag}");
    let (bt, bw, bb, tt, tb) = (n("bt"), n("bw"), n("bb"), n("tt"), n("tb"));
    net.resistor(input, &bt, 220_000.0)
        .capacitor(&bt, &bw, 0.001e-6)
        .capacitor(&bw, &bb, 0.01e-6)
        .resistor(&bb, "gnd", 22_000.0)
        .resistor(&bw, output, 120_000.0) // R26 / R22
        .capacitor(input, &tt, 0.00047e-6)
        .capacitor(&tb, "gnd", 0.0047e-6);
    match controls {
        Some((bass, treble)) => {
            net.pot(&bt, &bw, &bb, 1_000_000.0, Taper::Audio, bass).pot(
                &tt,
                output,
                &tb,
                1_000_000.0,
                Taper::Audio,
                treble,
            );
        }
        None => {
            let below = Taper::Audio.fraction(CHANNEL_2_KNOBS) * 1_000_000.0;
            net.resistor(&bt, &bw, 1_000_000.0 - below)
                .resistor(&bw, &bb, below)
                .resistor(&tt, output, 1_000_000.0 - below)
                .resistor(output, &tb, below);
        }
    }
}
