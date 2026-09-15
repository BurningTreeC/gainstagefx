//! Cabinet and microphone processing: filters, delay, polar patterns, placement
//! behaviour, geometry, dual microphones, automation safety and rate independence.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::filters::{analog, Biquad, DelayLine};
use gainstagefx::acoustics::mic::{MicPlacement, MicProfile, Pattern};
use gainstagefx::acoustics::speaker::SpeakerProfile;
use gainstagefx::acoustics::stage::{AcousticStage, MicSlot};
use std::f64::consts::{PI, TAU};

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

#[test]
fn biquads_match_their_analog_prototypes_in_the_audible_band() {
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for (fc, gain, q) in [(900.0, 4.2, 1.9), (2_454.0, 10.6, 1.26), (11_129.0, 1.76, 3.75)] {
            let b = Biquad::peaking(rate, fc, gain, q);
            for f in [100.0, 1_000.0, 3_000.0, 8_000.0, 12_000.0] {
                let err = db(b.magnitude(rate, f)) - analog::peaking_db(f, fc, gain, q);
                let bound = if fc > 0.2 * rate { 1.0 } else { 0.25 };
                assert!(err.abs() < bound, "peak {fc} at {f} Hz, {rate}: {err}");
            }
        }
        for (fc, q) in [(5_128.0, 0.5), (8_789.0, 1.269), (39_870.0, 0.454)] {
            let b = Biquad::lowpass(rate, fc, q);
            for f in [200.0, 2_000.0, 6_000.0, 12_000.0, 16_000.0] {
                let err = db(b.magnitude(rate, f)) - analog::lowpass_db(f, fc, q);
                assert!(err.abs() < 1.0, "lowpass {fc} at {f} Hz, {rate}: {err}");
            }
        }
        for (fc, q) in [(64.0, 0.46), (17.5, 0.528)] {
            let b = Biquad::highpass(rate, fc, q);
            for f in [20.0, 50.0, 200.0, 1_000.0] {
                let err = db(b.magnitude(rate, f)) - analog::highpass_db(f, fc, q);
                assert!(err.abs() < 0.05, "highpass {fc} at {f} Hz: {err}");
            }
        }
    }
}

#[test]
fn fractional_delay_is_exact_at_zero_and_accurate_between_samples() {
    let rate = 48_000.0;
    let mut line = DelayLine::new();
    let hz = 1_500.0;
    for k in 0..4_000 {
        let x = (TAU * hz * k as f64 / rate).sin();
        line.write(x);
        assert_eq!(line.read(0.0), x);
        if k > 500 {
            for delay in [0.37, 1.5, 17.25, 400.8] {
                let expected = (TAU * hz * (k as f64 - delay) / rate).sin();
                assert!((line.read(delay) - expected).abs() < 2e-3, "{delay}");
            }
        }
    }
}

#[test]
fn polar_patterns_have_their_textbook_shapes() {
    let deg = |d: f64| d * PI / 180.0;
    let c = Pattern::Cardioid;
    assert!(c.gain(0.0) > c.gain(deg(90.0)) && c.gain(deg(90.0)) > c.gain(deg(180.0)));
    assert!(c.gain(deg(180.0)).abs() < 1e-12);
    let f8 = Pattern::Figure8;
    assert!((f8.gain(0.0).abs() - f8.gain(deg(180.0)).abs()).abs() < 1e-12);
    assert!(f8.gain(deg(180.0)) < 0.0, "the rear lobe is inverted");
    assert!(f8.gain(deg(90.0)).abs() < 1e-12);
    let hyper = Pattern::Hypercardioid;
    assert!(hyper.gain(deg(109.47)).abs() < 1e-3);
    let omni = Pattern::Omni;
    assert_eq!(omni.gain(deg(135.0)), 1.0);
}

struct Rig {
    stage: AcousticStage,
    rate: f64,
}

impl Rig {
    fn new(rate: f64, cab: Option<&'static CabinetProfile>, speaker: &'static SpeakerProfile, mic: MicSlot) -> Self {
        let mut stage = AcousticStage::new(rate);
        stage.configure(cab, speaker, mic, MicSlot::Off);
        Self { stage, rate }
    }

    fn place(&mut self, position: f64, distance: f64, angle: f64) -> &mut Self {
        let p = MicPlacement { position, distance, angle };
        self.stage.set_placement(p, p, 0.0, false, false);
        self.stage.reset();
        self
    }

    /// Steady-state level of a sine through the stage, dB.
    fn level(&mut self, hz: f64) -> f64 {
        self.stage.reset();
        let n = (self.rate * 0.25) as usize + (self.rate * 8.0 / hz) as usize;
        let mut peak: f64 = 0.0;
        for k in 0..n {
            let y = self.stage.process(0.1 * (TAU * hz * k as f64 / self.rate).sin());
            if k > n * 3 / 4 {
                peak = peak.max(y.abs());
            }
        }
        db(peak / 0.1)
    }
}

const SM57: MicSlot = MicSlot::Profile(&MicProfile::DYNAMIC_57);

