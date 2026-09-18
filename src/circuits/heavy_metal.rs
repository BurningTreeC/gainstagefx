//! The Heavy Metal: two transistor stages, a hard clipper, a coring gate and
//! three gyrators.
//!
//! Engineering reference (developer documentation, not panel text): Boss HM-2
//! Heavy Metal, the Japanese original, built from Boss's own service-notes
//! drawing. See `docs/models/heavy_metal.md`.
//!
//! Three things here exist nowhere else in the plugin.
//!
//! The first is the **coring gate**: D6 and D7 sit *in series with the signal*
//! rather than across it, back to back, so nothing gets through until the
//! signal is bigger than a germanium diode's forward voltage. Small signals are
//! held back entirely and the pedal stops rather than fades. That is the gated,
//! chainsaw decay it is known for, and it is a primitive noise gate made of two
//! diodes.
//!
//! The second is the **gyrator stack**. A gyrator is an active circuit that
//! behaves like an inductor: an op-amp (here a transistor stage) with a
//! capacitor arranged so the input current lags, which gives a resonant
//! peak without winding anything. Three of them, at roughly 80 Hz, 900 Hz and
//! 2 kHz, are boosted or cut by the two Colour Mix controls -- so this is a
//! *boost and cut* tone section rather than the passive losses every other
//! pedal here has.
//!
//! The third is that the gain stage clips **asymmetrically**: D3 alone one way
//! against the D4/D5 pair the other, so the two halves of the wave are squared
//! off at different heights and the result is full of even harmonics.

use crate::dsp::netlist::{BipolarSpec, Circuit, DiodeSpec, Fault, JfetSpec, Netlist, Taper};

/// VR4, the Dist control, 250 kD on Boss's drawing. Its wiper is grounded:
/// one half shunts Q6 harder at low settings while the other half sets IC1B's
/// feedback gain, so one knob changes both pre-drive and clipper gain.
pub const DIST: usize = 0;
/// VR2, Colour Mix Low, 10 k.
pub const LOW: usize = 1;
/// VR3, Colour Mix High, 10 k.
pub const HIGH: usize = 2;
/// VR1, the Level control, 10 k audio.
pub const LEVEL: usize = 3;

/// Where the level control rests: **unity through the pedal** with DIST and
/// both Colour Mix controls centred. This is what the plugin's Level knob means
/// at noon; the ends still reach the ends of the real 10 k audio pot.
///
/// Re-measured after the 2026-09-17 HM-2 topology and Colour-Mix rail fixes
/// using the broadband low-chord probe in `examples/hm2level.rs`. Physical
/// level 0.51 measured -0.61 dB and 0.60 measured +6.88 dB. Because the upper
/// half of our audio taper is linear in divider fraction, those two readings
/// put unity at about 0.5148; 0.515 is the calibrated rest. Keeping the old
/// 0.60 rest over-drove following pedals by roughly seven decibels at noon.
pub const LEVEL_REST: f64 = 0.515;

/// The germanium pair of the coring gate, D6 and D7.
///
/// ESTIMATED to the **0.3 V** forward drop the published analysis measures on
/// these two parts, rather than the catalogue's generic germanium, which is
/// softer than that. Where the gate opens *is* the sound of this pedal, so it
/// carries its own constants, exactly as the Yellow Dist's 1N270s do.
const CORING: DiodeSpec = DiodeSpec {
    saturation: 3.3e-9,
    emission: 1.2,
};

/// The JFET input buffer, Q1. APPROXIMATED: the scan carries no part numbers,
/// and the Boss standard of the period is a 2SK30A-class small-signal JFET.
const BUFFER: JfetSpec = JfetSpec::J201;

/// Q6 is a 2SC2240-GR NPN and Q7 is a 2SA970-GR PNP on Boss's parts list.
/// The catalogue has no dedicated models for those complementary low-noise
/// Japanese parts yet, so use the same small-signal magnitude and mirror Q7
/// with `bipolar_pnp()`. The polarity is not optional: modelling Q7 as NPN
/// puts the whole high-gain block at the wrong operating point.
const SMALL_SIGNAL_BJT: BipolarSpec = BipolarSpec::NPN_2SC3378;

/// The op-amp rail, either side of the 4.5 V bias.
const SWING: f64 = 3.6;

