//! The independent pedal slot in front of any circuit.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone, NOMINAL_DBFS};

fn guitar(k: usize) -> f64 {
    let a = 10f64.powf(NOMINAL_DBFS / 20.0);
    a * ((k as f64 * 0.0144).sin() * 0.7 + (k as f64 * 0.0432).sin() * 0.3)
}

fn render(settings: &Settings, n: usize) -> (Chain, Vec<f64>) {
    let mut chain = Chain::new(48_000.0);
    chain.apply(settings);
    chain.settle();
    chain.find_operating_point();
    let out = (0..n).map(|k| chain.process(guitar(k))).collect();
    (chain, out)
}

fn rms(x: &[f64]) -> f64 {
    (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
}

#[test]
fn no_pedal_is_exactly_the_old_path() {
    let base = Settings { gain: Gain::Twin, tone: Tone::Off, ..Settings::default() };
    let explicit = Settings {
        pedal: PedalSettings { pedal: Pedal::None, drive: 0.9, tone: 0.1, level: 0.9 },
        ..base
    };
    let (_, a) = render(&base, 8_000);
    let (_, b) = render(&explicit, 8_000);
    assert_eq!(a, b);
}

#[test]
fn a_pedal_is_really_in_front_of_the_amplifier() {
    let base = Settings { gain: Gain::Twin, drive: 0.35, tone: Tone::Off, ..Settings::default() };
    let boosted = Settings {
        pedal: PedalSettings { pedal: Pedal::Green808, drive: 0.2, tone: 0.5, level: 0.9 },
        ..base
    };
    let (_, clean) = render(&base, 24_000);
    let (mut chain, pushed) = render(&boosted, 24_000);
    let difference = rms(&clean.iter().zip(&pushed).map(|(a, b)| a - b).collect::<Vec<_>>());
    assert!(difference > 0.05 * rms(&clean), "{difference}");
    let before = chain.solver_breakdown();
    assert_no_heap(|| {
        for k in 0..2_048 {
            assert!(chain.process(guitar(k)).is_finite());
        }
    });
    assert!(chain.solver_breakdown().saturating_delta(before).pedal.solves > 0);
}

#[test]
fn the_same_circuit_can_be_both_pedal_and_voice() {
    let s = Settings {
        gain: Gain::Screamer,
        tone: Tone::Off,
        pedal: PedalSettings { pedal: Pedal::Green808, drive: 0.7, tone: 0.4, level: 0.6 },
        ..Settings::default()
    };
    let (chain, out) = render(&s, 12_000);
    assert!(out.iter().all(|y| y.is_finite()));
    let work = chain.solver_breakdown();
    assert!(work.pedal.solves > 0 && work.gain.solves > 0);
}

#[test]
fn pedal_level_turns_the_pedal_up_and_down() {
    let at = |level: f64| {
        let s = Settings {
            gain: Gain::Clean,
            tone: Tone::Off,
            drive: 0.1,
            pedal: PedalSettings { pedal: Pedal::BigMuff, drive: 0.5, tone: 0.5, level },
            ..Settings::default()
        };
        rms(&render(&s, 24_000).1[12_000..])
    };
    let (low, mid, high) = (at(0.15), at(0.5), at(0.95));
    assert!(low < mid && mid < high, "{low} {mid} {high}");
}

#[test]
fn a_pedal_survives_rates_blocks_and_a_stereo_wake() {
    let s = Settings {
        gain: Gain::Boogie,
        pedal: PedalSettings { pedal: Pedal::Green808, drive: 0.5, tone: 0.6, level: 0.7 },
        ..Settings::default()
    };
    let mut chain = Chain::new(44_100.0);
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        chain.set_rate(rate);
        assert_no_heap(|| {
            chain.apply(&s);
            chain.reset();
            chain.find_operating_point();
            for block in [1, 7, 64, 127, 512] {
                chain.apply(&s);
                for k in 0..block {
                    assert!(chain.process(guitar(k)).is_finite(), "{rate}");
                }
            }
        });
    }
    let mut left = Chain::new(48_000.0);
    let mut right = Chain::new(48_000.0);
    left.apply(&s);
    right.apply(&s);
    left.find_operating_point();
    right.find_operating_point();
    for k in 0..3_000 {
        left.process(guitar(k));
    }
    right.copy_runtime_state_from(&left);
    for k in 3_000..4_000 {
        assert_eq!(left.process(guitar(k)), right.process(guitar(k)));
    }
}

mod revisions {
    use gainstagefx::circuits::ts808::{self, Values};
    use gainstagefx::dsp::time::Simulation;
    use std::f64::consts::TAU;

    /// Steady-state peak of a small sine through the pedal, with its drive nearly
    /// shut so the clipper stays out of the way.
    fn level(values: &Values, tone: f64, hz: f64) -> f64 {
        let rate = 96_000.0;
        let mut sim = Simulation::new(ts808::build_with(values, 10_000.0, 470_000.0).unwrap(), rate);
        sim.set_control(ts808::DRIVE, 0.0);
        sim.set_control(ts808::TONE, tone);
        sim.find_operating_point();
        let n = (rate * 0.3) as usize;
        let mut peak: f64 = 0.0;
        for k in 0..n {
            let y = sim.process(0.01 * (TAU * hz * k as f64 / rate).sin());
            if k > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        peak
    }

    #[test]
    fn the_drawing_values_give_the_tone_control_its_published_reach() {
        let reach = |v: &Values| 20.0 * (level(v, 1.0, 6_000.0) / level(v, 0.0, 6_000.0)).log10();
        let verified = reach(&ts808::TS808);
        let legacy = reach(&ts808::LEGACY);
        // A 220 ohm shunt under 0.22 uF turns over near 3.2 kHz and swings the top
        // far harder than the 1 k the legacy netlist carries.
        assert!(verified > legacy + 3.0, "verified {verified:.1} dB vs legacy {legacy:.1} dB");
        assert!(verified > 8.0, "{verified}");
    }

    #[test]
    fn green_9_is_its_own_circuit_but_still_a_tube_screamer() {
        let a = level(&ts808::TS808, 0.5, 1_000.0);
        let b = level(&ts808::TS9, 0.5, 1_000.0);
        assert!(a != b);
        assert!((20.0 * (b / a).log10()).abs() < 1.5, "{a} vs {b}");
    }
}

/// Every pedal in the slot runs in front of an amplifier, at every rate, without
/// allocating, and moves the sound.
#[test]
fn every_pedal_runs_in_front_of_an_amplifier() {
    for pedal in [Pedal::Green808, Pedal::BigMuff, Pedal::Green9, Pedal::Rodent, Pedal::RoundFuzz] {
        let base = Settings { gain: Gain::Twin, drive: 0.35, tone: Tone::Off, ..Settings::default() };
        let with = Settings {
            pedal: PedalSettings { pedal, drive: 0.6, tone: 0.5, level: 0.6 },
            ..base
        };
        let (_, clean) = render(&base, 12_000);
        let (mut chain, pushed) = render(&with, 12_000);
        assert!(pushed.iter().all(|y| y.is_finite()), "{pedal:?}");
        let difference = rms(&clean.iter().zip(&pushed).map(|(a, b)| a - b).collect::<Vec<_>>());
        assert!(difference > 0.05 * rms(&clean), "{pedal:?}: {difference}");
        assert_no_heap(|| {
            for k in 0..1_024 {
                assert!(chain.process(guitar(k)).is_finite());
            }
        });
    }
}
