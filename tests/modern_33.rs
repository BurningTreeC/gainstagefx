//! The Modern 33 (Fortin 33) against PedalPCB's trace of a genuine unit and
//! Fortin's figure for it. See `docs/models/modern_33.md`;
//! `examples/modern_33_op.rs` prints what these hold.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::modern_33::{self as m33, tap};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, Pedal, PedalSettings, Settings, Tone as Stack};

const RATE: f64 = 48_000.0;

/// The pedal's gain at `hz`, dB, LEVEL full, and its third harmonic, %.
fn gain(hz: f64, amplitude: f64, rate: f64) -> (f64, f64) {
    let mut s = Simulation::new(tap(10_000.0, 1_000_000.0, m33::OUTPUT).unwrap(), rate);
    s.set_control(m33::LEVEL, 1.0);
    s.find_operating_point();
    let t = Tone::near(rate, 8_192, hz, amplitude);
    let settle = (rate * if hz < 100.0 { 1.0 } else { 0.3 }) as usize;
    let m = measure::run(t, settle, |x| s.process(x));
    (
        20.0 * (m.fundamental().magnitude() / amplitude).log10(),
        m.harmonic_percent(3),
    )
}

/// Fortin: "+22 dB of clean gain". The trace's values give it at 1 kHz, not
/// fitted -- and give it nowhere near the low strings: the low E is some
/// 20 dB below, the cut that makes the boost "tight" in front of a
/// distorting amplifier.
#[test]
fn it_boosts_22_db_in_the_middle_and_holds_the_lows_back() {
    let (mid, _) = gain(1_000.0, 0.05, RATE);
    let (low_e, _) = gain(82.0, 0.05, RATE);
    let (treble, _) = gain(5_000.0, 0.05, RATE);
    println!("{mid:+.1} dB at 1 kHz, {low_e:+.1} at 82 Hz, {treble:+.1} at 5 kHz");
    assert!((mid - 22.0).abs() < 1.5, "{mid}");
    assert!(mid - low_e > 15.0, "{low_e}");
    assert!((treble - mid).abs() < 4.0, "{treble}");
}

/// The 33 V rail is what the charge pump is for: a guitar's level is far
/// below where the stage runs out of swing, so the boost is clean -- its
/// third harmonic a hundredth of a per cent at half a volt in -- and only
/// near a volt does it begin to clip.
#[test]
fn it_is_clean_far_past_a_guitars_level() {
    let (_, at_half) = gain(1_000.0, 0.5, RATE);
    let (_, at_two) = gain(1_000.0, 2.0, RATE);
    println!("third harmonic: {at_half:.3} % at 0.5 V, {at_two:.1} % at 2 V");
    assert!(at_half < 0.05, "{at_half}");
    assert!(at_two > 3.0, "{at_two}");
}

/// The bias the TC Integrated Preamp's DC analysis explains: the reference
/// near the top of the rail (820 k of 930 k), the collector held there by
/// the op-amp, and the op-amp's output well below it, near the middle of its
/// swing -- the headroom that the analysis says "correcting" it would lose.
#[test]
fn it_is_biased_as_the_tc_analysis_says() {
    let c = tap(10_000.0, 1_000_000.0, m33::OUTPUT).unwrap();
    let (vref, o1) = (
        c.unknown_named("vref").unwrap(),
        c.unknown_named("o1").unwrap(),
    );
    let mut s = Simulation::new(c, RATE);
    s.find_operating_point();
    let (vref, o1) = (s.voltage_at(vref), s.voltage_at(o1));
    println!("VREF {vref:.2} V, op-amp output {o1:.2} V");
    assert!((vref - m33::RAIL * 820.0 / 930.0).abs() < 0.3, "{vref}");
    assert!((14.0..24.0).contains(&o1), "{o1}");
}

/// The same pedal at every rate the solver runs at.
#[test]
fn it_is_the_same_at_every_rate() {
    let (at48, _) = gain(1_000.0, 0.05, 48_000.0);
    for rate in [44_100.0, 88_200.0, 96_000.0, 192_000.0] {
        let (g, _) = gain(1_000.0, 0.05, rate);
        assert!((g - at48).abs() < 0.3, "{rate}: {g} against {at48}");
    }
}

/// In the slot, ahead of a circuit, at every rate and oversampling, finite
/// and without allocating; its drive knob does nothing, its level does.
#[test]
fn in_the_slot_it_is_realtime_safe_and_its_level_is_its_knob() {
    let mut chain = Chain::new(44_100.0);
    for (rate, oversampling) in [
        (44_100.0, 1),
        (48_000.0, 2),
        (88_200.0, 1),
        (96_000.0, 2),
        (192_000.0, 1),
    ] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            pedal: PedalSettings::centred(Pedal::Modern33, 0.5, 0.8),
            gain: Gain::Crunch,
            drive: 0.5,
            tone: Stack::Off,
            oversampling,
            ..Settings::default()
        });
        chain.settle();
        chain.reset();
        chain.find_operating_point();
        let mut peak: f64 = 0.0;
        assert_no_heap(|| {
            for k in 0..4_096 {
                let x = 0.1 * (k as f64 * std::f64::consts::TAU * 440.0 / rate).sin();
                let y = chain.process(x);
                assert!(y.is_finite(), "{rate}");
                peak = peak.max(y.abs());
            }
        });
        assert!(peak > 1e-4, "{rate}: silent");
    }
    assert!(!Pedal::Modern33.has_drive());
    assert!(Pedal::Modern33.has_level());
}