#[test]
fn centre_is_brighter_than_edge_without_a_treble_knob() {
    let mut r = Rig::new(48_000.0, Some(&CabinetProfile::CLOSED_112), &SpeakerProfile::BRIT_V30, SM57);
    let centre = (r.place(0.0, 0.025, 0.0).level(500.0), r.level(5_000.0));
    let edge = (r.place(1.0, 0.025, 0.0).level(500.0), r.level(5_000.0));
    let darkening = (centre.1 - centre.0) - (edge.1 - edge.0);
    assert!((3.0..12.0).contains(&darkening), "edge darker by {darkening} dB");
    assert!((centre.0 - edge.0).abs() < 3.0, "low mids barely move: {centre:?} {edge:?}");
}

#[test]
fn distance_lowers_level_and_proximity_and_ribbons_have_more_of_it() {
    let tilt = |mic: MicSlot| {
        let mut r = Rig::new(48_000.0, Some(&CabinetProfile::CLOSED_112), &SpeakerProfile::BRIT_V30, mic);
        let close = r.place(0.2, 0.01, 0.0).level(100.0) - r.level(1_000.0);
        let close_1k = r.level(1_000.0);
        let far = r.place(0.2, 0.6, 0.0).level(100.0) - r.level(1_000.0);
        let far_1k = r.level(1_000.0);
        (close - far, close_1k - far_1k)
    };
    let (sm57_bass, sm57_level) = tilt(SM57);
    let (ribbon_bass, _) = tilt(MicSlot::Profile(&MicProfile::RIBBON_121));
    let (ideal_bass, ideal_level) = tilt(MicSlot::Ideal);
    assert!(sm57_level > 3.0, "farther is quieter: {sm57_level}");
    assert!(ideal_level > 3.0);
    assert!(sm57_bass > 1.0, "proximity: {sm57_bass}");
    assert!(ribbon_bass > sm57_bass + 2.0, "ribbon {ribbon_bass} vs dynamic {sm57_bass}");
    // An omni pressure transducer has no proximity; only geometry (array, baffle) remains.
    assert!(ideal_bass < sm57_bass, "omni {ideal_bass}");
}

#[test]
fn angle_is_microphone_specific() {
    let darken = |mic: MicSlot| {
        let mut r = Rig::new(48_000.0, Some(&CabinetProfile::CLOSED_112), &SpeakerProfile::BRIT_V30, mic);
        let on = r.place(0.0, 0.3, 0.0).level(8_000.0) - r.level(1_000.0);
        let off = r.place(0.0, 0.3, 45.0).level(8_000.0) - r.level(1_000.0);
        on - off
    };
    let dynamic = darken(SM57);
    let ribbon = darken(MicSlot::Profile(&MicProfile::RIBBON_121));
    assert!(dynamic > 0.3, "SM57 off-axis darker: {dynamic}");
    assert!(ribbon < dynamic, "ribbon stays more consistent: {ribbon} vs {dynamic}");
    // A figure-8 turned side-on to a single source nulls it.
    let mut r = Rig::new(48_000.0, Some(&CabinetProfile::CLOSED_112), &SpeakerProfile::BRIT_V30, MicSlot::Profile(&MicProfile::RIBBON_121));
    let null = r.place(0.0, 0.8, 0.0).level(1_000.0) - r.place(0.0, 0.8, 90.0).level(1_000.0);
    assert!(null > 15.0, "figure-8 side null: {null}");
}

#[test]
fn condenser_extends_further_than_dynamic() {
    let top = |mic: MicSlot| {
        let mut r = Rig::new(48_000.0, None, &SpeakerProfile::AMERICAN_VINTAGE_12, mic);
        r.place(0.0, 0.1, 0.0).level(14_000.0) - r.level(1_000.0)
    };
    assert!(top(MicSlot::Profile(&MicProfile::CONDENSER_87)) > top(SM57) + 3.0);
}

#[test]
fn a_4x12_is_not_a_louder_1x12() {
    let shape = |cab: &'static CabinetProfile| {
        let mut r = Rig::new(48_000.0, Some(cab), &SpeakerProfile::BRIT_V30, MicSlot::Ideal);
        r.place(0.0, 1.0, 0.0);
        [150.0, 1_000.0, 2_500.0, 4_000.0].map(|f| r.level(f))
    };
    let one = shape(&CabinetProfile::CLOSED_112);
    let four = shape(&CabinetProfile::BRIT_CLOSED);
    let rel = |s: [f64; 4]| [s[0] - s[1], s[2] - s[1], s[3] - s[1]];
    let (a, b) = (rel(one), rel(four));
    // At a metre, four cones add up at low frequencies and interfere and beam at high
    // ones, so the array's bass-to-treble balance differs from the single cone's.
    let tilt = |s: [f64; 4]| s[0] - s[3];
    assert!(tilt(four) > tilt(one) + 3.0, "array tilt: {one:?} vs {four:?}");
    let spread = (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max);
    assert!(spread > 3.0, "{a:?} vs {b:?}");
}

