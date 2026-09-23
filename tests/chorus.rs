//! The Jazz 120's bucket-brigade chorus, in the delay line and in the chain.
//!
//! Every figure checked here is printed on the EFF BOARD of the Sep. 2000
//! JC-120UT/JT service notes: the MN3007's 1024 stages, the MN3101's 2.5-12 us
//! clock period and the LFO's 1.2 s triangle in CHORUS. The delays and the
//! pitch shift below are those three numbers and arithmetic, not a fit.
//!
//! See docs/models/jazz_120.md, "The chorus, read off the sheet".
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::MicProfile;
use gainstagefx::acoustics::stage::MicSlot;
use gainstagefx::dsp::bbd::{self, Bbd, Mode};
use gainstagefx::voice::{
    AcousticSettings, Cabinet, CabinetChoice, Chain, Gain, PowerAmp, Settings, SpeakerChoice, Tone,
};

/// The five the solver is tested at.
const RATES: [f64; 5] = [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0];

fn signal(k: usize, rate: f64) -> f64 {
    // A guitar-ish pair well inside the bucket brigade's band.
    let t = k as f64 / rate;
    0.2 * (std::f64::consts::TAU * 220.0 * t).sin()
        + 0.1 * (std::f64::consts::TAU * 660.0 * t).sin()
}

fn jazz(chorus: f64) -> Settings {
    Settings {
        gain: Gain::Jazz120,
        power_amp: PowerAmp::Matched,
        tone: Tone::Off,
        cabinet: Cabinet::Off,
        chorus,
        acoustic: AcousticSettings {
            cabinet: CabinetChoice::Model(&CabinetProfile::AMERICAN_OPEN_212),
            speaker: SpeakerChoice::Matched,
            mic_a: MicSlot::Profile(&MicProfile::DYNAMIC_57),
            ..AcousticSettings::default()
        },
        ..Settings::default()
    }
}

// --- the delay line on its own ---------------------------------------------

/// `N / (2 f_clk)` at each end of the printed clock period.
#[test]
fn the_delay_range_is_the_printed_clock_range() {
    // 1024 stages, 2.5 us to 12 us.
    assert!((bbd::SHORTEST - 1.28e-3).abs() < 1e-9, "{}", bbd::SHORTEST);
    assert!((bbd::LONGEST - 6.144e-3).abs() < 1e-9, "{}", bbd::LONGEST);
}

/// The sweep, measured by pushing one impulse through the line and finding
/// where it comes out -- once at the oscillator's short end and once at its
/// long end.
///
/// What is asserted is the *difference* between the two. The absolute figure
/// carries the four one-pole sections' group delay as well, about 98 us, and
/// that is a property of the filters rather than of the clock; the difference
/// cancels it and leaves the printed 4.864 ms.
#[test]
fn the_line_sweeps_by_what_the_clock_says() {
    for rate in RATES {
        let mut line = Bbd::new(rate);
        line.set_depth(1.0);
        // Phase zero: the triangle is at zero, the clock is at 400 kHz and the
        // line is at its shortest.
        let short = impulse_delay(&mut line, rate, 0);

        // Half a period on, the triangle is at its peak and turning round, so
        // the delay is both longest and momentarily stationary.
        let mut line = Bbd::new(rate);
        line.set_depth(1.0);
        let long = impulse_delay(&mut line, rate, (bbd::CHORUS_PERIOD * rate / 2.0) as usize);

        let swept = (long - short) / rate;
        // 1024 / (2 x 83.3 kHz) - 1024 / (2 x 400 kHz), written out rather
        // than taken off the constants so this measures the sheet and not the
        // module's arithmetic about itself.
        let want = 4.864e-3;
        assert!(
            (swept - want).abs() < 100e-6,
            "rate {rate}: swept {:.3} ms against {:.3} ms",
            swept * 1e3,
            want * 1e3
        );
    }
}

/// Depth is how much of that range is used, so at zero the line sits at the
/// short end and does not move.
#[test]
fn depth_at_zero_holds_the_line_still() {
    let rate = 48_000.0;
    let mut line = Bbd::new(rate);
    line.set_depth(0.0);
    let short = impulse_delay(&mut line, rate, 0);
    let mut line = Bbd::new(rate);
    line.set_depth(0.0);
    let later = impulse_delay(&mut line, rate, (bbd::CHORUS_PERIOD * rate / 2.0) as usize);
    assert!((later - short).abs() < 1.0, "{short} then {later}");
}

/// Where an impulse comes back out, in samples, after `prime` samples of
/// silence have been run through first to place the oscillator. The filters
/// either side spread it, so this finds the centre of mass of what returns
/// rather than one sample.
fn impulse_delay(line: &mut Bbd, rate: f64, prime: usize) -> f64 {
    for _ in 0..prime {
        line.process(0.0);
    }
    let n = (bbd::LONGEST * rate) as usize * 3;
    let mut weight = 0.0;
    let mut moment = 0.0;
    for k in 0..n {
        let x = if k == 0 { 1.0 } else { 0.0 };
        let y = line.process(x).abs();
        weight += y;
        moment += y * k as f64;
    }
    assert!(weight > 1e-9, "nothing came back out of the line");
    moment / weight
}

/// CHORUS is a fixed 1.2 s triangle with no panel control; VIBRATO is the
/// CH1 board's own 120-500 ms.
#[test]
fn the_oscillator_periods_are_the_printed_ones() {
    let mut line = Bbd::new(48_000.0);
    for speed in [0.0, 0.5, 1.0] {
        line.set_speed(speed);
        assert_eq!(line.period(), bbd::CHORUS_PERIOD);
    }
    line.set_mode(Mode::Vibrato);
    line.set_speed(0.0);
    assert!((line.period() - bbd::VIBRATO_SLOW).abs() < 1e-12);
    line.set_speed(1.0);
    assert!((line.period() - bbd::VIBRATO_FAST).abs() < 1e-12);
}

