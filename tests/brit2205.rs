//! The Brit 2205 preamplifier and its power stage, against the manufacturer's
//! drawings and owner's manual. See `docs/models/brit_2205.md`.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::brit2205::{self, Clipper, BASS, GAIN, MASTER, MIDDLE, TREBLE, VOLUME};
use gainstagefx::circuits::{brit800, power};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{
    Chain, Gain, Level, PowerAmp, PowerModel, Settings, Tone as Stack, NOMINAL_DBFS,
};

const RATE: f64 = 96_000.0;
const SOURCE: f64 = 10_000.0;
const LOAD: f64 = 250_000.0;

fn sim(at: &str, controls: &[(usize, f64)]) -> Simulation {
    let mut s = Simulation::new(brit2205::tap(SOURCE, LOAD, at).expect("builds"), RATE);
    for &(control, value) in controls {
        s.set_control(control, value);
    }
    s
}

fn measure_at(controls: &[(usize, f64)], hz: f64, volts: f64) -> measure::Measured {
    let mut s = sim("out", controls);
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 4.0) as usize, |x| s.process(x))
}

/// The two figures anyone has published from this preamplifier: V1B's plate
/// and cathode, from one owner's notes on a 2210 (the same preamplifier) --
/// 83 V and 0.52 V -- and a second owner's 2205 at 60 V and 0.5 V. The cathode
/// is the diode's doing and is the sharper test: without D1 the valve would sit
/// three volts up its 10 k resistor.
#[test]
fn v1b_sits_where_the_diode_and_the_measured_amplifiers_put_it() {
    let circuit = brit2205::build(SOURCE, LOAD).unwrap();
    let names = ["a", "v1b_p", "v1b_k", "v1a_p", "v3a_p", "v4a_p"];
    let nodes: Vec<usize> = names
        .iter()
        .map(|n| circuit.unknown_named(n).expect("node"))
        .collect();
    let mut s = Simulation::new(circuit, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = nodes.iter().map(|&n| s.voltage_at(n)).collect();
    for (n, x) in names.iter().zip(&v) {
        println!("{n}: {x:.2} V");
    }
    let (plate, cathode) = (v[1], v[2]);
    assert!((0.45..0.6).contains(&cathode), "V1B cathode {cathode:.3} V");
    assert!((60.0..100.0).contains(&plate), "V1B plate {plate:.1} V");
    // Every other plate well inside its rail and well off the floor.
    for (&x, n) in v[3..].iter().zip(&names[3..]) {
        assert!(
            x > 100.0 && x < v[0],
            "{n}: {x:.1} V on a {:.1} V rail",
            v[0]
        );
    }
}

/// DB1 and D6 are built as one diode each way with three junctions' emission.
/// That is exact for identical junctions; this builds the bridge as drawn and
/// holds the two to the same waveform, hard enough to be clipping.
#[test]
fn the_folded_clipper_is_the_bridge_as_drawn() {
    let run = |clipper: Clipper| -> Vec<f64> {
        let mut s = Simulation::new(
            brit2205::tap_with(SOURCE, LOAD, "x", clipper).unwrap(),
            RATE,
        );
        s.set_control(GAIN, 0.9);
        assert!(s.find_operating_point());
        (0..(RATE as usize / 10))
            .map(|k| s.process(0.05 * (std::f64::consts::TAU * 220.0 * k as f64 / RATE).sin()))
            .collect()
    };
    let (folded, bridge) = (run(Clipper::Folded), run(Clipper::Bridge));
    let peak = folded.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let worst = folded
        .iter()
        .zip(&bridge)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!("clipper node peak {peak:.3} V, largest difference {worst:.2e} V");
    // It is clipping: three junctions each way hold it near two volts.
    assert!((1.2..2.6).contains(&peak), "{peak}");
    assert!(worst < 1e-3 * peak, "{worst:e} against a {peak} V peak");
}

/// The owner's manual gives each boost-channel tone control's swing, "all
/// controls maximum unless otherwise stated": Treble 28 dB at 5 kHz with the
/// Middle at minimum, Middle 15 dB at 400 Hz, Bass 23 dB at 50 Hz. The stack
/// here is not a 2203's -- Middle and Bass are rheostats to ground and it is
/// driven from a plate through 10 k -- and these three figures are what say it
/// was read right.
#[test]
fn the_tone_controls_swing_what_the_manual_says() {
    let full = |extra: &[(usize, f64)]| {
        let mut c = vec![
            (GAIN, 0.5),
            (VOLUME, 1.0),
            (MASTER, 1.0),
            (TREBLE, 1.0),
            (MIDDLE, 1.0),
            (BASS, 1.0),
        ];
        c.extend_from_slice(extra);
        c
    };
    let db = |extra: &[(usize, f64)], hz: f64| measure_at(&full(extra), hz, 1e-6).gain_db();
    let treble = db(&[(MIDDLE, 0.0)], 5_000.0) - db(&[(MIDDLE, 0.0), (TREBLE, 0.0)], 5_000.0);
    let middle = db(&[], 400.0) - db(&[(MIDDLE, 0.0)], 400.0);
    let bass = db(&[], 50.0) - db(&[(BASS, 0.0)], 50.0);
    println!("treble {treble:.1} dB (28), middle {middle:.1} dB (15), bass {bass:.1} dB (23)");
    for (name, got, manual) in [
        ("treble", treble, 28.0),
        ("middle", middle, 15.0),
        ("bass", bass, 23.0),
    ] {
        assert!(got > 0.0, "{name} runs backwards: {got:.1} dB");
        assert!(
            (got - manual).abs() < 3.0,
            "{name} {got:.1} dB against {manual} dB"
        );
    }
}

/// The Gain knob is two gangs of one pot, before and after V1B, so turning it
/// up both drives V1B harder and hands the clipper more. Its range is wide and
/// its distortion only goes up.
#[test]
fn the_gain_knob_sets_the_gain_and_the_distortion() {
    let small = |g: f64| measure_at(&[(GAIN, g)], 1_000.0, 1e-6).gain_db();
    let (low, high) = (small(0.1), small(1.0));
    println!("small-signal gain {low:.1} dB at 0.1, {high:.1} dB at 1.0");
    assert!(high - low > 40.0, "{low:.1} to {high:.1}");
    let thd = |g: f64| measure_at(&[(GAIN, g)], 220.0, 0.122).thd_percent();
    let curve: Vec<f64> = [0.2, 0.4, 0.6, 0.8, 1.0].iter().map(|&g| thd(g)).collect();
    println!("distortion at a guitar's level across Gain: {curve:.1?}");
    assert!(curve.windows(2).all(|w| w[1] > w[0] - 1.0), "{curve:?}");
    assert!(curve[4] > 30.0, "{curve:?}");
}

/// Why this amplifier exists in the catalogue: Morello's boost channel at
/// Gain 9 is not a 2203 turned up. With both preamplifiers at nine tenths and a
/// guitar's level going in, the 2205's is the far dirtier.
#[test]
fn the_boost_channel_is_hotter_than_the_2203() {
    let at_nine = |circuit: gainstagefx::dsp::netlist::Circuit, drive: usize| {
        let mut s = Simulation::new(circuit, RATE);
        s.set_control(drive, 0.9);
        let tone = Tone::near(RATE, 16_384, 220.0, 0.122);
        measure::run(tone, (RATE / 4.0) as usize, |x| s.process(x)).thd_percent()
    };
    let jcm2205 = at_nine(brit2205::build(SOURCE, LOAD).unwrap(), GAIN);
    let jcm2203 = at_nine(
        brit800::build(SOURCE, 1_000_000.0).unwrap(),
        brit800::VOLUME,
    );
    println!("2205 {jcm2205:.1} %, 2203 {jcm2203:.1} %");
    assert!(
        jcm2205 > jcm2203 * 1.5,
        "2205 {jcm2205:.1} % vs 2203 {jcm2203:.1} %"
    );
}

/// The Master is a rheostat from G to ground: nothing at zero, most at full,
/// and more with every step.
#[test]
fn the_master_runs_from_silence_up() {
    let db = |m: f64| measure_at(&[(MASTER, m)], 1_000.0, 1e-6).gain_db();
    let steps: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0].iter().map(|&m| db(m)).collect();
    println!("{steps:.1?}");
    assert!(steps.windows(2).all(|w| w[1] > w[0]), "{steps:?}");
    assert!(steps[4] - steps[0] > 40.0, "{steps:?}");
}

