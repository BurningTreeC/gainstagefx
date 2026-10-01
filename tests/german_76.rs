//! The German 76 (Telefunken V76) against the IRT's drawing S 1176 and the
//! acceptance sheet of its Braunbuch-Beschreibung (Blatt 5-8): every figure
//! on it, measured as the sheet says -- from a 200 ohm generator into 300 ohm.
//! See `docs/models/german_76.md`; `examples/german_76_op.rs` prints what
//! these hold and fits the three values the drawing leaves to the bench.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::german_76::{self as v76, tap, Fit};
use gainstagefx::dsp::complex::C;
use gainstagefx::dsp::measure::{self, Measured, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, Kind, Switch};
use gainstagefx::voice::{Chain, Gain, Settings, Throw, Tone as Stack};
use nice_plug::prelude::Enum;

const DB: [f64; 12] = [
    3.0, 9.0, 18.0, 24.0, 34.0, 40.0, 46.0, 52.0, 58.0, 64.0, 70.0, 76.0,
];
const FLAT: (usize, usize) = (1, 1);

fn position(n: usize) -> f64 {
    (n as f64 + 0.5) / 12.0
}

fn source_emf(level_db: f64) -> f64 {
    0.775 * 2f64.sqrt() * 10f64.powf(level_db / 20.0)
}

/// A tone at `level_db` (generator EMF, dB re 0.775 V) through switch
/// position `n` with the filters at `(low, mid)`, into `load`: what came out,
/// and the input terminals' and the generator's phasors.
fn run(
    n: usize,
    (low, mid): (usize, usize),
    hz: f64,
    level_db: f64,
    load: f64,
    rate: f64,
) -> (Measured, C, C) {
    let c = tap(200.0, load, "out").unwrap();
    let input = c.unknown_named("in").unwrap();
    let mut s = Simulation::new(c, rate);
    s.set_control(v76::GAIN, position(n));
    for (&slot, &value) in v76::LOW_CUT_SLOTS.iter().zip(&v76::LOW_CUT[low]) {
        s.set_value(slot, value);
    }
    s.set_value(v76::TREBLE_CUT_SLOTS[0], v76::TREBLE_CUT[mid][0]);
    s.find_operating_point();
    let window = 32_768.min((rate * 0.7) as usize);
    let t = Tone::near(rate, window, hz, source_emf(level_db));
    let settle = (rate * if hz < 100.0 { 2.0 } else { 0.5 }) as usize;
    let (mut xs, mut vs) = (Vec::new(), Vec::new());
    let m = measure::run(t, settle, |x| {
        let y = s.process(x);
        xs.push(x);
        vs.push(s.voltage_at(input));
        y
    });
    let phasor = |v: &[f64]| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &x) in v[v.len() - window..].iter().enumerate() {
            let a = std::f64::consts::TAU * t.bin * i as f64 / window as f64;
            re += x * a.cos();
            im -= x * a.sin();
        }
        C::new(re, im)
    };
    (m, phasor(&vs), phasor(&xs))
}

/// Response re `reference`, -64 dB in at 76 dB of gain (+12 out), as the
/// sheet measures it. Oversampled above 2 kHz, as the plugin runs it:
/// trapezoidal integration at 48 kHz bends the 15 kHz low-pass into the band.
fn response(sw: (usize, usize), hz: f64, reference: f64) -> f64 {
    let rate = if hz.max(reference) > 2_000.0 {
        192_000.0
    } else {
        48_000.0
    };
    let at = |f: f64| run(11, sw, f, -64.0, 300.0, rate).0.gain_db();
    at(hz) - at(reference)
}

/// Blatt 5, 4a: 76 to 34 dB in 6 dB steps, then 24, 18, 9 and 3 dB, each
/// within 0.5 dB, at 1 kHz and +6 dB out. Every one lands: the loop deck's
/// string sets the top eight, and the pads and 200 ohm termination, with the
/// loop held at the 34 or 40 dB position's, the bottom four.
///
/// The gain is referred to the generator, which is how the IRT measured it:
/// referred to the input terminals the padded positions miss -- position 4,
/// where 450 ohm in each leg drops less across the terminals than across
/// the transformer, would read over 24.5.
#[test]
fn the_gain_switch_gives_the_braunbuchs_twelve_gains() {
    for (n, &want) in DB.iter().enumerate() {
        let (m, terminal, emf) = run(n, FLAT, 1_000.0, 6.0 - want, 300.0, 96_000.0);
        let g = m.gain_db();
        println!("position {}: {g:.2} dB ({want})", n + 1);
        assert!((g - want).abs() < 0.5, "position {}: {g}", n + 1);
        if n == 3 {
            let at_terminals = g + 20.0 * (emf.magnitude() / terminal.magnitude()).log10();
            println!("  re the terminals: {at_terminals:.2}");
            assert!(at_terminals > 24.5, "{at_terminals}");
        }
    }
}

