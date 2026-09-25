//! The Orange Dist (Boss DS-1, TA7136P), against Boss's service notes, the
//! TA7136AP data sheet and ElectroSmash's analysis. See
//! `docs/models/orange_dist.md`; `examples/ds1_op.rs` prints what these hold.
use gainstagefx::circuits::orange_dist::{self, DIST, LEVEL, LEVEL_REST, TONE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, PedalModel};
use gainstagefx::voice::{Gain, Level, Pedal, GUITAR_VOLTS};

const RATE: f64 = 96_000.0;

/// Small-signal gain from the jack to `node`, dB.
fn gain_to(node: &str, dist: f64, tone: f64, hz: f64) -> f64 {
    let mut s = Simulation::new(orange_dist::tap(10_000.0, 470_000.0, node).unwrap(), RATE);
    s.set_control(DIST, dist);
    s.set_control(TONE, tone);
    s.set_control(LEVEL, 1.0);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 1e-5);
    measure::run(t, (RATE / 4.0) as usize, |x| s.process(x)).gain_db()
}

/// The booster alone: Q2's collector over Q1's emitter, which drives it.
fn booster(hz: f64) -> f64 {
    gain_to("c2", 0.5, 0.5, hz) - gain_to("e1", 0.5, 0.5, hz)
}

/// It solves, and the operating point is the one the drawing implies: Q1's
/// emitter a base-emitter drop under its base, Q2's collector near the
/// middle of the battery, everything else on the 4.5 V bias point.
#[test]
fn it_settles_where_the_drawing_puts_it() {
    let c = orange_dist::build(10_000.0, 470_000.0).unwrap();
    let names = ["vref", "b1", "e1", "b2", "c2", "amp", "clip"];
    let idx: Vec<usize> = names.iter().map(|n| c.unknown_named(n).unwrap()).collect();
    let mut s = Simulation::new(c, RATE);
    assert!(s.find_operating_point());
    let v: Vec<f64> = idx.iter().map(|&i| s.voltage_at(i)).collect();
    println!("{names:?}\n{v:.3?}");
    let (vref, b1, e1, b2, c2, amp, clip) = (v[0], v[1], v[2], v[3], v[4], v[5], v[6]);
    assert!((vref - 4.5).abs() < 0.1, "bias {vref}");
    assert!((0.5..0.75).contains(&(b1 - e1)), "Q1 vbe {}", b1 - e1);
    assert!((0.55..0.75).contains(&b2), "Q2 base {b2}");
    assert!((3.0..6.0).contains(&c2), "Q2 collector {c2}");
    assert!((amp - vref).abs() < 0.05, "op-amp output {amp}");
    assert!((clip - vref).abs() < 0.01, "clipper {clip}");

    for _ in 0..(RATE as usize / 10) {
        assert!(s.process(0.0).is_finite());
    }
    let tail: Vec<f64> = (0..256).map(|_| s.process(0.0)).collect();
    let rms = (tail.iter().map(|v| v * v).sum::<f64>() / tail.len() as f64).sqrt();
    assert!(rms < 1e-4, "silence is not silent: {rms}");
}

/// ElectroSmash: the feedback "will lower this gain to 35dB". Above a
/// kilohertz, the booster makes 35-36 dB.
#[test]
fn the_booster_makes_35_db() {
    for hz in [1_000.0, 3_300.0] {
        let g = booster(hz);
        println!("{hz} Hz: {g:+.1} dB");
        assert!((33.5..37.5).contains(&g), "{hz} Hz: {g:+.1} dB");
    }
}

/// And it cuts the bass *before* anything clips -- far more than
/// ElectroSmash's 33 Hz corner says. That corner is C3 against R6 alone; R7's
/// shunt feedback divided by the stage's gain puts about 3.5 k at the base,
/// and C3 against that is near a kilohertz. Boss's own output check (a
/// 200 Hz square wave, service notes p. 6) shows the output sagging back in
/// every half cycle, which a 33 Hz corner could not do.
#[test]
fn the_booster_is_a_high_pass_near_a_kilohertz() {
    let (low, mid, high) = (booster(100.0), booster(300.0), booster(3_300.0));
    println!("100 Hz {low:+.1}, 300 Hz {mid:+.1}, 3.3 kHz {high:+.1}");
    assert!(high - low > 12.0, "only {:.1} dB of bass cut", high - low);
    assert!(high - mid > 4.0, "{:.1} dB down at 300 Hz", high - mid);
    // First order: 100 Hz is about 9 dB under 300 Hz, not 18.
    assert!((6.0..12.0).contains(&(mid - low)), "{:.1}", mid - low);
}

