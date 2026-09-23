//! The Roland JC-120, CH-1.
//!
//! Every figure checked here is printed on the PA BOARD ASSY sheet of the
//! Sep. 2000 JC-120UT/JT service notes or on that document's specification
//! page. None of it was read out of this model and then asserted back at it.
//!
//! The sheet carries three test points and a Setting block that says what they
//! were taken under -- `VOL, EQ MAX`, `BRI OFF` -- which makes the channel's
//! gain a documented number rather than an estimated one. That is the strongest
//! check available on a solid-state circuit whose only nonlinear parts are two
//! JFETs and a follower, because it exercises the device models and the bias
//! network together.
use gainstagefx::circuits::jazz120::{
    self, BASS, BRIGHT_OFF_OHMS, BRIGHT_ON_OHMS, BRIGHT_SLOT, CHANNEL_OUT, HEAD_OUT,
    INPUT_HIGH_SERIES_OHMS, INPUT_LOW_SERIES_OHMS, INPUT_SERIES_SLOT, MIDDLE, STACK_OUT, TREBLE,
    VOLUME,
};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;
const SOURCE: f64 = 10_000.0;
/// What the main amplifier board presents at CN3.
const LOAD: f64 = 1_000_000.0;

/// The sheet's own measurement condition.
fn wide_open(at: &str) -> Simulation {
    let mut s = Simulation::new(jazz120::tap(SOURCE, LOAD, at).expect("JC-120 builds"), RATE);
    for control in [VOLUME, TREBLE, MIDDLE, BASS] {
        s.set_control(control, 1.0);
    }
    s.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
    s
}

fn level(s: &mut Simulation, hz: f64) -> f64 {
    let tone = Tone::near(RATE, 16_384, hz, 0.005);
    measure::run(tone, (RATE / 5.0) as usize, |x| s.process(x)).gain_db()
}

/// The sheet prints -30 dBm at the CH1 HIGH jack and -10 dBm at J3's drain:
/// twenty decibels across the input stage, with the tone network hanging on
/// that drain and loading it.
///
/// This is the check on `JfetSpec::J2SK184`. Nothing in the stage is adjusted
/// to reach it -- the drain load, the two source resistors and the bias
/// divider are all read off the sheet, so the only thing that can put the gain
/// wrong is the device model.
#[test]
fn the_input_stage_makes_the_sheet_s_twenty_decibels() {
    let mut s = wide_open(HEAD_OUT);
    let db = level(&mut s, 1_000.0);
    assert!(
        (db - 20.0).abs() < 1.5,
        "J3's drain is {db:.1} dB above the jack; the sheet's two test points \
         say 20 dB (-30 dBm in, -10 dBm at the drain)"
    );
}

/// And -30 dBm to +13 dBm at CN3: forty-three decibels across the whole
/// channel, tone network and all.
#[test]
fn the_channel_makes_the_sheet_s_forty_three_decibels() {
    let mut s = wide_open(CHANNEL_OUT);
    let db = level(&mut s, 1_000.0);
    assert!(
        (db - 43.0).abs() < 1.5,
        "the channel gives {db:.1} dB; the sheet's test points say 43 dB \
         (-30 dBm at the jack, +13 dBm at CN3)"
    );
}

