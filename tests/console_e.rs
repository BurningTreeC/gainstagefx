//! The British 4K E mic amplifier, against SSL's drawing and Jensen's data sheet.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/micpre.rs"]
mod micpre;
use gainstagefx::circuits::console_e::{self as ce, GAIN};
use gainstagefx::voice::Gain;
use micpre::*;

const RATE: f64 = 96_000.0;

/// Jensen JE-115K-E in Jensen's test circuit 1 (75 + 75 ohm source, 150 k load):
/// 19.75 dB at the terminals, -3 dB at 2.5 Hz and 90 kHz, 1 % at 20 Hz with
/// -2.5 dBu in.
#[test]
fn the_jensen_meets_its_data_sheet() {
    let t = transformer(150.0, 19.7, ce::INPUT_CORE, 0.1, 2_465.0, 85e-12, 150_000.0);
    // Against the source EMF: Jensen's 1.40 k input impedance costs 0.89 dB.
    let mid = measure(&t, RATE, &[], 1_000.0, 0.01).gain_db();
    assert!((mid + 0.89 - 19.75).abs() < 0.3, "{mid}");
    let low = measure(&t, 8_000.0, &[], 2.5, 0.01).gain_db() - mid;
    let high = measure(&t, 384_000.0, &[], 90_000.0, 0.01).gain_db() - mid;
    assert!((low + 3.0).abs() < 0.5, "{low}");
    assert!((high + 3.0).abs() < 0.5, "{high}");
    let thd = measure(&t, 48_000.0, &[], 20.0, dbu(-2.5) * 1_550.0 / 1_400.0).thd_percent();
    assert!((0.7..1.4).contains(&thd), "{thd}");
}

/// The MIC GAIN pot moves resistance from T9's input to T1's feedback: 0 dB to
/// 50 dB of op-amp gain, and 20 dB of transformer in front.
#[test]
fn mic_gain_spans_twenty_to_seventy_decibels() {
    let c = ce::build(150.0, 10_000.0).unwrap();
    let at = |g: f64| measure(&c, RATE, &[(GAIN, g)], 1_000.0, 1e-5).gain_db();
    let (low, mid, high) = (at(0.0), at(0.5), at(1.0));
    assert!((low - 19.0).abs() < 1.5, "{low}");
    assert!((high - 69.0).abs() < 1.5, "{high}");
    assert!(mid > low + 20.0 && mid < high - 20.0, "{low} {mid} {high}");
    let flat = |hz: f64| measure(&c, RATE, &[(GAIN, 0.5)], hz, 1e-4).gain_db() - mid;
    assert!(flat(20.0).abs() < 0.5 && flat(20_000.0).abs() < 0.5);
}

#[test]
fn it_runs_in_the_chain_without_allocating() {
    realtime_safe(Gain::ConsoleE, |f| allocations::assert_no_heap(f));
}
