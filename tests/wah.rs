//! The Black Wah and the Chrome Wah (Dunlop Cry Baby GCB-95, Vox V847)
//! against ElectroSmash's analyses of both. See `docs/models/wahs.md`;
//! `examples/wah_op.rs` prints the sweep these check.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::circuits::wah::{self, tap, Build};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{Chain, Gain, Settings, Tone as Stack, WahSettings};

const RATE: f64 = 48_000.0;

/// The wah's gain at `hz`, dB, with the treadle at `treadle` and a source of
/// `source` ohms.
fn gain(build: Build, source: f64, treadle: f64, hz: f64, rate: f64) -> f64 {
    let mut s = Simulation::new(tap(build, source, 470_000.0, wah::OUTPUT).unwrap(), rate);
    let (top, bottom) = wah::treadle_halves(treadle);
    s.set_realtime_value(wah::TREADLE_TOP, top);
    s.set_realtime_value(wah::TREADLE_BOTTOM, bottom);
    s.find_operating_point();
    let t = Tone::near(rate, 8_192, hz, 0.01);
    let m = measure::run(t, (rate * 0.12) as usize, |x| s.process(x));
    20.0 * (m.fundamental().magnitude() / 0.01).log10()
}

/// The resonant peak, by a coarse sweep refined around its best point:
/// (hertz, dB).
fn peak(build: Build, treadle: f64, rate: f64) -> (f64, f64) {
    let search = |from: f64, to: f64, step: f64| {
        let mut best = (from, f64::MIN);
        let mut hz = from;
        while hz <= to {
            let g = gain(build, 10_000.0, treadle, hz, rate);
            if g > best.1 {
                best = (hz, g);
            }
            hz *= step;
        }
        best
    };
    let (coarse, _) = search(300.0, 2_600.0, 1.1);
    search(coarse / 1.1, coarse * 1.1, 1.015)
}

/// ElectroSmash's figures for the Cry Baby: the peak at about 450 Hz heel
/// down, 750 Hz in the middle and 1.6 kHz toe down -- what the treadle's law
/// was fitted to, so this holds the fit -- and the lift at the peak roughly
/// constant across the sweep ("the treadle moves the peak, not the level"),
/// some 18 dB.
#[test]
fn the_cry_baby_sweeps_where_electrosmash_says() {
    let mut lifts = Vec::new();
    for (treadle, expected) in [(0.0, 450.0), (0.5, 750.0), (1.0, 1_600.0)] {
        let (hz, db) = peak(Build::CryBaby, treadle, RATE);
        println!("treadle {treadle}: peak {hz:.0} Hz at {db:+.1} dB");
        assert!(
            (hz / expected - 1.0).abs() < 0.08,
            "{treadle}: {hz} against {expected}"
        );
        assert!((15.0..24.0).contains(&db), "{treadle}: {db}");
        lifts.push(db);
    }
    let spread = lifts.iter().cloned().fold(f64::MIN, f64::max)
        - lifts.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread < 4.0, "{lifts:?}");
}

/// The Vox, built from the same values but R4 510 and R8 100 k: the same
/// sweep within a few per cent, and a little less gain at the peak, its
/// emitter resistor being larger (ElectroSmash's 18 dB against the Cry
/// Baby's 19 to 21 here).
#[test]
fn the_vox_sweeps_the_same_band_with_less_gain() {
    for treadle in [0.0, 0.5, 1.0] {
        let (vox, vox_db) = peak(Build::V847, treadle, RATE);
        let (cry, cry_db) = peak(Build::CryBaby, treadle, RATE);
        println!("treadle {treadle}: Vox {vox:.0} Hz {vox_db:+.1} dB, Cry Baby {cry:.0} Hz {cry_db:+.1} dB");
        assert!(
            (vox / cry - 1.0).abs() < 0.12,
            "{treadle}: {vox} against {cry}"
        );
        assert!(vox_db < cry_db, "{treadle}: {vox_db} against {cry_db}");
    }
}

/// The buffer is the difference a guitar hears. ElectroSmash gives the Vox's
/// input as 69.5 k -- R1 and R2, the transistor's base being held low by the
/// loop -- and the buffered Cry Baby's as about 750 k: from a 100 k source
/// against a 1 k one the Vox loses 20 log(69.5 / 169.5) = -7.7 dB, the Cry
/// Baby under a decibel. At 3 kHz with the heel down, where C1's reactance is
/// small and the peak is far away.
#[test]
fn the_buffer_is_what_keeps_a_pickup_from_being_loaded() {
    let lost = |build| {
        gain(build, 100_000.0, 0.0, 3_000.0, RATE) - gain(build, 1_000.0, 0.0, 3_000.0, RATE)
    };
    let (vox, cry) = (lost(Build::V847), lost(Build::CryBaby));
    println!("a 100 k source costs the Vox {vox:+.1} dB, the Cry Baby {cry:+.1}");
    assert!((vox + 7.7).abs() < 1.0, "{vox}");
    assert!(cry > -1.0, "{cry}");
}

