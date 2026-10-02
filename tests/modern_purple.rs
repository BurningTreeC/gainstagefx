//! The Modern Purple (Revv G3, the original) against PedalPCB's trace of a
//! genuine unit: each stage's gain derived from the drawn values, what the
//! Aggression toggle does, the clippers and the tone controls. See
//! `docs/models/modern_purple.md`; `examples/modern_purple_op.rs` prints what
//! these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::modern_purple::{self as g3, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone as Stack, PEDAL_TONES};

const RATE: f64 = 48_000.0;
const SMALL: f64 = 0.0005;

fn sim(node: &str, controls: &[(usize, f64)], rate: f64) -> Simulation {
    let mut s = Simulation::new(tap(10_000.0, 470_000.0, node).unwrap(), rate);
    for &(c, v) in controls {
        s.set_control(c, v);
    }
    s.find_operating_point();
    s
}

/// The gain to `node` at `hz`, as a ratio.
fn gain(node: &str, controls: &[(usize, f64)], hz: f64, amplitude: f64, rate: f64) -> f64 {
    let mut s = sim(node, controls, rate);
    let t = Tone::near(rate, 8_192, hz, amplitude);
    let m = measure::run(t, (rate * 0.25) as usize, |x| s.process(x));
    m.fundamental().magnitude() / amplitude
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

/// The first stage is R7 over R6, x2.74; the gain stage at full GAIN is one
/// plus R17 and the 1 M track (with C11 100 pF across them) over its leg:
/// R16 33 k alone with Aggression off, R16 with R14 33 k (Blue), R16 with
/// R15 10 k (Red), C10 220 nF in series. Derived here from the values, at
/// 1 kHz, and held to 5 %.
#[test]
fn each_stage_has_the_gain_its_values_give() {
    let first = gain("o22", &[], 1_000.0, SMALL, RATE);
    println!("first stage: x{first:.2} (1 + 82/47 = 2.74)");
    assert!((first / 2.745 - 1.0).abs() < 0.05, "{first}");

    let w = std::f64::consts::TAU * 1_000.0;
    let feedback = {
        // 1.056 M in parallel with 100 pF.
        let r: f64 = 56_000.0 + 1_000_000.0;
        let x = 1.0 / (w * 100e-12);
        (r * x * x / (r * r + x * x), -r * r * x / (r * r + x * x))
    };
    for (name, aggression, leg) in [
        ("off", 0.1, 33_000.0),
        ("Blue", 0.5, 33_000.0 * 33_000.0 / 66_000.0),
        ("Red", 0.9, 33_000.0 * 10_000.0 / 43_000.0),
    ] {
        let leg_x = -1.0 / (w * 220e-9);
        // 1 + Zf / Zleg, complex.
        let (zr, zi) = feedback;
        let d = leg * leg + leg_x * leg_x;
        let (qr, qi) = ((zr * leg + zi * leg_x) / d, (zi * leg - zr * leg_x) / d);
        let expected = ((1.0 + qr).powi(2) + qi * qi).sqrt();
        let c = [(g3::GAIN, 1.0), (g3::AGGRESSION, aggression)];
        let measured =
            gain("o21", &c, 1_000.0, SMALL, RATE) / gain("p21", &c, 1_000.0, SMALL, RATE);
        println!("gain stage, Aggression {name}: x{measured:.1} (x{expected:.1} from the values)");
        assert!(
            (measured / expected - 1.0).abs() < 0.05,
            "{name}: {measured} against {expected}"
        );
    }
}

/// Aggression's other half: engaged either way it puts C7 beside C6, so the
/// low cut into the gain stage (C6 into R10 33 k, 2.2 kHz) halves to 1.1
/// kHz: more of the low strings reach the gain. Off, 200 Hz is some 4 dB
/// further down against 3 kHz than engaged.
#[test]
fn aggression_lets_more_bass_into_the_gain_stage() {
    let tilt = |a: f64| {
        let c = [(g3::AGGRESSION, a)];
        db(gain("p21", &c, 200.0, SMALL, RATE)) - db(gain("p21", &c, 3_000.0, SMALL, RATE))
    };
    let (off, blue, red) = (tilt(0.1), tilt(0.5), tilt(0.9));
    println!(
        "200 Hz against 3 kHz into the gain stage: off {off:+.1}, Blue {blue:+.1}, Red {red:+.1} dB"
    );
    assert!(blue - off > 3.0, "{off} {blue}");
    assert!((blue - red).abs() < 0.2, "{blue} {red}");
}

/// The clipping is fixed: the 1N4148 pairs in IC2.4's loop hold its output
/// near two diode drops, and the red LEDs to ground hold the next node near
/// their knee -- whatever the Aggression toggle, which changes the gain into
/// them. At a guitar's level and GAIN at noon the LED node is pinned.
#[test]
fn the_clippers_hold_the_signal() {
    for a in [0.1, 0.5, 0.9] {
        let mut s = sim("clip", &[(g3::AGGRESSION, a)], RATE);
        let mut peak: f64 = 0.0;
        let n = (RATE * 0.3) as usize;
        for k in 0..n {
            let y = s.process(0.122 * (std::f64::consts::TAU * 220.0 * k as f64 / RATE).sin());
            if k > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        println!("Aggression {a}: the LED node peaks at {peak:.2} V");
        assert!((1.2..2.2).contains(&peak), "{a}: {peak}");
    }
}

/// The tone controls run the way the box's do: BASS up lifts 100 Hz, TREBLE
/// up lifts 5 kHz, MID up lifts 700 Hz, each at the output stage.
#[test]
fn the_tone_controls_run_the_right_way() {
    for (name, control, hz) in [
        ("BASS", g3::BASS, 100.0),
        ("TREBLE", g3::TREBLE, 5_000.0),
        ("MID", g3::MID, 700.0),
    ] {
        let lo = db(gain("o13", &[(control, 0.0)], hz, SMALL, RATE));
        let hi = db(gain("o13", &[(control, 1.0)], hz, SMALL, RATE));
        println!("{name}: {lo:+.1} -> {hi:+.1} dB at {hz} Hz");
        assert!(hi - lo > 6.0, "{name}: {lo} {hi}");
    }
}

/// The same pedal at every rate the solver runs at.
#[test]
fn it_is_the_same_at_every_rate() {
    let at48 = db(gain("o13", &[], 1_000.0, SMALL, 48_000.0));
    for rate in [44_100.0, 88_200.0, 96_000.0, 192_000.0] {
        let g = db(gain("o13", &[], 1_000.0, SMALL, rate));
        assert!((g - at48).abs() < 0.3, "{rate}: {g} against {at48}");
    }
}

/// In the slot, ahead of an amplifier, at every rate, finite and without
/// allocating; it holds the chain at the host rate (it is one of the
/// expensive pedals), and its four tone knobs are its stack and the toggle.
#[test]
fn in_the_slot_it_is_realtime_safe() {
    assert!(Pedal::ModernPurple.is_expensive());
    let labels = Pedal::ModernPurple.tone_labels();
    assert_eq!(
        labels[..4],
        [Some("bass"), Some("mid"), Some("treble"), Some("aggr.")]
    );
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            pedal: PedalSettings {
                pedal: Pedal::ModernPurple,
                drive: 0.7,
                tone: [0.5; PEDAL_TONES],
                level: 0.5,
            },
            gain: Gain::Clean,
            drive: 0.3,
            tone: Stack::Off,
            oversampling: 2,
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let mut peak: f64 = 0.0;
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.1 * (k as f64 * std::f64::consts::TAU * 110.0 / rate).sin();
                let y = chain.process(x);
                assert!(y.is_finite(), "{rate}");
                peak = peak.max(y.abs());
            }
        });
        assert!(peak > 1e-3, "{rate}: silent");
    }
}