/// The direct-current solve, against what the sheet's own values require.
///
/// +VA is the MTZ-30B zener, and R20's 680 ohms drops it to +VO under the
/// channel's own current -- so +VO is not a chosen number, it is what the
/// three stages draw. The two drains have to land with room to swing: the
/// sheet asks CN3 for 9.79 Vpp, which Q2's emitter has to be able to deliver.
#[test]
fn the_operating_point_leaves_room_for_the_sheet_s_output() {
    let circuit = jazz120::build(SOURCE, LOAD).expect("JC-120 builds");
    let mut s = Simulation::new(circuit.clone(), RATE);
    assert!(s.find_operating_point(), "the DC solve converges");

    let at = |name: &str| {
        let node = circuit
            .unknown_named(name)
            .unwrap_or_else(|| panic!("{name} is a node"));
        s.voltage_at(node)
    };

    let vo = at("vo");
    assert!(
        (24.0..30.0).contains(&vo),
        "+VO is {vo:.2} V; R20's 680 ohms off the 30 V zener cannot drop more \
         than a few volts at this channel's current"
    );

    // Both stages are common-source with the drain load off +VO, so both
    // drains have to idle between the rails rather than against one.
    for name in ["j3_d", "q1_d"] {
        let v = at(name);
        assert!(
            v > 0.2 * vo && v < 0.8 * vo,
            "{name} idles at {v:.2} V on a {vo:.2} V rail -- a starved or \
             saturated common-source stage, not a biased one"
        );
    }

    // Q2 is an emitter follower, so its emitter sits a base-emitter drop below
    // Q1's drain and has to hold the sheet's +-4.9 V.
    let q1_d = at("q1_d");
    let q2_e = at("q2_e");
    assert!(
        (0.4..0.9).contains(&(q1_d - q2_e)),
        "Q2 drops {:.2} V base to emitter, which is not a conducting silicon \
         junction",
        q1_d - q2_e
    );
    assert!(
        q2_e > 4.9 && vo - q2_e > 4.9,
        "Q2's emitter at {q2_e:.2} V on a {vo:.2} V rail cannot deliver the \
         sheet's 9.79 Vpp"
    );
}

/// The published input impedance, 680 kOhm.
///
/// It is R8, the gate return, and nothing else: there is no resistor from the
/// head amp's input to ground, so the only path is through the gate return
/// into the bias divider. The HIGH jack's 33 k sits in front of it.
///
/// Measured at 100 Hz on purpose. Roland's figure is the resistive one, and by
/// a kilohertz C4's 150 pF and C8's Miller-multiplied 15 pF have pulled the
/// magnitude a third of the way down -- which is the amplifier's behaviour,
/// not a discrepancy, but it is not what the specification means.
#[test]
fn the_input_impedance_is_the_published_680k() {
    let mut s = wide_open(HEAD_OUT);
    // Two source impedances: the loss between them is the input impedance,
    // whatever else the stage does with the signal.
    let near = level(&mut s, 100.0);
    let mut far = Simulation::new(
        jazz120::tap(680_000.0, LOAD, HEAD_OUT).expect("JC-120 builds"),
        RATE,
    );
    for control in [VOLUME, TREBLE, MIDDLE, BASS] {
        far.set_control(control, 1.0);
    }
    far.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
    let loss = near - level(&mut far, 100.0);
    // 680 k of gate return behind the HIGH jack's 33 k is 713 k, so a 680 k
    // source costs 20 log10((713 + 680) / (713 + 10)) = 5.7 dB.
    assert!(
        (5.0..6.5).contains(&loss),
        "a 680 k source costs {loss:.1} dB, so the input impedance is not the \
         published 680 k"
    );
}

/// The two jacks, from the input board: 33 k on HIGH and 68 k on LOW, into
/// the same node. The sheet prints -30 dBm and -20 dBm as their sensitivities,
/// so LOW has to be the quieter path by roughly that much less than the
/// resistor ratio suggests -- the 680 k it works into dominates both.
#[test]
fn the_low_jack_is_the_quieter_one() {
    let mut high = wide_open(HEAD_OUT);
    high.set_value(INPUT_SERIES_SLOT, INPUT_HIGH_SERIES_OHMS);
    let mut low = wide_open(HEAD_OUT);
    low.set_value(INPUT_SERIES_SLOT, INPUT_LOW_SERIES_OHMS);
    let drop = level(&mut high, 1_000.0) - level(&mut low, 1_000.0);
    assert!(
        (0.1..1.5).contains(&drop),
        "LOW is {drop:.2} dB below HIGH; 33 k and 68 k into 680 k is a small \
         difference, not none and not a large one"
    );
}