/// Blatt 6, 5a: flat from 60 Hz to 15 kHz within 0.5 dB, at 40 Hz +0.5 /
/// -1.5 dB, 15 Hz at least 12 dB down, and falling steadily above 15 kHz.
/// The bottom is the input transformer's high-pass, which the Braunbuch says
/// has no resonant rise; the top the 15 kHz low-pass, 25 / 84 / 26.
#[test]
fn the_response_is_the_sheets() {
    let at = |hz: f64| response(FLAT, hz, 1_000.0);
    let (lf15, lf40) = (at(15.0), at(40.0));
    println!("15 Hz {lf15:+.2}, 40 Hz {lf40:+.2}");
    assert!(lf15 <= -12.0, "{lf15}");
    assert!((-1.5..=0.5).contains(&lf40), "{lf40}");
    for hz in [60.0, 100.0, 300.0, 5_000.0, 10_000.0, 15_000.0] {
        let r = at(hz);
        println!("{hz:.0} Hz {r:+.2}");
        assert!(r.abs() <= 0.5, "{hz}: {r}");
    }
    let (top, past) = (at(20_000.0), at(30_000.0));
    println!("20 kHz {top:+.2}, 30 kHz {past:+.2}");
    assert!(top < -0.5 && past < top - 3.0);
}

/// Blatt 6, 5b: "80 Hz" at most 3 dB down at 80 Hz and at least 12 at 40;
/// "300 Hz" at most 4 at 300 and at least 14 at 40; both together at most 4
/// at 300 and at least 28 at 40. And Blatt 5, 4b: no switch position costs
/// more than 1.5 dB of gain.
#[test]
fn the_low_cut_meets_the_sheet() {
    let flat = run(11, FLAT, 1_000.0, -64.0, 300.0, 48_000.0).0.gain_db();
    for (name, low, at80_or_300, most, at40_least) in [
        ("80 Hz", 0, 80.0, 3.0, 12.0),
        ("300 Hz", 2, 300.0, 4.0, 14.0),
        ("80 + 300 Hz", 3, 300.0, 4.0, 28.0),
    ] {
        let corner = response((low, 1), at80_or_300, 1_000.0);
        let deep = response((low, 1), 40.0, 1_000.0);
        let loss = flat
            - run(11, (low, 1), 1_000.0, -64.0, 300.0, 48_000.0)
                .0
                .gain_db();
        println!("{name}: {at80_or_300:.0} Hz {corner:+.2}, 40 Hz {deep:+.2}, 1 kHz {loss:.2} below flat");
        assert!(corner >= -most, "{name}: {corner}");
        assert!(deep <= -at40_least, "{name}: {deep}");
        assert!(loss <= 1.5, "{name}: {loss}");
    }
}

/// Blatt 6, 5c: "3 kHz", re 500 Hz, at most 0.6 dB down at 1 kHz, 5-7 dB at
/// 10 kHz and 6-8 dB at 15 kHz -- 20 1 nF and 68 25 k off 66 / 67's junction.
#[test]
fn the_treble_cut_meets_the_sheet() {
    let at = |hz: f64| response((1, 0), hz, 500.0);
    let (k1, k10, k15) = (at(1_000.0), at(10_000.0), at(15_000.0));
    println!("1 kHz {k1:+.2}, 10 kHz {k10:+.2}, 15 kHz {k15:+.2}");
    assert!(k1 >= -0.6, "{k1}");
    assert!((-7.0..=-5.0).contains(&k10), "{k10}");
    assert!((-8.0..=-6.0).contains(&k15), "{k15}");
}

/// Blatt 5, 3: the input impedance at 34 dB, at least 600 ohm from 60 Hz to
/// 8 kHz and 300 ohm at 40 Hz and 15 kHz. And Blatt 8: the output impedance
/// at most 30 ohm at 1 kHz, 40 ohm from 40 Hz to 15 kHz.
#[test]
fn the_impedances_meet_the_sheet() {
    for (hz, least) in [
        (40.0, 300.0),
        (60.0, 600.0),
        (1_000.0, 600.0),
        (8_000.0, 600.0),
        (15_000.0, 300.0),
    ] {
        let rate = if hz < 100.0 { 48_000.0 } else { 192_000.0 };
        let (_, terminal, emf) = run(4, FLAT, hz, -38.0, 300.0, rate);
        let z = (C::new(200.0, 0.0) * terminal / (emf - terminal)).magnitude();
        println!("input {hz:.0} Hz: {z:.0} ohm (at least {least})");
        assert!(z >= least, "{hz}: {z}");
    }
    for (hz, most) in [(40.0, 40.0), (1_000.0, 30.0), (15_000.0, 40.0)] {
        let rate = if hz < 100.0 { 48_000.0 } else { 192_000.0 };
        let at = |load: f64| run(11, FLAT, hz, -76.0, load, rate).0.gain_db();
        let ratio = 10f64.powf((at(3_000.0) - at(300.0)) / 20.0);
        let z = (ratio - 1.0) / (1.0 / 300.0 - ratio / 3_000.0);
        println!("output {hz:.0} Hz: {z:.1} ohm (at most {most})");
        assert!(z <= most, "{hz}: {z}");
    }
}