#[test]
fn silence_in_is_silence_out() {
    let mut s = sim("out", &[(GAIN, 1.0)]);
    assert!(s.find_operating_point());
    let worst = (0..(RATE as usize / 4))
        .map(|_| s.process(0.0).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.05, "{worst}");
}

/// The manual's sensitivity: 0.12 mV at 1 kHz for rated output, every control
/// full. The model's small-signal gain through both netlists is about 3 dB
/// short of that, and this holds it to within 8, which a mis-wired stage or a
/// lost gang would not be.
#[test]
fn the_whole_amplifier_is_about_as_sensitive_as_the_manual_says() {
    let rms_out = |input_rms: f64| -> f64 {
        let mut pre = sim("out", &[]);
        for c in [GAIN, VOLUME, TREBLE, MIDDLE, BASS, MASTER] {
            pre.set_control(c, 1.0);
        }
        let mut pwr = Simulation::new(
            power::build(&power::PowerSpec::BRIT_2205_EL34, 10_000.0).unwrap(),
            RATE,
        );
        pwr.set_control(power::PRESENCE, 1.0);
        let tone = Tone::near(RATE, 16_384, 1_000.0, input_rms * 2f64.sqrt());
        let m = measure::run(tone, (RATE / 4.0) as usize, |x| pwr.process(pre.process(x)));
        m.fundamental().magnitude() / 2f64.sqrt()
    };
    // Small-signal, so the gain is the gain and not the clipping.
    let gain_db = 20.0 * (rms_out(10e-6) / 10e-6).log10();
    let manual_db = 20.0 * ((50.0f64 * 4.0).sqrt() / 0.12e-3).log10();
    println!("whole-chain gain {gain_db:.1} dB against the manual's {manual_db:.1} dB");
    assert!(
        (gain_db - manual_db).abs() < 8.0,
        "{gain_db:.1} vs {manual_db:.1}"
    );
}