/// The tone network is passive and its bottom returns to ground, so it can
/// only ever lose. A network that shows gain anywhere has been mis-wired --
/// which is exactly the failure the first reading of this circuit had, when
/// the stack was suspected of sitting inside a feedback loop.
#[test]
fn the_tone_network_is_passive_everywhere() {
    for (t, m, b) in [
        (0.0, 0.0, 0.0),
        (1.0, 1.0, 1.0),
        (1.0, 0.0, 1.0),
        (0.0, 1.0, 0.0),
        (0.5, 0.5, 0.5),
    ] {
        for hz in [50.0, 350.0, 1_000.0, 10_000.0] {
            let mut head = Simulation::new(
                jazz120::tap(SOURCE, LOAD, HEAD_OUT).expect("JC-120 builds"),
                RATE,
            );
            let mut out = Simulation::new(
                jazz120::tap(SOURCE, LOAD, STACK_OUT).expect("JC-120 builds"),
                RATE,
            );
            for s in [&mut head, &mut out] {
                s.set_control(TREBLE, t);
                s.set_control(MIDDLE, m);
                s.set_control(BASS, b);
                s.set_control(VOLUME, 1.0);
                s.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
            }
            let through = level(&mut out, hz) - level(&mut head, hz);
            assert!(
                through < 0.1,
                "the stack gives {through:.2} dB at {hz} Hz with treble {t}, \
                 middle {m}, bass {b} -- a passive divider cannot"
            );
        }
    }
}

/// Every control has to do something, in the direction its panel says.
#[test]
fn each_control_moves_its_own_band_the_right_way() {
    for (control, hz, name) in [
        (TREBLE, 10_000.0, "Treble"),
        (MIDDLE, 350.0, "Middle"),
        (BASS, 50.0, "Bass"),
    ] {
        let mut s = wide_open("vol");
        s.set_control(TREBLE, 0.5);
        s.set_control(MIDDLE, 0.5);
        s.set_control(BASS, 0.5);
        s.set_control(VOLUME, 0.5);

        s.set_control(control, 0.0);
        let down = level(&mut s, hz);
        s.set_control(control, 1.0);
        let up = level(&mut s, hz);
        assert!(
            up - down > 3.0,
            "{name} moves {hz} Hz by {:.1} dB from minimum to maximum, and the \
             wrong way round if that is negative -- check the rheostat taper",
            up - down
        );
    }
}

/// SW2 shorts R4 so C7 couples fully across the Volume; it does not switch the
/// capacitor in and out. Off, the 680 k leaves a little tilt rather than none.
#[test]
fn the_bright_switch_lifts_the_top_and_only_the_top() {
    let mut s = wide_open("vol");
    s.set_control(VOLUME, 0.5);
    s.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
    let (off_top, off_bottom) = (level(&mut s, 10_000.0), level(&mut s, 100.0));
    s.set_value(BRIGHT_SLOT, BRIGHT_ON_OHMS);
    let (on_top, on_bottom) = (level(&mut s, 10_000.0), level(&mut s, 100.0));

    assert!(
        on_top - off_top > 2.0,
        "BRI lifts 10 kHz by {:.1} dB, which is not a bright switch",
        on_top - off_top
    );
    assert!(
        (on_bottom - off_bottom).abs() < 0.5,
        "BRI moves 100 Hz by {:.1} dB, so it is not only touching the top",
        on_bottom - off_bottom
    );
}

/// Nothing in this channel depends on the sample rate: it is a solid-state
/// circuit with no time constant near the audio band's top.
#[test]
fn the_channel_is_the_same_at_every_rate() {
    let mut first = None;
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut s = Simulation::new(
            jazz120::tap(SOURCE, LOAD, CHANNEL_OUT).expect("JC-120 builds"),
            rate,
        );
        for control in [VOLUME, TREBLE, MIDDLE, BASS] {
            s.set_control(control, 1.0);
        }
        s.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
        let tone = Tone::near(rate, 16_384, 1_000.0, 0.005);
        let db = measure::run(tone, (rate / 5.0) as usize, |x| s.process(x)).gain_db();
        match first {
            None => first = Some(db),
            Some(f) => assert!(
                (db - f).abs() < 0.2,
                "{rate} Hz gives {db:.2} dB against {f:.2} dB at 44.1 kHz"
            ),
        }
    }
}

