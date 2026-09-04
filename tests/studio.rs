//! Preamplifier channels: what makes one different from a guitar amplifier,
//! and from each other.

use gainstagefx::circuits::studio::{self, Amplifier, Values, GAIN};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// A channel with an output transformer bolted on, which is what the panel's
/// Iron control puts there. The catalogue itself carries only the input
/// transformer, because that is the part that makes a console a console.
fn ironed(mut v: Values) -> Values {
    v.output_iron = Some(gainstagefx::circuits::iron::OUTPUT);
    v
}

fn run(v: &Values, hz: f64, volts: f64, gain: f64) -> measure::Measured {
    let c = studio::build(v, 200.0, 10_000.0).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(GAIN, gain);
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 4.0) as usize, |x| sim.process(x))
}

/// All three build, and they are not the same circuit.
#[test]
fn the_channels_build_and_differ() {
    let sizes: Vec<usize> = [studio::CONSOLE, studio::VALVE_CHANNEL, studio::STUDIO]
        .iter()
        .map(|v| studio::build(v, 200.0, 10_000.0).expect("builds").nodes)
        .collect();
    assert!(
        sizes.iter().any(|n| *n != sizes[0]),
        "three channels that all came out the same size: {sizes:?}"
    );
    assert_eq!(studio::CONSOLE.amplifier, Amplifier::Jfet);
    assert_eq!(studio::VALVE_CHANNEL.amplifier, Amplifier::Valve);
    assert_eq!(studio::STUDIO.amplifier, Amplifier::OpAmp);
}

/// An op-amp with enough loop gain contributes nothing of its own until it
/// hits the rail. That is what it is for, and it is why a studio channel needs
/// its output transformer to have any character at all.
#[test]
fn a_studio_channel_has_no_character_of_its_own_in_the_band() {
    let m = run(&ironed(studio::STUDIO), 2000.0, 0.05, 1.0);
    assert!(
        m.thd_percent() < 0.1,
        "an op-amp channel should be clean across the band: {:.3} %",
        m.thd_percent()
    );
    let console = run(&ironed(studio::CONSOLE), 2000.0, 0.05, 1.0);
    assert!(
        console.thd_percent() > m.thd_percent() * 10.0,
        "a discrete channel should be audibly less clean than an op-amp one: \
         {:.2} % against {:.3} %",
        console.thd_percent(),
        m.thd_percent()
    );
}

/// The iron is what colours the bottom of the band, on every channel.
///
/// Measured as the *difference* the transformer makes rather than as a ratio
/// across the band, because a ratio only shows it on a channel whose gain
/// device is clean. The valve channel reads 2.4 per cent at 40 Hz and 1.8 at
/// 2 kHz -- barely a ratio at all -- not because its iron does nothing but
/// because the bottle is making 1.8 per cent everywhere and swamping it. Take
/// the transformers out and the bottom end comes back into line with the top,
/// which is the thing actually worth asserting.
#[test]
fn the_iron_is_what_colours_the_bottom() {
    for (name, v) in [
        ("console", studio::CONSOLE),
        ("valve channel", studio::VALVE_CHANNEL),
        ("studio", studio::STUDIO),
    ] {
        let with_iron = ironed(v);
        let mut bare = v;
        bare.input_iron = None;
        bare.output_iron = None;

        let tilt = |values: &Values| {
            let low = run(values, 40.0, 0.05, 1.0).thd_percent();
            let high = run(values, 2000.0, 0.05, 1.0).thd_percent();
            (low, high)
        };
        let (low, high) = tilt(&with_iron);
        let (bare_low, bare_high) = tilt(&bare);
        println!(
            "{name}: with iron {low:.2} % / {high:.2} %, without {bare_low:.2} % / {bare_high:.2} %"
        );

        // Without iron the gain device treats the bottom of the band much like
        // the top: whatever tilt there is, is small.
        assert!(
            bare_low < bare_high.max(0.01) * 2.0,
            "{name} without iron still tilts: {bare_low:.3} % at 40 Hz against \
             {bare_high:.3} % at 2 kHz, so something other than the transformer \
             is frequency dependent"
        );
        // With it, the bottom is much worse than it was.
        assert!(
            low > bare_low * 3.0 + 0.5,
            "{name}: putting the iron in should colour the bottom -- \
             {bare_low:.2} % became {low:.2} %"
        );
    }
}

/// Take the iron out and the character goes with it, which is the check that
/// the transformer is doing the work rather than the gain stage.
#[test]
fn the_iron_is_where_the_bottom_end_comes_from() {
    let with_iron = run(&ironed(studio::STUDIO), 40.0, 0.05, 1.0).thd_percent();
    let without = run(&studio::STUDIO, 40.0, 0.05, 1.0).thd_percent();
    println!("with iron {with_iron:.2} %, without {without:.3} %");
    assert!(
        with_iron > without * 20.0,
        "the output transformer should be the whole of it: {with_iron:.2} % \
         with against {without:.3} % without"
    );
}