/// The three gyrators, as the inductance each one stands for.
///
/// A gyrator is an op-amp wired so that the current into it lags the voltage:
/// it *is* an inductor, made without winding one, which is how a pedal gets a
/// resonant tone control into a box this size. The drawing has three, all the
/// same shape -- a resistor from the node to the follower's input, a capacitor
/// from there to ground, and the follower's output back to the node through a
/// second resistor -- and what it simulates is **L = R1 x R2 x C**.
///
/// So they are built here as the inductors they are, which is what the Mark
/// IIC+'s graphic equaliser already does with its five real ones
/// (`markiic::BANDS`). What that leaves out is the op-amp's own limits: a real
/// gyrator can run out of swing and an inductor cannot. At the levels this
/// stack sees, after the clipper and the Dist control, it does not.
///
/// Each entry: the equivalent inductance, its series resistance, and the
/// capacitor in front of it, which together decide where the band sits.
///
/// | | from the drawing | L | with | resonance |
/// |---|---|---|---|---|
/// | low | R51 100k, R47 330, C30 .068 | 2.244 H | C35 1.5 uF | **87 Hz** |
/// | mid | R43 82k, R41 330, C27 .0068 | 0.184 H | C28 .15 uF | **958 Hz** |
/// | high | R44 100k, R45 330, C26 .0047 | 0.155 H | C29 .1 uF | **1278 Hz** |
///
/// Which is what the published analysis hears: the Low control works "at around
/// 80Hz", and the High one "between about 900Hz and 1.3kHz" -- that is these
/// two upper bands, both on the one knob.
const LOW_BAND: (f64, f64, f64) = (2.244, 330.0, 1.5e-6);
const MID_BAND: (f64, f64, f64) = (0.184, 330.0, 0.15e-6);
const HIGH_BAND: (f64, f64, f64) = (0.1551, 330.0, 0.1e-6);

/// One band of the Colour Mix: the series capacitor, the gyrator's inductance
/// and its resistance, hung off a wiper.
fn band(net: &mut Netlist, wiper: &str, prefix: &str, (l, r, c): (f64, f64, f64)) {
    let a = format!("{prefix}_a");
    let b = format!("{prefix}_b");
    net.capacitor(wiper, &a, c)
        .inductor(&a, &b, l)
        .resistor(&b, "gnd", r);
}

pub fn build(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap(source, load, "out")
}

#[cfg(test)]
pub fn build_full_newton_reference(source: f64, load: f64) -> Result<Circuit, Fault> {
    tap_impl(source, load, "out", false)
}

pub fn tap(source: f64, load: f64, at: &str) -> Result<Circuit, Fault> {
    tap_impl(source, load, at, true)
}