/// The gain stage: 1 + 100 k / 4.7 k, 26.9 dB, with Dist up and unity with it
/// down (ElectroSmash's figure is 26.5). The pot is linear, as Boss's
/// "100KB" is, so noon is only about 6 dB.
#[test]
fn the_dist_control_runs_from_unity_to_27_db() {
    let stage =
        |dist: f64| gain_to("amp", dist, 0.5, 1_000.0) - gain_to("plus", dist, 0.5, 1_000.0);
    let (down, mid, up) = (stage(0.0), stage(0.5), stage(1.0));
    println!("dist 0 {down:+.1}, 0.5 {mid:+.1}, 1 {up:+.1}");
    assert!(down.abs() < 0.3, "{down:+.1} dB with Dist down");
    assert!((25.5..28.0).contains(&up), "{up:+.1} dB with Dist up");
    assert!((4.5..7.0).contains(&mid), "{mid:+.1} dB at noon");
}

/// The DS-1's C8 is 1 uF (the DS-1A's is .47): with Dist up the gain stage's
/// low corner is 1 / (2 pi R13 C8), 34 Hz, so 100 Hz is nearly full gain.
#[test]
fn the_gain_stage_low_corner_is_the_ta7136_versions() {
    let stage = |hz: f64| gain_to("amp", 1.0, 0.5, hz) - gain_to("plus", 1.0, 0.5, hz);
    let (at100, at1k) = (stage(100.0), stage(1_000.0));
    println!("100 Hz {at100:+.1}, 1 kHz {at1k:+.1}");
    assert!(at1k - at100 < 1.5, "{:.1} dB down at 100 Hz", at1k - at100);
    assert!(at1k - stage(34.0) > 2.0, "no corner near 34 Hz");
}

/// The tone section is a blend with a scoop in the middle. At noon it has a
/// notch between 400 Hz and 1 kHz at least 3 dB under both ends of the
/// band; turned down it is a low-pass, turned up a high-pass.
#[test]
fn the_tone_control_is_a_scooped_blend() {
    let tone = |t: f64, hz: f64| gain_to("tone", 0.0, t, hz) - gain_to("clip", 0.0, t, hz);
    let freqs: Vec<f64> = (0..40).map(|i| 100.0 * 1.12f64.powi(i)).collect();
    let noon: Vec<f64> = freqs.iter().map(|&f| tone(0.5, f)).collect();
    let (notch_at, notch) =
        freqs.iter().zip(&noon).fold(
            (0.0, f64::MAX),
            |b, (f, d)| if *d < b.1 { (*f, *d) } else { b },
        );
    println!(
        "notch {notch:.1} dB at {notch_at:.0} Hz; ends {:.1}, {:.1}",
        noon[0], noon[39]
    );
    assert!(
        (400.0..1_000.0).contains(&notch_at),
        "notch at {notch_at:.0} Hz"
    );
    assert!(noon[0] - notch > 3.0 && noon[39] - notch > 3.0);
    assert!(
        tone(0.0, 100.0) - tone(0.0, 3_000.0) > 10.0,
        "Tone down is not dark"
    );
    assert!(
        tone(1.0, 3_000.0) - tone(1.0, 100.0) > 8.0,
        "Tone up is not bright"
    );
}

