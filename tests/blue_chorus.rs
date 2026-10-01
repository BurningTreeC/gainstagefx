//! The Blue Chorus (Boss CE-2) against Boss's service notes, ElectroSmash's
//! analysis of the same drawing, and a measurement of a real one. See
//! `docs/models/blue_chorus.md`; `examples/blue_chorus_op.rs` prints what
//! these hold and fits the clock law.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::blue_chorus::{self as ce2, tap};
use gainstagefx::dsp::bbd::Brigade;
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::params::{Circuit, PedalModel};
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone as Stack, PEDAL_TONES};
use nice_plug::prelude::Enum;

const RATE: f64 = 48_000.0;

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

/// Gain at `hz` to `node`, with the bucket brigade's return held at rest --
/// the dry path alone at the output.
fn open_loop(node: &str, hz: f64) -> f64 {
    let mut s = Simulation::new(tap(10_000.0, 470_000.0, node).unwrap(), RATE);
    s.set_control(ce2::DEPTH, 0.0);
    s.find_operating_point();
    let t = Tone::near(RATE, 16_384, hz, 0.01);
    let settle = (RATE * if hz < 100.0 { 1.0 } else { 0.4 }) as usize;
    measure::run(t, settle, |x| s.process(x))
        .fundamental()
        .magnitude()
        / 0.01
}

/// The pre-emphasis ahead of the bucket brigade and the de-emphasis after it
/// are mirror images, so the dry signal, which takes both, comes out flat;
/// and the pre-emphasis itself is ElectroSmash's: unity in the bass, +15 dB
/// (x5.7) in the treble, with corners at 410 Hz and 2.34 kHz.
#[test]
fn the_emphasis_pair_is_flat_for_the_dry_signal() {
    let reference = open_loop(ce2::OUTPUT, 1_000.0);
    for hz in [100.0, 300.0, 3_000.0, 8_000.0] {
        let d = db(open_loop(ce2::OUTPUT, hz) / reference);
        println!("dry at {hz} Hz: {d:+.2} dB re 1 kHz");
        assert!(d.abs() < 1.0, "{hz}: {d}");
    }
    let lift = db(open_loop("a1", 10_000.0) / open_loop("a1", 100.0));
    println!("pre-emphasis: {lift:+.1} dB from 100 Hz to 10 kHz");
    assert!((lift - 15.0).abs() < 1.0, "{lift}");
}

/// The anti-aliasing filter into the bucket brigade: three poles round Q2,
/// which ElectroSmash puts at 6.6 and 6.9 kHz -- down a few decibels there and
/// steeply past it.
#[test]
fn the_brigade_is_fed_through_a_three_pole_filter() {
    let at = |hz: f64| db(open_loop(ce2::BBD_IN, hz) / open_loop("a1", hz));
    let (pass, corner, stop) = (at(1_000.0), at(6_800.0), at(15_000.0));
    println!(
        "anti-aliasing: {pass:+.1} dB at 1 kHz, {corner:+.1} at 6.8 kHz, {stop:+.1} at 15 kHz"
    );
    assert!(corner - pass < -2.0 && corner - pass > -9.0);
    assert!(stop - pass < -15.0);
}

/// The pedal with its bucket brigade in its loop, as the slot runs it.
struct Running {
    sim: Simulation,
    line: Brigade,
    send: usize,
    control: usize,
    tri: usize,
    returned: f64,
}