/// The Volume is the amplifier's own 1 M pot and the plugin's Drive knob turns
/// it, so it has to behave like a level control across its whole travel: no
/// dead zone at the bottom, no step, and monotonic.
///
/// A 1 M linear track feeding a 1.5 M gate return is light enough that it
/// stays a divider rather than a loaded one, and the bright network across it
/// is 330 pF -- inaudible at the frequency this is measured at.
#[test]
fn the_volume_is_a_real_monotonic_level_control() {
    let mut last = f64::NEG_INFINITY;
    let mut levels = Vec::new();
    for step in 0..=10 {
        let position = f64::from(step) / 10.0;
        let mut s = Simulation::new(
            jazz120::tap(SOURCE, LOAD, CHANNEL_OUT).expect("JC-120 builds"),
            RATE,
        );
        for control in [TREBLE, MIDDLE, BASS] {
            s.set_control(control, 0.5);
        }
        s.set_control(VOLUME, position);
        s.set_value(BRIGHT_SLOT, BRIGHT_OFF_OHMS);
        let db = level(&mut s, 1_000.0);
        assert!(
            db > last,
            "Volume {position:.1} gives {db:.2} dB, below the step before it"
        );
        last = db;
        levels.push(db);
    }
    let span = levels[10] - levels[0];
    assert!(
        span > 40.0,
        "the Volume covers only {span:.1} dB end to end"
    );
}

/// CH-1 runs from one supply, so **every node in it is between ground and
/// that supply** and the channel clips when it runs out of either. That is not
/// a modelling preference, it is what a single-rail circuit can do.
///
/// It is checked here because it was not true. The second 2SK184 is driven
/// hard enough to take its drain below its source, and the JFET law returned
/// zero current with zero slope for any `vds <= 0` -- a cliff, and physically
/// wrong, because a JFET channel is symmetric and conducts both ways. The
/// solve fell off it: 40,760 fallbacks in 9,600 samples, twelve Newton passes
/// a sample, and `q1_d` answering **-23 V on a +27 V rail**. That number is
/// not a solution; it is what the fallback left behind, and converted to audio
/// it is the crackle the amplifier was reported for.
///
/// See `Jfet::drain` and `examples/stutter.rs`.
#[test]
fn hard_driven_the_channel_stays_inside_its_own_supply() {
    // The channel's supply, which `docs/models/jazz_120.md` derives from the
    // MTZ-30B zener and the DC solve puts at 27.2 V. A volt of margin, because
    // this is a bound on physics and not a fit.
    const SUPPLY: f64 = 27.2 + 1.0;
    for at in [HEAD_OUT, "q1_d"] {
        for volts in [0.122, 0.488, 0.976, 2.0] {
            let mut s =
                Simulation::new(jazz120::tap(SOURCE, LOAD, at).expect("JC-120 builds"), RATE);
            for control in [TREBLE, MIDDLE, BASS] {
                s.set_control(control, 0.5);
            }
            s.set_control(VOLUME, 0.55);
            s.find_operating_point();
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for k in 0..(RATE as usize / 10) {
                let t = k as f64 / RATE;
                let y = s.process(volts * (std::f64::consts::TAU * 220.0 * t).sin());
                // Past the coupling capacitors' settling.
                if k > RATE as usize / 20 {
                    lo = lo.min(y);
                    hi = hi.max(y);
                }
            }
            assert!(
                lo > -1.0 && hi < SUPPLY,
                "{at} at {volts} V reaches {lo:.2} .. {hi:.2} V, outside 0 .. {SUPPLY} V"
            );
        }
    }
}

/// And the solve that produces it settles. A node inside its rails that took a
/// fallback to get there is still wrong; this is the other half of the check.
#[test]
fn hard_driving_the_channel_does_not_defeat_the_solver() {
    for volts in [0.488, 0.976, 2.0] {
        let mut s = Simulation::new(
            jazz120::tap(SOURCE, LOAD, CHANNEL_OUT).expect("JC-120 builds"),
            RATE,
        );
        for control in [TREBLE, MIDDLE, BASS] {
            s.set_control(control, 0.5);
        }
        s.set_control(VOLUME, 0.55);
        s.find_operating_point();
        let before = s.health();
        let n = RATE as usize / 10;
        for k in 0..n {
            let t = k as f64 / RATE;
            s.process(volts * (std::f64::consts::TAU * 220.0 * t).sin());
        }
        let fallbacks = s.health().1 - before.1;
        assert!(
            fallbacks < n as u64 / 100,
            "{volts} V: {fallbacks} fallbacks in {n} samples"
        );
    }
}
