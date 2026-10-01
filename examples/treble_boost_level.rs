//! Where the Treble Boost's one pot is unity: `treble_boost::BOOST_REST`.
//!
//! The pedal alone in volts, fed a guitar's low chord -- the probe every level
//! measurement in the project uses -- and the Boost pot (10 k audio) bisected
//! for 0 dB broadband. On a low chord, because that is what "unity" means
//! everywhere else here; this pedal's whole point is that the treble above it
//! comes up by far more. Re-run after any change to `treble_boost.rs` and
//! paste the result.
//!
//! `cargo run --release --example treble_boost_level`

use gainstagefx::circuits::treble_boost::{self, BOOST, BOOST_REST};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::GUITAR_VOLTS;

const RATE: f64 = 48_000.0;

fn sample(k: usize) -> f64 {
    let t = k as f64 / RATE;
    GUITAR_VOLTS
        * ((std::f64::consts::TAU * 82.4 * t).sin() * 0.50
            + (std::f64::consts::TAU * 123.5 * t).sin() * 0.30
            + (std::f64::consts::TAU * 246.9 * t).sin() * 0.20)
}

/// Broadband gain through the pedal, dB.
fn gain_db(boost: f64) -> f64 {
    let mut sim = Simulation::new(treble_boost::build(10_000.0, 470_000.0).unwrap(), RATE);
    sim.set_control(BOOST, boost);
    sim.find_operating_point();
    let warm = (RATE / 2.0) as usize;
    for k in 0..warm {
        sim.process(sample(k));
    }
    let (mut x2, mut y2) = (0.0, 0.0);
    for k in warm..warm + RATE as usize {
        let x = sample(k);
        let y = sim.process(x);
        x2 += x * x;
        y2 += y * y;
    }
    10.0 * (y2 / x2).log10()
}

fn main() {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        if gain_db(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let rest = 0.5 * (lo + hi);
    println!("unity on a guitar's low chord: Boost {rest:.4}");
    println!("BOOST_REST = {BOOST_REST}: {:+.2} dB", gain_db(BOOST_REST));
    for boost in [0.25, 0.5, 0.75, 1.0] {
        println!("  boost {boost:.2}: {:+.1} dB", gain_db(boost));
    }
}