/// A single-ended stage is asymmetric, so it makes even harmonics. A JFET
/// follows a square law, which has only a second-order term, so it makes them
/// more purely than anything else here.
#[test]
fn the_discrete_stages_make_even_harmonics() {
    for (name, v) in [
        ("console", studio::CONSOLE),
        ("valve channel", studio::VALVE_CHANNEL),
    ] {
        let m = run(&ironed(v), 2000.0, 0.05, 1.0);
        assert!(
            m.harmonic_percent(2) > m.harmonic_percent(3) * 3.0,
            "{name} made {:.2} % second against {:.2} % third, which is not a \
             single-ended stage",
            m.harmonic_percent(2),
            m.harmonic_percent(3)
        );
    }
}

/// Turning the gain down has to clean up, at every setting, on every channel.
///
/// It did not, once. With the control between the gain stage and the output
/// transformer, the pot itself was what drove the iron -- and a pot's wiper
/// impedance is highest in the middle of its travel, around 210 k here against
/// 22 k wide open. A transformer colours according to what feeds it, so the
/// channel measured 18 per cent distortion at half gain and 5 at full: the
/// knob inverted the character as it crossed the middle.
///
/// Measured at 40 Hz, because that is where all three channels have something
/// to clean up. At 220 Hz the op-amp channel reads 0.000 per cent at every
/// setting, which is correct and tells you nothing.
#[test]
fn turning_the_gain_down_cleans_up() {
    for (name, v) in [
        ("console", studio::CONSOLE),
        ("valve channel", studio::VALVE_CHANNEL),
        ("studio", studio::STUDIO),
    ] {
        let mut last = -1.0;
        for gain in [0.2, 0.4, 0.6, 0.8, 1.0] {
            let thd = run(&ironed(v), 40.0, 0.05, gain).thd_percent();
            assert!(
                thd >= last - 0.05,
                "{name} at gain {gain} made {thd:.3} %, less than the \
                 {last:.3} % below it: the control inverts somewhere"
            );
            last = thd;
        }
        assert!(last > 0.5, "{name} never bends at all: {last:.3} %");
    }
}

/// The gain control covers the span it is designed to and no more.
///
/// Not "as much as possible". It used to run to silence, which is eighty
/// decibels, and eighty decibels of travel put everything worth hearing in the
/// last tenth of the knob: the High Gain voice made 0.1 per cent distortion at
/// a quarter turn and 25 at the stop. The pot works into a resistor now, and
/// the span between its ends is `preamp::SPAN` -- about eighteen decibels,
/// which is what the gain control on an amplifier of this kind covers.
///
/// Both bounds matter. Too little and the control does nothing; too much and
/// it does nothing until the end.
#[test]
fn the_gain_control_covers_its_designed_span() {
    let span_of = |v: &Values| {
        run(v, 220.0, 0.001, 1.0).gain_db() - run(v, 220.0, 0.001, 0.0).gain_db()
    };

    // The channels built round a valve or a transistor put the control where
    // the hardware does, as a volume in front of the stage.
    let designed = 20.0 * gainstagefx::circuits::preamp::SPAN.log10();
    for (name, v) in [("console", studio::CONSOLE), ("valve channel", studio::VALVE_CHANNEL)] {
        let span = span_of(&v);
        println!("{name}: a span of {span:.1} dB against {designed:.1} designed");
        assert!(
            (span - designed).abs() < 4.0,
            "{name} moves {span:.1} dB end to end, against the {designed:.1} \
             it is built for"
        );
    }

    // The op-amp channel is a different control: the pot is in the feedback
    // path, so its range is the ratio of that path to the leg rather than
    // anything to do with a volume in front of a valve.
    let span = span_of(&studio::STUDIO);
    let feedback = 20.0
        * ((studio::STUDIO.leg + studio::STUDIO.feedback) / studio::STUDIO.leg).log10();
    println!("studio: a span of {span:.1} dB, feedback path allows {feedback:.1}");
    assert!(
        span > 25.0 && span <= feedback + 1.0,
        "the op-amp channel moves {span:.1} dB, and its feedback path allows \
         {feedback:.1}"
    );
}

/// A microphone preamplifier is built not to run out of room, which is the
/// opposite of what a guitar preamplifier is for. The op-amp channel should
/// take a great deal more before it does.
#[test]
fn a_studio_channel_has_the_headroom() {
    let level = 0.05;
    let studio_thd = run(&ironed(studio::STUDIO), 220.0, level, 1.0).thd_percent();
    let console_thd = run(&ironed(studio::CONSOLE), 220.0, level, 1.0).thd_percent();
    println!("at {level} V: studio {studio_thd:.3} %, console {console_thd:.2} %");
    assert!(
        studio_thd < console_thd,
        "the op-amp channel should be the one with the headroom: \
         {studio_thd:.3} % against {console_thd:.2} %"
    );
}
