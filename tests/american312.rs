//! The American 312, against API's card drawing and data sheets.
#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/micpre.rs"]
mod micpre;
use gainstagefx::circuits::american312::{self as a312, GAIN};
use gainstagefx::voice::Gain;
use micpre::*;

const RATE: f64 = 96_000.0;

/// The input transformer alone, as API rates it: ±0.5 dB at 30 Hz and 20 kHz from
/// a 150 ohm source, and about 1 % at its 0 dBm maximum level at 30 Hz.
#[test]
fn the_input_transformer_meets_its_data_sheet() {
    let t = transformer(
        150.0,
        20.0,
        a312::INPUT_CORE,
        a312::INPUT_RATIO,
        800.0,
        50e-12,
        1e9,
    );
    let mid = measure(&t, RATE, &[], 1_000.0, 1e-3).gain_db();
    // 150 ohm to 10 k: 18.2 dB of voltage gain.
    assert!((mid - 18.2).abs() < 0.5, "{mid}");
    for hz in [30.0, 20_000.0] {
        let rel = measure(&t, RATE, &[], hz, 1e-3).gain_db() - mid;
        assert!(rel.abs() < 0.6, "{hz} Hz: {rel:+.2} dB");
    }
    let thd = measure(&t, 48_000.0, &[], 30.0, dbu(0.0) * 2.0).thd_percent();
    assert!((0.5..2.5).contains(&thd), "{thd}");
}

/// The trim sweeps the card from 30 dB to 64 dB, evenly in decibels: 1 + 20 k /
/// (200 + trim) in the 2520's loop, 18 dB of input iron, 6 dB of output winding.
#[test]
fn the_gain_trim_spans_the_cards_range_evenly() {
    let c = a312::build(150.0, 10_000.0).unwrap();
    let at = |g: f64| measure(&c, RATE, &[(GAIN, g)], 1_000.0, 1e-5).gain_db();
    let points: Vec<f64> = [0.0, 0.25, 0.5, 0.75, 1.0].into_iter().map(at).collect();
    assert!((points[0] - 30.2).abs() < 1.0, "{points:?}");
    assert!((points[4] - 64.2).abs() < 1.0, "{points:?}");
    for pair in points.windows(2) {
        let step = pair[1] - pair[0];
        assert!((5.0..11.0).contains(&step), "{points:?}");
    }
}

/// It is clean until the 2520 meets its rail, and then it is not.
#[test]
fn it_is_clean_until_the_op_amp_runs_out() {
    let c = a312::build(150.0, 10_000.0).unwrap();
    let clean = measure(&c, RATE, &[(GAIN, 1.0)], 220.0, 0.005).thd_percent();
    let clipped = measure(&c, RATE, &[(GAIN, 1.0)], 220.0, 0.03).thd_percent();
    assert!(clean < 0.1, "{clean}");
    assert!(clipped > 10.0, "{clipped}");
}

#[test]
fn it_runs_in_the_chain_without_allocating() {
    realtime_safe(Gain::American312, |f| allocations::assert_no_heap(f));
}