/// Two clippers in series. A guitar's level through the booster alone is
/// already squared off, and unevenly: a transistor saturates one way and
/// cuts off the other, which is where the second harmonic comes from.
#[test]
fn the_booster_clips_first_and_unevenly() {
    let mut s = Simulation::new(orange_dist::tap(10_000.0, 470_000.0, "c2").unwrap(), RATE);
    s.set_control(DIST, 0.0);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, 1_000.0, GUITAR_VOLTS);
    let m = measure::run(t, (RATE / 4.0) as usize, |x| s.process(x));
    println!(
        "THD {:.1} %, second {:.1} %, third {:.1} %",
        m.thd_percent(),
        m.harmonic_percent(2),
        m.harmonic_percent(3)
    );
    assert!(m.thd_percent() > 10.0, "the booster is not clipping");
    assert!(
        m.harmonic_percent(2) > 2.0,
        "the booster clips symmetrically"
    );
}

/// The Level control's resting position is unity through the pedal on a
/// guitar's low chord, with Dist and Tone centred.
#[test]
fn level_rest_is_unity() {
    const CAL: f64 = 48_000.0;
    let mut s = Simulation::new(orange_dist::build(10_000.0, 470_000.0).unwrap(), CAL);
    s.set_control(DIST, 0.5);
    s.set_control(TONE, 0.5);
    s.set_control(LEVEL, LEVEL_REST);
    s.find_operating_point();
    let x = |k: usize| {
        let t = k as f64 / CAL;
        GUITAR_VOLTS
            * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.50
                + (std::f64::consts::TAU * 123.5 * t).sin() * 0.30
                + (std::f64::consts::TAU * 246.9 * t).sin() * 0.20)
    };
    let warm = (CAL / 2.0) as usize;
    for k in 0..warm {
        s.process(x(k));
    }
    let (mut x2, mut y2) = (0.0, 0.0);
    for k in warm..warm + CAL as usize {
        let (a, b) = (x(k), s.process(x(k)));
        x2 += a * a;
        y2 += b * b;
    }
    let db = 10.0 * (y2 / x2).log10();
    println!("{db:+.2} dB");
    assert!(db.abs() < 0.5, "{db:+.2} dB at LEVEL_REST");
}

/// It plays at every rate the plugin runs at, driven hard.
#[test]
fn it_plays_at_every_rate() {
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut s = Simulation::new(orange_dist::build(10_000.0, 470_000.0).unwrap(), rate);
        s.set_control(DIST, 1.0);
        s.set_control(LEVEL, LEVEL_REST);
        s.find_operating_point();
        let mut peak = 0.0f64;
        for k in 0..(rate as usize / 5) {
            let x = 0.5 * (std::f64::consts::TAU * 110.0 * k as f64 / rate).sin();
            let y = s.process(x);
            assert!(y.is_finite(), "{rate}");
            peak = peak.max(y.abs());
        }
        println!("{rate}: peak {peak:.3} V");
        assert!((0.05..2.0).contains(&peak), "{rate}: {peak}");
    }
}

/// It is in both lists, under stable ids, with its three knobs where the
/// panel expects them.
#[test]
fn it_is_a_pedal_and_a_circuit() {
    use nih_plug::prelude::Enum;
    let pedal_ids = PedalModel::ids().unwrap();
    assert_eq!(
        pedal_ids[PedalModel::OrangeDist.to_index()],
        "pedal_boss_ds1"
    );
    assert_eq!(PedalModel::OrangeDist.voice(), Pedal::OrangeDist);
    assert_eq!(PedalModel::OrangeDist.name(), "Orange Dist");
    let circuit_ids = Circuit::ids().unwrap();
    assert_eq!(
        circuit_ids[Circuit::Ds1.to_index()],
        "pedal_boss_ds1_circuit"
    );
    assert_eq!(Circuit::Ds1.voice(), Gain::Ds1);
    // Appended to both lists, so nothing recorded against them moves.
    assert_eq!(PedalModel::ALL.last(), Some(&PedalModel::OrangeDist));
    assert_eq!(Circuit::ALL.last(), Some(&Circuit::Ds1));

    assert_eq!(
        Pedal::OrangeDist.tone_labels(),
        [Some("tone"), None, None, None]
    );
    assert_eq!(Gain::Ds1.drive_control(), DIST);
    assert!(matches!(
        Gain::Ds1.level_control(),
        Some(Level::Circuit(LEVEL))
    ));
    assert!(Gain::Ds1.single_tone());
    assert!(Gain::Ds1.is_modelled());
    assert!(Gain::Ds1.power_stage().is_none());
}