/// The sweep is 4.864 ms over a 0.6 s half period, which is 0.81 % and about
/// 14 cents. That number is why this chorus is musical rather than seasick,
/// and it is the one the research log quotes.
#[test]
fn the_pitch_shift_is_about_fourteen_cents() {
    let rate = bbd::CHORUS_PERIOD / 2.0;
    let shift = (bbd::LONGEST - bbd::SHORTEST) / rate;
    let cents = 1200.0 * (1.0 + shift).log2();
    assert!((13.0..15.0).contains(&cents), "{cents} cents");
}

/// A delay line that changes rate must not allocate on the way, and must not
/// read past what it holds at the highest rate the plugin is tested at.
#[test]
fn changing_rate_never_reads_stale_or_out_of_range() {
    let mut line = Bbd::new(192_000.0);
    for rate in RATES {
        line.set_rate(rate);
        assert_no_heap(|| {
            for k in 0..(rate as usize / 8) {
                assert!(line.process(signal(k, rate)).is_finite());
            }
        });
    }
}

// --- and in the chain ------------------------------------------------------

/// Only the Jazz 120 has one. Every other circuit's knob must reach nothing,
/// which is the defect BUG-023 was about from the other side.
#[test]
fn only_the_jazz_120_has_a_chorus() {
    for gain in Gain::ALL {
        assert_eq!(gain.has_chorus(), gain == Gain::Jazz120, "{}", gain.name());
    }
}

/// SW3 OFF: both power amplifiers carry the dry signal, so the two outputs are
/// the same sample. Bit for bit, because the split is a lerp at zero and not a
/// filter that happens to be flat.
#[test]
fn the_chorus_at_zero_leaves_both_speakers_dry() {
    for rate in RATES {
        let mut chain = Chain::new(rate);
        chain.apply(&jazz(0.0));
        chain.find_operating_point();
        for k in 0..2_000 {
            let (left, right) = chain.process_stereo(signal(k, rate));
            assert_eq!(left, right, "rate {rate} sample {k}");
        }
    }
}

/// A circuit with no chorus must be untouched by the knob, however far it is
/// turned -- the same guard `set_reverb_and_tremolo` carries.
#[test]
fn the_knob_does_nothing_to_a_circuit_without_one() {
    let mut a = Chain::new(48_000.0);
    let mut b = Chain::new(48_000.0);
    let base = Settings {
        gain: Gain::Twin,
        tone: Tone::Off,
        ..Settings::default()
    };
    a.apply(&base);
    b.apply(&Settings {
        chorus: 1.0,
        ..base
    });
    a.find_operating_point();
    b.find_operating_point();
    for k in 0..2_000 {
        let x = signal(k, 48_000.0);
        assert_eq!(a.process(x), b.process(x), "sample {k}");
    }
}

/// SW3 CHORUS: one speaker carries the delay and the other the dry, so the two
/// sides differ -- and the left is still exactly what the mono path gives,
/// because the dry side is not touched.
#[test]
fn the_chorus_sends_one_speaker_the_delay_and_keeps_the_other_dry() {
    let rate = 48_000.0;
    let mut wet = Chain::new(rate);
    let mut dry = Chain::new(rate);
    wet.apply(&jazz(1.0));
    dry.apply(&jazz(0.0));
    wet.find_operating_point();
    dry.find_operating_point();

    let mut differed = 0;
    let mut left_matched = 0;
    // Past the longest delay, so the line is primed before anything is judged.
    for k in 0..8_000 {
        let x = signal(k, rate);
        let (left, right) = wet.process_stereo(x);
        let reference = dry.process(x);
        if k > 2_000 {
            if (left - right).abs() > 1e-6 {
                differed += 1;
            }
            if (left - reference).abs() < 1e-12 {
                left_matched += 1;
            }
        }
    }
    assert!(differed > 5_000, "the two speakers agreed: {differed}");
    assert_eq!(
        left_matched,
        6_000 - 1,
        "the dry side moved: {left_matched}"
    );
}

/// The chorus runs inside the audio callback. No allocation, at every rate.
#[test]
fn a_chorusing_chain_does_not_allocate() {
    for rate in RATES {
        let mut chain = Chain::new(rate);
        chain.apply(&jazz(1.0));
        chain.find_operating_point();
        assert_no_heap(|| {
            for k in 0..2_048 {
                let (left, right) = chain.process_stereo(signal(k, rate));
                assert!(left.is_finite() && right.is_finite());
            }
        });
    }
}

/// Turning the knob while playing must not step, and switching away from the
/// amplifier must leave the delay line out of the path entirely.
#[test]
fn the_knob_and_the_voice_switch_stay_finite() {
    let rate = 48_000.0;
    let mut chain = Chain::new(rate);
    chain.apply(&jazz(0.0));
    chain.find_operating_point();
    for k in 0..12_000 {
        let amount = (k as f64 / 12_000.0).min(1.0);
        chain.set_chorus(amount);
        if k == 6_000 {
            chain.apply(&Settings {
                gain: Gain::Twin,
                tone: Tone::Off,
                chorus: 1.0,
                ..Settings::default()
            });
        }
        let (left, right) = chain.process_stereo(signal(k, rate));
        assert!(left.is_finite() && right.is_finite(), "sample {k}");
        assert!(left.abs() < 32.0 && right.abs() < 32.0, "sample {k}");
    }
}