fn tap_impl(source: f64, load: f64, at: &str, partition_filters: bool) -> Result<Circuit, Fault> {
    let mut net = Netlist::new("Heavy Metal");

    // --- supply and bias ------------------------------------------------------------
    // R5 and R6 10 k each with C25 make the 4.5 V the whole pedal sits on.
    net.supply("v9", 47.0, 9.0)
        .capacitor("v9", "gnd", 100e-6) // C2
        .resistor("v9", "vref", 10_000.0) // R6
        .resistor("vref", "gnd", 10_000.0) // R5
        .capacitor("vref", "gnd", 47e-6) // C25
        // R10 and C5 decouple the two gain stages' rail from everything else.
        // Left out at first, and the pedal then had a floor forty decibels down
        // that did not depend on the signal at all: the gain stages were pulling
        // the supply about and the output followers were passing it on.
        .resistor("v9", "v9a", 1_000.0) // R10
        .capacitor("v9a", "gnd", 47e-6); // C5

    // --- input buffer, Q1 -------------------------------------------------------------
    // R1 10 k and C1 .047 into the gate, R8 1 M holding it at the bias point.
    net.input("in", source)
        .resistor("in", "c1", 10_000.0) // R1
        .capacitor("c1", "g1", 0.047e-6) // C1
        .resistor("g1", "vref", 1_000_000.0) // R8
        .jfet("v9", "g1", "s1", BUFFER)
        .resistor("s1", "gnd", 10_000.0); // R7, the source resistor

    // --- distortion pre-gain, Q6 and Q7 -------------------------------------------
    // Boss's original circuit is a complementary two-transistor block. Q6 is
    // 2SC2240-GR (NPN): 47 nF/22 k feed the base, 100 k biases it to ground,
    // 10 k is the collector load, 22 ohm the emitter resistor, and 470 k/100 pF
    // feed collector voltage back to the base. The previous model had
    // an extra bipolar follower here and a 22 k collector load; neither is on
    // the HM-2 audio path.
    net.capacitor("s1", "q6_in", 0.047e-6) // C6
        .resistor("q6_in", "b6", 22_000.0) // R22
        .resistor("b6", "gnd", 100_000.0) // R17
        .bipolar("c6", "b6", "e6", SMALL_SIGNAL_BJT) // Q6, 2SC2240-GR
        .resistor("v9a", "c6", 10_000.0) // R19
        .resistor("e6", "gnd", 22.0) // R18
        .resistor("c6", "b6", 470_000.0) // R16
        .capacitor("c6", "b6", 100e-12); // C10

    // Q7 is the complementary 2SA970-GR **PNP**. Its emitter goes toward the
    // positive rail through 120 ohm, its collector toward ground through 10 k,
    // and R29/C14 are the same collector-to-base shunt feedback used around
    // Q6. The old model instantiated this as another NPN, which is a topology
    // error rather than a harmless transistor approximation.
    net.capacitor("c6", "q7_in", 0.047e-6) // C11
        .resistor("q7_in", "b7", 22_000.0) // R27
        .resistor("v9a", "b7", 100_000.0) // R26
        .bipolar_pnp("c7", "b7", "e7", SMALL_SIGNAL_BJT) // Q7, 2SA970-GR
        .resistor("v9a", "e7", 120.0) // R28
        .resistor("c7", "gnd", 10_000.0) // collector load
        .resistor("c7", "b7", 470_000.0) // R29
        .capacitor("c7", "b7", 100e-12); // C14

    // --- Dist, VR4 -------------------------------------------------------------------
    // This is the unusual part of the real HM-2 and the reason a plain
    // pre-clipper attenuator gives the wrong control law. VR4 is a 250 kD pot
    // with its **wiper grounded**. One end is AC-coupled back to Q6's collector
    // through C31/R49; the other end continues through R25/C22 to IC1B's
    // inverting input. Turning DIST up therefore does two things at once:
    //
    //   low:  Q6 is strongly shunted + IC1B has a large ground-leg resistance
    //   high: Q6 is barely shunted   + IC1B has a small ground-leg resistance
    //
    // Boss used a D/log-like track. `Audio` is deliberately used instead of
    // the old linear track so the electrical action is spread over the knob's
    // travel instead of piling almost all useful range at one end.
    //
    // `Pot(a, wiper, b)`: at p=0, b->wiper is ~0 and a->wiper is the full
    // track. Put the Q6 shunt on `b` and the feedback leg on `a` so p=0 really
    // is minimum distortion.
    net.capacitor("c6", "dist_shunt_c", 10e-6) // C31
        .resistor("dist_shunt_c", "dist_shunt", 150.0) // R49
        .pot(
            "dist_fb",
            "gnd",
            "dist_shunt",
            250_000.0,
            Taper::Audio,
            DIST,
        )
        .resistor("dist_fb", "dist_fb_r", 47_000.0) // R25
        .capacitor("dist_fb_r", "u1b_m", 0.047e-6); // C22

    // --- asymmetric soft clipper, IC1B ----------------------------------------------
    // Q7 drives the non-inverting input directly. A 68 k resistor returns that
    // node to the 4.5 V reference; the DIST-controlled leg above sets the closed-loop
    // gain at the inverting input. D3 against D4+D5 gives the asymmetric soft
    // clipping in Boss's drawing.
    net.resistor("c7", "vref", 68_000.0) // bias return
        .opamp_biased("u1b", "c7", "u1b_m", "vref", SWING)
        .resistor("u1b", "u1b_m", 220_000.0) // R20
        .capacitor("u1b", "u1b_m", 100e-12) // C9
        .diode("u1b_m", "u1b", DiodeSpec::SILICON) // D3
        .diode("u1b", "d45", DiodeSpec::SILICON) // D4
        .diode("d45", "u1b_m", DiodeSpec::SILICON); // D5

    // --- coring gate and hard clipper ------------------------------------------------
    // C12/R23 AC-couple IC1B into the anti-parallel germanium pair D6/D7.
    // R30 then feeds the *separate* D8/D9 hard clipper to ground, with C16
    // across it. The old model put R23/R30 to the 4.5 V bias and moved D8/D9
    // into IC1A's feedback loop. That is not the HM-2 schematic and lets this
    // section produce enormous internal excursions instead of bounding them.
    net.capacitor("u1b", "c12", 1e-6) // C12
        .resistor("c12", "gate_in", 10_000.0) // R23
        .diode("gate_in", "gate_out", CORING) // D6
        .diode("gate_out", "gate_in", CORING) // D7
        // Real germanium parts leak. Keeping a very large parallel resistance
        // prevents the intentionally-open coring node from becoming a numerically
        // floating island without materially bypassing the gate.
        .resistor("gate_in", "gate_out", 10_000_000.0)
        .resistor("gate_out", "hard_clip", 10_000.0) // R30
        .diode("hard_clip", "gnd", DiodeSpec::SILICON) // D8
        .diode("gnd", "hard_clip", DiodeSpec::SILICON) // D9
        .capacitor("hard_clip", "gnd", 0.001e-6); // C16

    // --- recovery/buffer, IC1A --------------------------------------------------------
    // C15 lifts the ground-referenced hard-clipped signal back onto the 4.5 V
    // bias through the 68 k input return. IC1A is a voltage follower in the
    // original pedal; it is not another diode-feedback clipping amplifier.
    net.capacitor("hard_clip", "u1a_p", 1e-6) // C15
        .resistor("u1a_p", "vref", 68_000.0)
        .opamp_biased("u1a", "u1a_p", "u1a", "vref", SWING)
        // Keep the existing tone-section interface here. The distortion fix is
        // intentionally isolated from the already-verified Colour Mix Schur
        // partition; the tone network can be re-derived separately.
        .resistor("u1a", "eqfeed", 47_000.0);

    // --- the Colour Mix -------------------------------------------------------------------
    // A feedback equaliser, the same shape as the Mark IIC+'s graphic: op-amp 3a
    // with R52 3.3 k into one input and R54 3.3 k of feedback into the other, and
    // each control's track running between those two nodes with its resonant leg
    // on the wiper. Toward the input side the leg shunts the signal and cuts its
    // band; toward the feedback side it shunts the loop and boosts it; centred,
    // the two cancel and the control is flat.
    //
    // That is why this is a *boost and cut* tone section rather than the passive
    // losses the other pedals here have -- and why both knobs on full is a real
    // setting rather than simply the loudest one.
    // The Dist track is 250 k and the equaliser's input is 3.3 k, so the wiper
    // is buffered into it -- as the Mark IIC+'s graphic buffers its own input
    // with Q1 for the same reason. Without it the control would lose sixteen
    // decibels into the stack and the tone knobs would be working on nothing.
    // Ground-referenced from here on, with the wiper coupled in: the same
    // construction the Rodent's op-amp needed. Riding the 4.5 V bias through
    // the equaliser left the solver with a follower that would not follow
    // below about a tenth of a volt.
    net.capacitor("eqfeed", "eqac", 1e-6)
        .resistor("eqac", "gnd", 1_000_000.0);
    if partition_filters {
        net.linear_opamp("eqin", "eqac", "eqin");
    } else {
        net.opamp("eqin", "eqac", "eqin", SWING);
    }
    net.resistor("eqin", "cut", 3_300.0); // R52
                                          // IC3A is the actual boost/cut amplifier. Boss's service check specifies
                                          // roughly +21 dB at the Colour Mix resonances from centre to full clockwise.
                                          // With the hard-clipped signal feeding it, that is enough to hit the M5218's
                                          // finite output swing at the classic all-knobs-max setting. It therefore must
                                          // remain rail-aware even in the partitioned production circuit. Making this
                                          // op-amp linear/unlimited was only equivalent in the earlier moderate-level
                                          // probe and made the Swedish-death settings physically too loud.
    net.opamp("u3a", "cut", "boost", SWING);
    net.resistor("u3a", "boost", 3_300.0) // R54
        .capacitor("u3a", "boost", 470e-12); // C33

    // VR2, Colour Mix Low: the 87 Hz band.
    net.pot(
        "boost",
        "w_low",
        "cut",
        10_000.0,
        Taper::Symmetric { span: 150.0 },
        LOW,
    );
    band(&mut net, "w_low", "bl", LOW_BAND);

    // VR3, Colour Mix High: the two upper bands, both on the one knob.
    net.pot(
        "boost",
        "w_high",
        "cut",
        10_000.0,
        Taper::Symmetric { span: 150.0 },
        HIGH,
    );
    band(&mut net, "w_high", "bm", MID_BAND);
    band(&mut net, "w_high", "bh", HIGH_BAND);

    net.capacitor("u3a", "lvl_top", 10e-6) // C34
        .resistor("lvl_top", "gnd", 10_000.0); // R53

    // --- Level and the output buffer ----------------------------------------------------
    net.rest(LEVEL, LEVEL_REST)
        .pot("lvl_top", "lvl", "gnd", 10_000.0, Taper::Audio, LEVEL) // VR1
        // C32 straight on to the output buffer. Q10 is on the drawing between
        // them, but it sits in the switching corner with D12 and R60 -- it is
        // one of the two JFETs that make this pedal's bypass, like Q1 at the
        // input, not a stage. Built as an amplifier it cost a nonlinear device
        // and two nodes for nothing.
        .capacitor("lvl", "q10", 1e-6) // C32
        .resistor("q10", "vref", 1_000_000.0) // R60
        .capacitor("q10", "b3", 0.047e-6) // C7
        .resistor("b3", "vref", 1_000_000.0) // R48
        .bipolar("v9", "b3", "e3", SMALL_SIGNAL_BJT)
        .resistor("e3", "gnd", 10_000.0) // R13
        .capacitor("e3", "out", 1e-6) // C3
        .resistor("out", "gnd", 10_000.0) // R3
        .resistor("out", "gnd", load);

    net.build(at)
}