/// A 50 W amplifier: its two EL34s idle where the bias was set and it makes
/// about fifty watts into its tap before it squares off.
#[test]
fn the_power_stage_is_a_fifty_watt_pair() {
    let spec = &power::PowerSpec::BRIT_2205_EL34;
    let c = power::build(spec, 10_000.0).unwrap();
    let ht = c.unknown_named("ht").unwrap();
    let mut s = Simulation::new(c, RATE);
    assert!(s.find_operating_point());
    let each_ma = (spec.plate_supply - s.voltage_at(ht)) / spec.supply_resistance / 2.0 * 1e3;
    println!("idle {each_ma:.1} mA a valve");
    assert!((30.0..45.0).contains(&each_ma), "{each_ma}");
    let most = [2.0, 4.0, 8.0, 16.0]
        .iter()
        .map(|&v| {
            let mut s = Simulation::new(power::build(spec, 10_000.0).unwrap(), RATE);
            let tone = Tone::near(RATE, 16_384, 1_000.0, v);
            let m = measure::run(tone, (RATE / 4.0) as usize, |x| s.process(x));
            let vrms = m.fundamental().magnitude() / 2f64.sqrt();
            vrms * vrms / spec.speaker
        })
        .fold(0.0, f64::max);
    println!("most {most:.1} W");
    assert!((45.0..75.0).contains(&most), "{most}");
}

#[test]
fn matched_is_the_2205_stage_and_the_master_is_in_the_preamp() {
    assert_eq!(
        PowerAmp::Matched.resolved(Gain::Brit2205),
        Some(PowerModel::Brit2205EL34)
    );
    assert_eq!(
        Gain::Brit2205.level_control(),
        Some(Level::Circuit(brit2205::MASTER))
    );
    assert_eq!(Gain::Brit2205.drive_control(), brit2205::GAIN);
}

/// At every rate the plugin runs at, the whole chain -- preamplifier, power
/// stage and all -- stays finite, does the work, and does not allocate.
#[test]
fn the_chain_is_realtime_safe_at_every_rate() {
    let settings = Settings {
        gain: Gain::Brit2205,
        drive: 0.9,
        tone: Stack::Off,
        oversampling: 1,
        ..Settings::default()
    };
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&settings);
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let before = chain.solver_breakdown();
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = amplitude * (k as f64 * std::f64::consts::TAU * 110.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
        let work = chain.solver_breakdown().saturating_delta(before);
        assert!(work.gain.solves > 0 && work.power.solves > 0, "{rate}");
    }
}