#[test]
fn an_open_back_cancels_low_frequencies_a_closed_box_does_not() {
    let bass = |cab: &'static CabinetProfile| {
        let mut r = Rig::new(48_000.0, Some(cab), &SpeakerProfile::AMERICAN_CERAMIC, MicSlot::Ideal);
        r.place(0.0, 1.0, 0.0).level(70.0) - r.level(1_000.0)
    };
    let open = bass(&CabinetProfile::AMERICAN_OPEN_112);
    let closed = bass(&CabinetProfile::CLOSED_112);
    assert!(closed > open + 2.0, "closed {closed} vs open {open}");
}

#[test]
fn dual_microphones_keep_physical_delay_and_polarity_works() {
    let rate = 48_000.0;
    let mut stage = AcousticStage::new(rate);
    stage.configure(Some(&CabinetProfile::BRIT_CLOSED), &SpeakerProfile::BRIT_T75, SM57, SM57);
    let near = MicPlacement { position: 0.3, distance: 0.02, angle: 0.0 };
    // Identical microphones at identical places, inverted and blended equally: silence.
    stage.set_placement(near, near, 0.5, true, false);
    stage.reset();
    let mut peak: f64 = 0.0;
    for k in 0..4_800 {
        peak = peak.max(stage.process((TAU * 440.0 * k as f64 / rate).sin()).abs());
    }
    assert!(peak < 1e-9, "{peak}");
    // Moving B back by 30 cm delays it by about 42 samples unless aligned.
    let far = MicPlacement { distance: 0.32, ..near };
    stage.set_placement(near, far, 0.5, false, false);
    stage.reset();
    let expected = 0.30 / 343.0 * rate;
    let (b_first, _) = stage.relative_delays(1);
    assert_eq!(stage.relative_delays(0).0, 0.0);
    assert!((b_first / expected - 1.0).abs() < 0.05, "{b_first} vs {expected}");
    stage.set_placement(near, far, 0.5, false, true);
    stage.reset();
    assert_eq!(stage.relative_delays(1).0, 0.0, "aligned");
}

#[test]
fn close_placement_adds_no_latency() {
    let mut stage = AcousticStage::new(48_000.0);
    stage.configure(Some(&CabinetProfile::CLOSED_112), &SpeakerProfile::BRIT_V30, MicSlot::Ideal, MicSlot::Off);
    let p = MicPlacement { position: 0.0, distance: 0.02, angle: 0.0 };
    stage.set_placement(p, p, 0.0, false, false);
    stage.reset();
    let response: Vec<f64> = (0..64).map(|k| stage.process(if k == 0 { 1.0 } else { 0.0 })).collect();
    let first = response.iter().position(|y| y.abs() > 1e-6).unwrap();
    assert_eq!(first, 0, "{response:?}");
}

#[test]
fn automation_is_smooth_bounded_and_allocation_free_at_every_rate() {
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        for cab in CabinetProfile::ALL {
            let mut stage = AcousticStage::new(rate);
            stage.configure(Some(cab), cab.default_speaker, SM57, MicSlot::Profile(&MicProfile::RIBBON_121));
            let mut seed = 0x9e3779b9u32;
            let mut rand = move || {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                f64::from(seed) / f64::from(u32::MAX)
            };
            let mut previous = 0.0;
            let mut worst_step: f64 = 0.0;
            assert_no_heap(|| {
                for block in 0..200 {
                    let a = MicPlacement { position: rand(), distance: 0.01 + rand() * 0.99, angle: rand() * 90.0 };
                    let b = MicPlacement { position: rand(), distance: 0.01 + rand() * 0.99, angle: rand() * 90.0 };
                    stage.set_placement(a, b, rand(), block % 7 == 0, block % 5 == 0);
                    for k in 0..128 {
                        let x = 0.25 * (TAU * 220.0 * (block * 128 + k) as f64 / rate).sin();
                        let y = stage.process(x);
                        assert!(y.is_finite() && y.abs() < 20.0, "{} at {rate}: {y}", cab.id);
                        worst_step = worst_step.max((y - previous).abs());
                        previous = y;
                    }
                }
            });
            // A 220 Hz sine moves at most ~0.03 per sample at this level; random
            // placement jumps every block must not produce steps far beyond that.
            assert!(worst_step < 1.5, "{} at {rate}: step {worst_step}", cab.id);
        }
    }
}

#[test]
fn placement_response_does_not_depend_on_sample_rate() {
    let mut reference = None;
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let mut r = Rig::new(rate, Some(&CabinetProfile::BRIT_V30), &SpeakerProfile::BRIT_V30, SM57);
        r.place(0.4, 0.05, 15.0);
        let levels = [200.0, 1_000.0, 3_000.0, 6_000.0].map(|f| r.level(f));
        match reference {
            None => reference = Some(levels),
            Some(first) => {
                for (i, (a, b)) in first.iter().zip(levels.iter()).enumerate() {
                    assert!((a - b).abs() < 1.0, "{rate} band {i}: {a} vs {b}");
                }
            }
        }
    }
}