/// Blatt 7: k2 and k3 at +12 and +22 dB out. The one figure that is loose --
/// 1.5 % k3 at 40 Hz from 34 dB of gain -- is the input transformer at its
/// largest flux, and its knee is set by `examples/german_76_op.rs` to give 1 %
/// there; every other figure is the valves and the output transformer, with
/// nothing set for them.
#[test]
fn the_distortion_meets_the_sheet() {
    for (n, out, hz, k2_most, k3_most) in [
        (11, 12.0, 40.0, 0.2, 0.2),
        (11, 12.0, 1_000.0, 0.1, 0.1),
        (11, 12.0, 5_000.0, 0.1, 0.1),
        (11, 22.0, 40.0, 0.3, 0.2),
        (11, 22.0, 1_000.0, 0.2, 0.2),
        (4, 22.0, 40.0, 1.0, 1.5),
        (4, 22.0, 1_000.0, 0.2, 0.2),
        (4, 22.0, 5_000.0, 0.2, 0.2),
    ] {
        let rate = if hz < 100.0 { 48_000.0 } else { 96_000.0 };
        let m = run(n, FLAT, hz, out - DB[n], 300.0, rate).0;
        let (k2, k3) = (m.harmonic_percent(2), m.harmonic_percent(3));
        println!(
            "{} dB, {out:+} dB out, {hz} Hz: k2 {k2:.3} % k3 {k3:.3} % ({k2_most} / {k3_most})",
            DB[n]
        );
        assert!(k2 <= k2_most && k3 <= k3_most, "{n} {out} {hz}: {k2} {k3}");
    }
}

/// The drawing's voltages. The supply, the cathodes and V2 and V4 land on
/// them; V1's and V3's plates, which sit below their screens, run low -- 43
/// against 67 V and 70 against 80: below the screen voltage the Koren
/// pentode gives the plate current the real valve gives its screen
/// (APPROXIMATED in the log).
#[test]
fn it_runs_where_the_drawing_does() {
    let c = tap(200.0, 300.0, "out").unwrap();
    let names = [
        ("b0", 300.0, 2.0),
        ("b2", 279.0, 3.0),
        ("b1", 242.0, 5.0),
        ("k1", 11.0, 1.5),
        ("k2", 2.3, 0.2),
        ("s2", 165.0, 8.0),
        ("p2", 175.0, 10.0),
        ("k3", 16.0, 1.0),
        ("k4", 2.2, 0.2),
        ("s4", 140.0, 10.0),
        ("p4", 170.0, 10.0),
        ("p1", 67.0, 30.0),
        ("p3", 80.0, 15.0),
    ];
    let idx: Vec<usize> = names
        .iter()
        .map(|n| c.unknown_named(n.0).unwrap())
        .collect();
    let mut s = Simulation::new(c, 48_000.0);
    s.set_control(v76::GAIN, position(11));
    s.find_operating_point();
    for ((name, want, within), &i) in names.iter().zip(&idx) {
        let v = s.voltage_at(i);
        println!("{name}: {v:.2} V ({want})");
        assert!((v - want).abs() <= *within, "{name}: {v}");
    }
}

/// The values the drawing leaves to the bench stay where the bench would put
/// them: 75, drawn 80 k "abgeglichen", within 15 % of it.
#[test]
fn the_trim_is_near_the_drawn_value() {
    let r75 = Fit::SET.r75;
    assert!((r75 / 80_000.0 - 1.0).abs() < 0.15, "{r75}");
}