/// The same circuit at every rate the solver runs at: the middle of the
/// sweep within a few per cent of where it is at 48 kHz.
#[test]
fn the_sweep_holds_at_every_rate() {
    let (at48, _) = peak(Build::CryBaby, 0.5, 48_000.0);
    for rate in [44_100.0, 88_200.0, 96_000.0, 192_000.0] {
        let (hz, db) = peak(Build::CryBaby, 0.5, rate);
        println!("{rate}: {hz:.0} Hz at {db:+.1} dB");
        assert!(
            (hz / at48 - 1.0).abs() < 0.04,
            "{rate}: {hz} against {at48}"
        );
    }
}

/// A chain with the wah ahead of a clean circuit, a tone at `hz`: its level.
fn through_chain(chain: &mut Chain, wah: WahSettings, amplitude: f64, hz: f64, rate: f64) -> f64 {
    chain.apply(&Settings {
        wah,
        gain: Gain::Clean,
        drive: 0.2,
        tone: Stack::Off,
        oversampling: 1,
        ..Settings::default()
    });
    chain.settle();
    chain.reset();
    chain.find_operating_point();
    let n = (rate * 0.4) as usize;
    let mut sum = 0.0;
    for k in 0..n {
        let y = chain.process(amplitude * (std::f64::consts::TAU * hz * k as f64 / rate).sin());
        if k > n / 2 {
            sum += y * y;
        }
    }
    10.0 * (sum / (n - n / 2 - 1) as f64).log10()
}

/// In the chain the treadle is the parameter: toe down, 1.6 kHz comes up
/// against heel down, and 450 Hz the other way.
#[test]
fn the_treadle_moves_the_peak_in_the_chain() {
    let mut chain = Chain::new(RATE);
    let at = |treadle| WahSettings {
        wah: Some(Build::CryBaby),
        treadle,
        ..WahSettings::default()
    };
    let high = through_chain(&mut chain, at(1.0), 0.05, 1_600.0, RATE)
        - through_chain(&mut chain, at(0.0), 0.05, 1_600.0, RATE);
    let low = through_chain(&mut chain, at(0.0), 0.05, 450.0, RATE)
        - through_chain(&mut chain, at(1.0), 0.05, 450.0, RATE);
    println!(
        "toe against heel: {high:+.1} dB at 1.6 kHz; heel against toe: {low:+.1} dB at 450 Hz"
    );
    assert!(high > 10.0, "{high}");
    assert!(low > 10.0, "{low}");
}

/// Auto: picked softly, the follower leaves the treadle near the heel; picked
/// hard, it reaches the knob's position at the toe. A tone at 1.6 kHz is far
/// louder, relative to its input, played hard.
#[test]
fn auto_opens_the_wah_with_the_playing() {
    let mut chain = Chain::new(RATE);
    let auto = WahSettings {
        wah: Some(Build::CryBaby),
        treadle: 1.0,
        auto: true,
        sense: 0.5,
    };
    let soft = through_chain(&mut chain, auto, 0.005, 1_600.0, RATE) - 20.0 * 0.005f64.log10();
    let hard = through_chain(&mut chain, auto, 0.2, 1_600.0, RATE) - 20.0 * 0.2f64.log10();
    println!("1.6 kHz relative to its input: {soft:+.1} dB soft, {hard:+.1} dB hard");
    assert!(hard - soft > 10.0, "{soft} {hard}");
}

/// The wah at every rate, its treadle swept at audio rate in Auto, finite and
/// without allocating.
#[test]
fn the_wah_is_realtime_safe_at_every_rate() {
    let mut chain = Chain::new(44_100.0);
    for (rate, oversampling, build) in [
        (44_100.0, 1, Build::CryBaby),
        (48_000.0, 2, Build::V847),
        (88_200.0, 1, Build::CryBaby),
        (96_000.0, 2, Build::V847),
        (192_000.0, 1, Build::CryBaby),
    ] {
        chain.set_rate(rate);
        chain.apply(&Settings {
            wah: WahSettings {
                wah: Some(build),
                treadle: 1.0,
                auto: true,
                sense: 0.8,
            },
            gain: Gain::Clean,
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
            for k in 0..8_192 {
                // A plucked note's envelope, so the follower sweeps the pot.
                let envelope = (-(k % 4_096) as f64 / (0.02 * rate)).exp();
                let x = 0.3 * envelope * (k as f64 * std::f64::consts::TAU * 220.0 / rate).sin();
                let y = chain.process(x);
                assert!(y.is_finite(), "{rate}");
                peak = peak.max(y.abs());
            }
        });
        assert!(peak > 1e-3, "{rate}: silent");
    }
}