impl Running {
    fn new(rate: f64, depth: f64) -> Self {
        let c = tap(10_000.0, 470_000.0, ce2::OUTPUT).unwrap();
        let (send, control) = (
            c.unknown_named(ce2::BBD_IN).unwrap(),
            c.unknown_named(ce2::CLOCK_CONTROL).unwrap(),
        );
        let tri = c.unknown_named("tri").unwrap();
        let mut sim = Simulation::new(c, RATE);
        sim.set_control(ce2::RATE, rate);
        sim.set_control(ce2::DEPTH, depth);
        sim.find_operating_point();
        Self {
            sim,
            line: Brigade::new(ce2::LONGEST, RATE),
            send,
            control,
            tri,
            returned: 0.0,
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        self.sim.set_aux_input(ce2::BBD_RETURN, self.returned);
        let y = self.sim.process(x);
        let delay = ce2::delay_seconds(self.sim.voltage_at(self.control)) * RATE;
        self.returned = self
            .line
            .process(self.sim.voltage_at(self.send), delay, RATE);
        y
    }
}

/// The oscillator's rate across RATE: a triangle of 4 R32 C19 (33/47) / k,
/// 0.32 to 3.6 Hz from the values (ElectroSmash: "3.5 Hz" at the top).
#[test]
fn rate_runs_the_oscillator_from_a_third_of_a_hertz_to_three_and_a_half() {
    let rate = |knob: f64, seconds: f64| {
        let mut p = Running::new(knob, 0.5);
        let n = (RATE * seconds) as usize;
        let tri: Vec<f64> = (0..n)
            .map(|_| {
                p.process(0.0);
                p.sim.voltage_at(p.tri)
            })
            .collect();
        let tail = &tri[RATE as usize..];
        let rises: Vec<usize> = tail
            .windows(2)
            .enumerate()
            .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
            .map(|(i, _)| i)
            .collect();
        assert!(rises.len() >= 2, "rate {knob}: no oscillation");
        RATE * (rises.len() - 1) as f64 / (rises[rises.len() - 1] - rises[0]) as f64
    };
    let (slow, fast) = (rate(0.0, 8.0), rate(1.0, 3.0));
    println!("rate: {slow:.3} Hz down, {fast:.2} Hz up");
    assert!(slow > 0.25 && slow < 0.45, "{slow}");
    assert!(fast > 3.0 && fast < 4.2, "{fast}");
}

/// The delay the clock gives: about 110 kHz with DEPTH down, 4.65 ms through
/// 1024 stages, and a sweep of 28 kHz at the middle of DEPTH (a measurement
/// of a real one reads 100 to 128 kHz there). The measurement's two figures
/// put the sweep's middle at 114 kHz against 110 at rest, which no plausible
/// monotonic law fits; the linear one built keeps the rest and the span.
#[test]
fn the_delay_sits_where_the_measured_clock_puts_it() {
    let mut rest = Running::new(0.5, 0.0);
    for _ in 0..(RATE as usize / 2) {
        rest.process(0.0);
    }
    let at_rest = ce2::delay_seconds(rest.sim.voltage_at(rest.control));
    let mut swept = Running::new(1.0, 0.5);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for k in 0..(RATE as usize * 3) {
        swept.process(0.0);
        if k > RATE as usize {
            let d = ce2::delay_seconds(swept.sim.voltage_at(swept.control));
            lo = lo.min(d);
            hi = hi.max(d);
        }
    }
    println!(
        "delay: {:.2} ms at rest; {:.2} .. {:.2} ms at middle depth",
        at_rest * 1e3,
        lo * 1e3,
        hi * 1e3
    );
    assert!((at_rest * 1e3 - 4.65).abs() < 0.05);
    let span = 1024.0 / (2.0 * lo) - 1024.0 / (2.0 * hi);
    assert!((span - 28_000.0).abs() < 2_000.0, "{span}");
}

/// Dry and delayed at half and half: with DEPTH down the delay stands still,
/// and the pedal is a comb, its notches where the delay is an odd number of
/// half periods -- 107.5, 322.6, 537.6 Hz ... for 4.65 ms -- and its peaks
/// between. Not at the first: the wet signal is high-passed at 102 Hz on its
/// way into the mixer (C14 into R22's 47 k on IC1b's virtual earth), and by
/// 48 Hz and 15 Hz before that, so in the bass it is weaker than the dry and
/// turned in phase, and nothing cancels. Boss kept the chorus out of the
/// bass. From the second notch up the comb is there.
#[test]
fn the_dry_and_the_delayed_make_a_comb_above_the_bass() {
    let level = |hz: f64| {
        let mut p = Running::new(0.5, 0.0);
        let n = (RATE * 0.8) as usize;
        let (mut si, mut so) = (0.0, 0.0);
        for k in 0..n {
            let x = 0.05 * (std::f64::consts::TAU * hz * k as f64 / RATE).sin();
            let y = p.process(x);
            if k > n / 2 {
                si += x * x;
                so += y * y;
            }
        }
        10.0 * (so / si).log10()
    };
    let notch = [280.0, 300.0, 320.0, 340.0, 360.0]
        .into_iter()
        .map(|hz| (level(hz), hz))
        .fold((f64::MAX, 0.0), |a, b| if b.0 < a.0 { b } else { a });
    let peak = level(430.0);
    let bass = level(107.5);
    println!(
        "comb: {:+.1} dB at its notch near {:.0} Hz, {peak:+.1} at the peak at 430 Hz; {bass:+.1} at 107.5 Hz",
        notch.0, notch.1
    );
    assert!((300.0..=345.0).contains(&notch.1), "{}", notch.1);
    assert!(peak - notch.0 > 8.0, "{} {peak}", notch.0);
    assert!(bass > -3.0, "{bass}");
}

/// Registered under its ids, a pedal and a circuit, Rate on the drive and
/// Depth on the tone knob, no level.
#[test]
fn it_is_registered_as_a_pedal_and_a_circuit() {
    let pedals = PedalModel::ids().unwrap();
    assert_eq!(pedals[PedalModel::BlueChorus.to_index()], "pedal_boss_ce2");
    let circuits = Circuit::ids().unwrap();
    assert_eq!(
        circuits[Circuit::BlueChorus.to_index()],
        "pedal_boss_ce2_circuit"
    );
    assert_eq!(Pedal::BlueChorus.as_circuit(), Some(Gain::BlueChorus));
    assert!(Pedal::BlueChorus.has_drive() && !Pedal::BlueChorus.has_level());
    assert_eq!(Pedal::BlueChorus.tone_labels()[0], Some("depth"));
    assert_eq!(Gain::BlueChorus.drive_name(), "RATE");
    assert!(Gain::BlueChorus.bucket_brigade().is_some());
    assert!(Gain::BlueChorus.drive_is_channel_volume());
}

/// In the slot and as the circuit, at every rate and every oversampling the
/// slot can run it at, finite and moving and without allocating -- the delay
/// line included, which is sized for the deepest oversampling up front.
#[test]
fn it_is_realtime_safe_at_every_rate() {
    for as_pedal in [true, false] {
        let mut chain = Chain::new(44_100.0);
        for (rate, oversampling) in [
            (44_100.0, 1),
            (48_000.0, 2),
            (88_200.0, 1),
            (96_000.0, 4),
            (192_000.0, 8),
        ] {
            chain.set_rate(rate);
            let settings = if as_pedal {
                Settings {
                    gain: Gain::Clean,
                    tone: Stack::Off,
                    oversampling,
                    pedal: PedalSettings {
                        pedal: Pedal::BlueChorus,
                        drive: 1.0,
                        tone: [1.0; PEDAL_TONES],
                        level: 0.5,
                    },
                    ..Settings::default()
                }
            } else {
                Settings {
                    gain: Gain::BlueChorus,
                    drive: 1.0,
                    circuit_tone: 1.0,
                    tone: Stack::Off,
                    oversampling,
                    ..Settings::default()
                }
            };
            chain.apply(&settings);
            chain.settle();
            chain.reset();
            chain.find_operating_point();
            let mut peak: f64 = 0.0;
            assert_no_heap(|| {
                for k in 0..4_096 {
                    let x = 0.2 * (k as f64 * std::f64::consts::TAU * 330.0 / rate).sin();
                    let y = chain.process(x);
                    assert!(y.is_finite(), "{rate}");
                    peak = peak.max(y.abs());
                }
            });
            assert!(peak > 1e-3, "{rate} {as_pedal}: silent");
        }
    }
}