/// Registered as a microphone amplifier, with its gain switch on the Drive
/// knob and its two filter switches on the panel's switch selectors, flat in
/// the second position of each, as every circuit's default is.
#[test]
fn it_is_registered_with_its_switches() {
    let ids = Circuit::ids().unwrap();
    assert_eq!(ids[Circuit::German76.to_index()], "pre_telefunken_v76");
    assert_eq!(Circuit::German76.name(), "German 76");
    let gain = Circuit::German76.voice();
    assert_eq!(gain, Gain::German76);
    assert_eq!(Circuit::German76.kind(), Kind::MicPre);
    assert!(gain.is_modelled() && gain.power_stage().is_none());
    assert_eq!(gain.drive_name(), "GAIN");
    let low = gain.low_switch().unwrap();
    assert_eq!(low.labels, ["80 Hz", "Flat", "300 Hz", "80+300"]);
    assert_eq!(low.at(Throw::Centre), &v76::LOW_CUT[1]);
    let mid = gain.mid_switch().unwrap();
    assert_eq!(mid.labels, ["3 kHz", "Flat"]);
    // A two-position switch thrown past its end is at its last.
    assert_eq!(mid.at(Throw::Far), &v76::TREBLE_CUT[1]);
    assert_eq!(Switch::ALL.len(), 4);
    assert_eq!(Switch::Far.voice(), Throw::Far);
}

/// The panel's switches reach the circuit through `apply` without
/// allocating, and do what they are named: the low cut's last position takes
/// most of 60 Hz away, "3 kHz" some of 12 kHz.
#[test]
fn the_switches_are_thrown_from_the_panel_without_allocating() {
    let rate = 48_000.0;
    let rms = |chain: &mut Chain, hz: f64| {
        let mut sum = 0.0;
        for k in 0..24_000 {
            let x = 0.01 * (k as f64 * std::f64::consts::TAU * hz / rate).sin();
            let y = chain.process(x);
            if k > 12_000 {
                sum += y * y;
            }
        }
        sum.sqrt()
    };
    let mut chain = Chain::new(rate);
    let base = Settings {
        gain: Gain::German76,
        drive: 0.5,
        tone: Stack::Off,
        oversampling: 1,
        ..Settings::default()
    };
    chain.apply(&base);
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    let (low_flat, high_flat) = (rms(&mut chain, 60.0), rms(&mut chain, 12_000.0));
    let thrown = Settings {
        low_switch: Throw::Far,
        mid_switch: Throw::Left,
        ..base
    };
    let (mut low_cut, mut high_cut) = (0.0, 0.0);
    assert_no_heap(|| {
        chain.apply(&thrown);
        low_cut = rms(&mut chain, 60.0);
        high_cut = rms(&mut chain, 12_000.0);
        chain.apply(&base);
    });
    println!("60 Hz {low_flat:.4} -> {low_cut:.4}; 12 kHz {high_flat:.4} -> {high_cut:.4}");
    assert!(low_cut < 0.2 * low_flat, "{low_flat} {low_cut}");
    assert!(high_cut < 0.75 * high_flat, "{high_flat} {high_cut}");
}

/// Through the chain the make-up holds every position at one level, right
/// up to each position's edges: a gain switch's make-up is its position's
/// own, where a line between two measured points either side of a step was
/// off by up to the whole step (the 46 dB position read 3.5 dB loud at the
/// knob's middle). The British 47's three positions with it.
#[test]
fn every_gain_position_plays_at_one_level() {
    let rate = 48_000.0;
    for (gain, steps) in [(Gain::German76, 12), (Gain::British47, 3)] {
        let mut levels = Vec::new();
        for k in 0..steps {
            let width = 1.0 / steps as f64;
            for drive in [
                k as f64 * width + 0.01,
                (k as f64 + 0.5) * width,
                (k + 1) as f64 * width - 0.01,
            ] {
                let mut chain = Chain::new(rate);
                chain.apply(&Settings {
                    gain,
                    drive,
                    tone: Stack::Off,
                    oversampling: 1,
                    ..Settings::default()
                });
                chain.settle();
                let mut sum = 0.0;
                for i in 0..9_600 {
                    let x = 0.01 * (i as f64 * std::f64::consts::TAU * 1_000.0 / rate).sin();
                    let y = chain.process(x);
                    if i >= 4_800 {
                        sum += y * y;
                    }
                }
                levels.push((drive, 10.0 * (sum / 4_800.0).log10()));
            }
        }
        let (lo, hi) = levels
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), &(_, l)| {
                (lo.min(l), hi.max(l))
            });
        println!(
            "{gain:?}: {:.2} dB spread over {} settings",
            hi - lo,
            levels.len()
        );
        assert!(hi - lo < 1.0, "{gain:?}: {levels:?}");
    }
}

/// At every rate the plugin runs at, at the top of its gain, finite and
/// without allocating.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            gain: Gain::German76,
            drive: 0.99,
            tone: Stack::Off,
            oversampling: 1,
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.5 * (k as f64 * std::f64::consts::TAU * 440.0 / rate).sin();
                assert!(chain.process(x).is_finite(), "{rate}");
            }
        });
    }
}