#[cfg(test)]
mod partition_tests {
    use super::*;
    use crate::dsp::time::Simulation;

    #[test]
    fn colour_mix_opamps_leave_newton_boundary_without_changing_linear_operation() {
        let mut partitioned = Simulation::new(build(10_000.0, 470_000.0).unwrap(), 48_000.0);
        let mut reference = Simulation::new(
            build_full_newton_reference(10_000.0, 470_000.0).unwrap(),
            48_000.0,
        );

        for (control, value) in [(DIST, 0.7), (LOW, 0.85), (HIGH, 0.8), (LEVEL, 0.6)] {
            partitioned.set_control(control, value);
            reference.set_control(control, value);
        }

        let before = reference
            .nonlinear_reduction()
            .expect("reference reduction")
            .0;
        let after = partitioned
            .nonlinear_reduction()
            .expect("partitioned reduction")
            .0;
        assert!(
            after < before,
            "HM-2 boundary did not shrink: {before} -> {after}"
        );

        let mut worst = 0.0_f64;
        for k in 0..12_000 {
            let t = k as f64 / 48_000.0;
            let x = 0.018
                * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.7
                    + (std::f64::consts::TAU * 164.8 * t).sin() * 0.3);
            let a = partitioned.process(x);
            let b = reference.process(x);
            worst = worst.max((a - b).abs());
        }
        assert!(
            worst < 1e-6,
            "HM-2 linear filter partition changed response: {worst:e}"
        );
    }

    #[test]
    fn colour_mix_partition_keeps_rail_clipping_at_all_knobs_max() {
        let mut partitioned = Simulation::new(build(10_000.0, 470_000.0).unwrap(), 48_000.0);
        let mut reference = Simulation::new(
            build_full_newton_reference(10_000.0, 470_000.0).unwrap(),
            48_000.0,
        );

        for (control, value) in [(DIST, 1.0), (LOW, 1.0), (HIGH, 1.0), (LEVEL, 1.0)] {
            partitioned.set_control(control, value);
            reference.set_control(control, value);
        }
        partitioned.find_operating_point();
        reference.find_operating_point();

        let before = reference
            .nonlinear_reduction()
            .expect("reference reduction")
            .0;
        let after = partitioned
            .nonlinear_reduction()
            .expect("partitioned reduction")
            .0;
        assert!(
            after < before,
            "HM-2 boundary did not shrink: {before} -> {after}"
        );

        let mut worst = 0.0_f64;
        let mut peak_partitioned = 0.0_f64;
        let mut peak_reference = 0.0_f64;
        for k in 0..24_000 {
            let t = k as f64 / 48_000.0;
            // Hot B-standard-ish two-note input, deliberately harder than the
            // old moderate partition probe. This is the operating region of the
            // Swedish-death preset where IC3A's +21 dB resonant boost can rail.
            let x = 0.122
                * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.65
                    + (std::f64::consts::TAU * 123.5 * t).sin() * 0.35);
            let a = partitioned.process(x);
            let b = reference.process(x);
            worst = worst.max((a - b).abs());
            peak_partitioned = peak_partitioned.max(a.abs());
            peak_reference = peak_reference.max(b.abs());
        }

        assert!(
            worst < 1e-6,
            "HM-2 partition lost Colour Mix rail clipping: worst={worst:e}, partitioned_peak={peak_partitioned:.6}, reference_peak={peak_reference:.6}"
        );
    }
}
