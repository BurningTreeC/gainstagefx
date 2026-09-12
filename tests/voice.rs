//! A circuit made playable, and the two joins that has to get right.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Cabinet, Chain, Gain, Tone as ToneSection, CALIBRATION};
use gainstagefx::voice::{NOMINAL_DBFS, POINTS, VOICES};

const RATE: f64 = 96_000.0;

fn nominal() -> f64 {
    10f64.powf(NOMINAL_DBFS / 20.0)
}

/// Peak level in dBFS of a settled tone at a chain's output.
fn level_through(chain: &mut Chain, amplitude: f64) -> f64 {
    let tone = Tone::near(RATE, 16_384, 220.0, amplitude);
    measure::run(tone, (RATE / 2.0) as usize, |x| chain.process(x))
        .fundamental()
        .magnitude()
        .max(1e-12)
        .log10()
        * 20.0
}

/// Everything the plugin can select has to build. A `Fault` at this point is
/// a panic in a host, so it is worth one cheap test.
#[test]
fn the_whole_catalogue_builds() {
    for index in 0..VOICES {
        let (gain, diode, amplifier) = voice::voice_at(index);
        voice::build_voice(gain, diode, amplifier).unwrap_or_else(|f| {
            panic!(
                "{} / {} / {}: {f:?}",
                gain.name(),
                diode.name(),
                amplifier.name()
            )
        });
    }
    for t in ToneSection::ALL {
        if let Some(built) = t.build() {
            built.unwrap_or_else(|f| panic!("tone {}: {f:?}", t.name()));
        }
    }
    for c in Cabinet::ALL {
        if let Some(built) = c.build() {
            built.unwrap_or_else(|f| panic!("cabinet {}: {f:?}", c.name()));
        }
    }
}

/// The full schematic models are intentionally kept at the host rate. They
/// are large nonlinear solves, and applying the global 4x default to them can
/// make a DAW miss its real-time deadline.
///
/// Oversampling-factor changes are deliberately installed on the first sample
/// of a switch fade so resetting the halfband histories cannot click. The test
/// therefore has to advance the chain once before asking which factor is
/// actually active.
#[test]
fn modelled_circuits_stay_at_host_rate() {
    for gain in [
        Gain::Screamer,
        Gain::Muff,
        Gain::Boogie,
        Gain::Peavey,
        Gain::Neve,
        Gain::Twin,
    ] {
        let mut chain = Chain::new(RATE);
        chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        chain.set_oversampling(8);
        chain.process(0.0);
        assert_eq!(
            chain.effective_oversampling(),
            1,
            "{} must not inherit the global oversampling factor",
            gain.name(),
        );
    }

    let mut generic = Chain::new(RATE);
    generic.set_voice(
        Gain::Overdrive,
        voice::Diode::Silicon,
        voice::Amplifier::Valve,
    );
    generic.set_oversampling(4);
    generic.process(0.0);
    assert_eq!(generic.effective_oversampling(), 4);
}

/// The Twin power stage deliberately stops its realtime line search at 1/16.
/// Keep that performance policy tied to an explicit accuracy measurement: the
/// normal four-backtrack solve must null far below audibility against the full
/// six-backtrack / 64-pass reference on hard, preamp-driven attacks.
#[test]
#[ignore = "high-accuracy Twin solver reference; run in release mode"]
fn twin_power_four_backtracks_matches_full_reference() {
    const RATE: f64 = 48_000.0;
    const SAMPLES: usize = 4_096;
    const MAX_NULL_DB: f64 = -140.0;

    #[cfg(debug_assertions)]
    panic!("use cargo test --release for the Twin solver reference");

    let mut exercised_backtracks = 0u64;
    for drive in [0.2, 0.6, 1.0] {
        let mut channel = Simulation::new(
            voice::build_voice(Gain::Twin, voice::Diode::Silicon, voice::Amplifier::Valve)
                .expect("Twin channel builds"),
            RATE,
        );
        channel.set_control(Gain::Twin.drive_control(), drive);
        channel.find_operating_point();

        let power = voice::build_power(Gain::Twin)
            .expect("Twin has a power stage")
            .expect("Twin power stage builds");
        let mut realtime = Simulation::new(power.clone(), RATE);
        realtime.set_backtracks(4);
        realtime.set_pass_ceiling(64);
        realtime.find_operating_point();

        let mut reference = Simulation::new(power, RATE);
        reference.set_backtracks(6);
        reference.set_pass_ceiling(64);
        reference.find_operating_point();

        let mut error_energy = 0.0;
        let mut reference_energy = 0.0;
        for i in 0..SAMPLES {
            let t = i as f64 / RATE;
            let attack = (t / 0.0005).min(1.0);
            let decay = (-(t / 0.080)).exp();
            let input = 0.969
                * 0.5
                * attack
                * decay
                * ((std::f64::consts::TAU * 110.0 * t).sin()
                    + 0.5 * (std::f64::consts::TAU * 330.0 * t).sin());
            let power_input = channel.process(input);
            let got = realtime.process(power_input);
            let want = reference.process(power_input);
            let error = got - want;
            error_energy += error * error;
            reference_energy += want * want;
        }

        let null_db = 10.0
            * (error_energy.max(1e-300) / reference_energy.max(1e-300)).log10();
        let realtime_health = realtime.health();
        let reference_health = reference.health();
        exercised_backtracks += reference_health.0;
        eprintln!(
            "Twin power reference: drive={drive:.1}, null={null_db:.1} dB, realtime_backtracks={}, reference_backtracks={}",
            realtime_health.0, reference_health.0
        );
        assert!(
            null_db < MAX_NULL_DB,
            "Twin drive {drive:.1}: four-backtrack power solve is {null_db:.1} dB from the full reference"
        );
        assert_eq!(
            realtime_health.2,
            0,
            "realtime Twin produced non-finite corrections"
        );
        assert_eq!(
            reference_health.2,
            0,
            "reference Twin produced non-finite corrections"
        );
    }
    assert!(
        exercised_backtracks > 0,
        "Twin accuracy reference did not exercise the Newton line search"
    );
}

/// The table in `src/calibration.rs` is measured, and a circuit that has
/// changed underneath it makes it a set of stale numbers that still compile.
/// This is the test that notices.
#[test]
fn the_calibration_table_still_describes_the_circuits() {
    for (index, c) in CALIBRATION.iter().enumerate() {
        let (gain, diode, amplifier) = voice::voice_at(index);
        let netlist = voice::build_voice(gain, diode, amplifier).expect("builds");
        // A voice is two circuits where it has a power stage, and measuring
        // only the first would describe a preamplifier that no longer reaches
        // the output on its own.
        let behind = voice::build_power(gain).map(|b| b.expect("builds"));
        for (i, expected) in c.make_up_db.iter().enumerate() {
            let mut sim = Simulation::new(netlist.clone(), RATE);
            let mut power = behind.clone().map(|netlist| Simulation::new(netlist, RATE));
            // The knots are not evenly spaced -- see `voice::knot_position`.
            sim.set_control(gain.drive_control(), voice::knot_position(i));
            let tone = Tone::near(RATE, 16_384, 220.0, c.drive_volts);
            let got = -measure::run(tone, (RATE / 10.0) as usize, |x| {
                let y = sim.process(x);
                match power {
                    Some(ref mut p) => p.process(y),
                    None => y,
                }
            })
            .gain_db();
            assert!(
                (got - expected).abs() < 0.5,
                "{} / {} / {} at drive {}/{}: the table says {expected:.2} dB \
                 and the circuit now says {got:.2}. Re-run `cargo run --release \
                 --example calibrate > src/calibration.rs`.",
                gain.name(),
                diode.name(),
                amplifier.name(),
                i,
                POINTS - 1,
            );
        }
    }
}

/// The make-up is nine measured points with straight lines between them, and
/// what matters is the level *between* the points, not at them. If the curve
/// is too coarse the level sags in the middle of every segment and the drive
/// control is once again partly a volume control.
#[test]
fn the_make_up_holds_the_level_between_the_measured_points() {
    for index in 0..VOICES {
        let (gain, diode, amplifier) = voice::voice_at(index);
        let mut worst: f64 = 0.0;
        let mut worst_at = 0.0;
        // Deliberately off the measured grid.
        for step in 0..17 {
            let drive = (step as f64 + 0.5) / 17.0;
            let mut chain = Chain::new(RATE);
            chain.set_voice(gain, diode, amplifier);
            chain.set_drive(drive);
            let out = level_through(&mut chain, nominal());
            if (out - NOMINAL_DBFS).abs() > worst {
                worst = (out - NOMINAL_DBFS).abs();
                worst_at = drive;
            }
        }
        assert!(
            worst < 3.0,
            "{} / {} / {} drifts {worst:.2} dB at drive {worst_at:.2}, \
             so the drive control is partly a volume control",
            gain.name(),
            diode.name(),
            amplifier.name(),
        );
    }
}

/// A plate sits at a couple of hundred volts and the coupling capacitor is
/// what stops that reaching the output. If it does not, the first thing anyone
/// hears when the plugin loads is an enormous thump.
#[test]
fn silence_in_is_silence_out() {
    for index in 0..VOICES {
        let (gain, diode, amplifier) = voice::voice_at(index);
        let mut chain = Chain::new(RATE);
        chain.set_voice(gain, diode, amplifier);
        chain.set_drive(0.7);
        let mut worst: f64 = 0.0;
        for _ in 0..(RATE as usize / 4) {
            worst = worst.max(chain.process(0.0).abs());
        }
        assert!(
            worst < 0.02,
            "{} / {} / {} put out {worst:.4} on silence, which is a thump",
            gain.name(),
            diode.name(),
            amplifier.name(),
        );
    }
}

/// Switching a passive section in must not drop the plugin twenty decibels.
/// A tone stack can only cut -- that is what passive means -- so the loss has
/// to be given back, and the AC solver knows exactly how much.
#[test]
fn the_passive_sections_do_not_cost_the_level() {
    let reference = {
        let mut chain = Chain::new(RATE);
        chain.set_voice(Gain::Crunch, voice::Diode::Silicon, voice::Amplifier::Valve);
        chain.set_drive(0.8);
        level_through(&mut chain, nominal())
    };
    for t in [ToneSection::Wide, ToneSection::Scooping] {
        for c in [Cabinet::Off, Cabinet::Combo, Cabinet::Stack] {
            let mut chain = Chain::new(RATE);
            chain.set_voice(Gain::Crunch, voice::Diode::Silicon, voice::Amplifier::Valve);
            chain.set_tone_section(t);
            chain.set_cabinet(c);
            chain.set_drive(0.8);
            let out = level_through(&mut chain, nominal());
            assert!(
                (out - reference).abs() < 12.0,
                "tone {} into cabinet {} moved the level {:.1} dB",
                t.name(),
                c.name(),
                out - reference,
            );
        }
    }
}

/// A voice named Clean should be clean and a voice named Distortion should
/// not be, at the same knob setting. If the names do not order that way the
/// selection is decoration.
#[test]
fn the_voices_are_in_the_order_their_names_claim() {
    let thd = |gain: Gain| {
        let netlist = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
            .expect("builds");
        let c =
            CALIBRATION[voice::voice_index(gain, voice::Diode::Silicon, voice::Amplifier::Valve)];
        let mut sim = Simulation::new(netlist, RATE);
        sim.set_control(gain.drive_control(), 1.0);
        let tone = Tone::near(RATE, 16_384, 220.0, c.drive_volts);
        measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x)).thd_percent()
    };
    let (clean, crunch, high) = (thd(Gain::Clean), thd(Gain::Crunch), thd(Gain::HighGain));
    assert!(
        clean < crunch && crunch < high,
        "clean {clean:.1} %, crunch {crunch:.1} %, high gain {high:.1} %"
    );
    assert!(thd(Gain::Overdrive) < thd(Gain::Distortion));
}

/// The plugin has to delay by exactly what it tells the host, at every
/// oversampling setting.
///
/// It did not, at one of them. The padding that brings the shorter settings up
/// to the reported figure is exactly zero at eight times, and the delay line
/// rounded a length of zero up to one sample -- so the wet path came out one
/// sample later than the dry path it is mixed against, and one later than the
/// host had been told. A one sample offset between two copies of the same
/// signal is a comb filter, and it was there at that setting and no other.
#[test]
fn the_reported_latency_is_the_real_one_at_every_setting() {
    use gainstagefx::voice::LATENCY;

    for factor in [1usize, 2, 4, 8] {
        let mut chain = Chain::new(RATE);
        chain.set_voice(Gain::Clean, voice::Diode::Silicon, voice::Amplifier::Valve);
        chain.set_oversampling(factor);
        chain.set_drive(0.5);
        // Settle the operating point on silence, so the impulse is the only
        // thing in the answer.
        for _ in 0..(RATE as usize / 2) {
            chain.process(0.0);
        }

        let mut peak = (0usize, 0.0f64);
        for n in 0..(LATENCY as usize * 3) {
            let y = chain.process(if n == 0 { 0.05 } else { 0.0 }).abs();
            if y > peak.1 {
                peak = (n, y);
            }
        }
        assert_eq!(
            peak.0, LATENCY as usize,
            "{factor}x delays by {} samples and reports {LATENCY}",
            peak.0
        );
    }
}

/// And the dry path has to line up with the wet one, which is the thing the
/// latency figure is actually for.
#[test]
fn the_dry_path_lines_up_with_the_wet_one() {
    for factor in [1usize, 2, 4, 8] {
        let mut chain = Chain::new(RATE);
        chain.set_voice(Gain::Clean, voice::Diode::Silicon, voice::Amplifier::Valve);
        chain.set_oversampling(factor);
        for _ in 0..(RATE as usize / 2) {
            chain.process(0.0);
            chain.delayed_dry(0.0);
        }
        let mut dry_at = 0;
        for n in 0..(gainstagefx::voice::LATENCY as usize * 3) {
            let x = if n == 0 { 1.0 } else { 0.0 };
            chain.process(x);
            if chain.delayed_dry(x).abs() > 0.5 {
                dry_at = n;
                break;
            }
        }
        assert_eq!(
            dry_at,
            gainstagefx::voice::LATENCY as usize,
            "at {factor}x the dry path arrives at {dry_at}, not with the wet one"
        );
    }
}

/// Resetting has to leave the circuit at rest, not flat.
///
/// The host calls reset when the transport starts. What keeps a valve plate's
/// couple of hundred volts out of the output is the charge standing on the
/// coupling capacitor -- so emptying every capacitor without putting the
/// operating point back means the circuit charges up through them from
/// nothing, and that arrives as a loud pop at the moment playback begins.
#[test]
fn resetting_does_not_pop() {
    for index in 0..VOICES {
        let (gain, diode, amplifier) = voice::voice_at(index);
        let mut chain = Chain::new(RATE);
        chain.set_voice(gain, diode, amplifier);
        chain.set_drive(0.7);
        chain.settle();

        // Play a little, as a host would before stopping.
        for i in 0..2000 {
            chain.process(0.05 * (i as f64 * 0.01).sin());
        }
        chain.reset();

        // Transport starts again: silence in should be silence out.
        let mut worst: f64 = 0.0;
        for _ in 0..(RATE as usize / 4) {
            worst = worst.max(chain.process(0.0).abs());
        }
        assert!(
            worst < 0.02,
            "{} / {} / {} put out {worst:.4} after a reset, which is a pop",
            gain.name(),
            diode.name(),
            amplifier.name(),
        );
    }
}

/// A pick attack is a genuine discontinuity at the input, and the solver
/// has to be able to follow it without either gating it silent or letting
/// a Newton overshoot through as a click.
///
/// The two symptoms this catches, in order of how they appeared during
/// development:
///
/// - The output drops to near silence during the attack and takes a few
///   samples to recover. That is a `failed`-sample answer being clamped
///   too tight: the solver did not converge, the fallback used a bound
///   scaled by the recent movement, and the recent movement was the
///   pre-attack quiet.
/// - The output spikes and then continues ringing after the input stops.
///   That is the transformer core's flux or a capacitor's charge being
///   integrated from a failed sample's bounded guess, walking the state
///   far enough from any plausible operating point that the circuit
///   cannot return.
///
/// It is deliberately not a full-spectrum test. It is an instrument for
/// detecting the two specific states, so that a change to the solver
/// that re-opens either one is caught on the same commit.
#[test]
fn a_pick_attack_is_followed_and_does_not_leave_the_chain_ringing() {
    // The voices with the largest solvers and the most state to walk away
    // from. The pedals do not fail in this regime and are not tested.
    for gain in [Gain::Twin, Gain::Peavey, Gain::Boogie] {
        let mut chain = Chain::new(RATE);
        chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        chain.set_drive(0.85);
        chain.settle();

        // Settle on silence, so the attack is the only thing in flight.
        for _ in 0..(RATE as usize / 2) {
            chain.process(0.0);
        }

        // A pick: an instantaneous attack with a short decay, one string
        // at a time, then a chord. Amplitude is 0.5 of full scale so the
        // input trim does not push it into the plugin's own limiter.
        //
        // The attack shape is measured from what a piezo pickup produces
        // on a hard pick: a one-or-two-sample rise from nothing to the
        // peak, a short plateau, then an exponential decay over about
        // eighty milliseconds.
        let play = |chain: &mut Chain, seconds: f64, amplitude: f64, hz: f64| {
            let n = (seconds * RATE) as usize;
            let mut peak_output: f64 = 0.0;
            for i in 0..n {
                let t = i as f64 / RATE;
                // The envelope: rise in 0.5 ms, hold 5 ms, decay in 80 ms.
                let env = if t < 0.0005 {
                    t / 0.0005
                } else if t < 0.0055 {
                    1.0
                } else {
                    (-(t - 0.0055) / 0.080).exp()
                };
                let x = env * amplitude * (std::f64::consts::TAU * hz * t).sin();
                let y = chain.process(x).abs();
                if y > peak_output {
                    peak_output = y;
                }
            }
            peak_output
        };

        // Play a note and measure its peak. Then play the same note
        // through a version with a slow attack -- the same total energy
        // but arrived in milliseconds rather than a sample -- and check
        // that the peaks are in the same order of magnitude. If the fast
        // attack is gated, its peak is far below the slow one's.
        let fast_peak = play(&mut chain, 0.15, 0.5, 220.0);

        // Reset and settle for the slow comparison.
        chain.reset();
        chain.settle();
        for _ in 0..(RATE as usize / 2) {
            chain.process(0.0);
        }
        let slow_peak = {
            let n = (0.15 * RATE) as usize;
            let mut peak: f64 = 0.0;
            for i in 0..n {
                let t = i as f64 / RATE;
                // 10 ms rise.
                let env = if t < 0.010 {
                    t / 0.010
                } else {
                    (-(t - 0.010) / 0.080).exp()
                };
                let x = env * 0.5 * (std::f64::consts::TAU * 220.0 * t).sin();
                let y = chain.process(x).abs();
                if y > peak {
                    peak = y;
                }
            }
            peak
        };

        assert!(
            fast_peak > slow_peak * 0.25,
            "{} gated the pick attack: the fast attack peaked at {fast_peak:.4} \
             against {slow_peak:.4} for the same note played slowly",
            gain.name(),
        );
        assert!(
            fast_peak < slow_peak * 10.0,
            "{} overshot the pick attack: the fast attack peaked at \
             {fast_peak:.4} against {slow_peak:.4} for the same note played \
             slowly",
            gain.name(),
        );

        // And after the input stops, the chain has to return to silence
        // rather than keep ringing. First allow a quarter of a second for
        // the transformer's flux and the coupling capacitors to settle, then
        // listen for another quarter second. Measuring from the very first
        // silent sample mistakes the ordinary decay of the last note for the
        // persistent ringing this regression test is meant to catch.
        for _ in 0..(RATE as usize / 4) {
            chain.process(0.0);
        }
        let mut worst: f64 = 0.0;
        for _ in 0..(RATE as usize / 4) {
            worst = worst.max(chain.process(0.0).abs());
        }
        assert!(
            worst < 0.05,
            "{} kept ringing after a pick attack: peak {worst:.4} on silence",
            gain.name(),
        );
    }
}

/// A chord with three notes, each picked, is where the Twin was reported
/// to go into its pulsating after the pick. The mechanism was the
/// transformer core's flux being integrated from a failed sample's
/// bounded guess for a run of consecutive failures. This test plays the
/// chord, then listens on silence for two seconds, and asserts that
/// nothing is audible.
#[test]
fn a_chord_of_picks_does_not_leave_the_twin_pulsating() {
    let mut chain = Chain::new(RATE);
    chain.set_voice(Gain::Twin, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_drive(0.85);
    chain.settle();
    for _ in 0..(RATE as usize / 2) {
        chain.process(0.0);
    }

    // Three strings picked in quick succession, the way a chord is
    // strummed: 30 ms apart, each with a fast attack and a short decay.
    for hz in [110.0, 220.0, 330.0] {
        let offset = (0.030 * RATE) as usize;
        for _ in 0..offset {
            chain.process(0.0);
        }
        let n = (0.15 * RATE) as usize;
        for i in 0..n {
            let t = i as f64 / RATE;
            let env = if t < 0.001 {
                t / 0.001
            } else {
                (-(t - 0.001) / 0.100).exp()
            };
            let x = env * 0.35 * (std::f64::consts::TAU * hz * t).sin();
            chain.process(x);
        }
    }

    // Give the ordinary note/capacitor tail the same quarter-second settling
    // window as the single-pick test, then listen for two full seconds. The
    // old test started measuring at sample zero, so a perfectly decaying last
    // note could fail a test whose purpose is persistent self-oscillation.
    for _ in 0..(RATE as usize / 4) {
        chain.process(0.0);
    }
    let mut worst: f64 = 0.0;
    let mut worst_at = 0usize;
    for i in 0..(RATE as usize * 2) {
        let y = chain.process(0.0).abs();
        if y > worst {
            worst = y;
            worst_at = i;
        }
    }
    assert!(
        worst < 0.05,
        "the Twin kept pulsating after a chord of picks: peak {worst:.4} \
         at sample {worst_at}, which is {:.3} s after the settling window",
        worst_at as f64 / RATE,
    );
}

/// What the solver actually does on a pick attack, printed rather than
/// asserted.
///
/// This is the instrument the two pick-attack symptoms were never
/// diagnosed with. Everything I have proposed so far -- raising
/// `MAX_ITERATIONS`, skipping the post-failure extrapolation, widening
/// the failed-iterate bound -- has been a guess at the mechanism, and the
/// user reported that the guesses did not help. That means the guesses
/// are wrong and the mechanism needs to be measured.
///
/// What it prints, for one voice:
///
/// - the worst consecutive run of unsettled samples on a pick attack
/// - the total unsettled count
/// - the input and output at each of the first twenty failed samples
/// - the final solver statistics for the gain circuit and the power stage
///
/// Run with `cargo test --test voice diagnose_the_pick_attack -- --nocapture`
/// and paste the output.
#[test]
fn diagnose_the_pick_attack() {
    use gainstagefx::dsp::time::Simulation;
    use gainstagefx::voice;

    const RATE: f64 = 96_000.0;

    // The three voices with the largest solvers.
    for gain in [Gain::Twin, Gain::Peavey, Gain::Boogie] {
        let netlist = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve)
            .expect("builds");
        let power = voice::build_power(gain).map(|b| b.expect("builds"));

        let index = voice::voice_index(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
        let cal = &CALIBRATION[index];

        let mut sim = Simulation::new(netlist, RATE);
        let mut power_sim = power.map(|n| Simulation::new(n, RATE));

        sim.set_control(gain.drive_control(), 0.85);

        // Settle on silence so the pick is the only thing in flight.
        for _ in 0..(RATE as usize / 2) {
            let y = sim.process(0.0);
            if let Some(ref mut p) = power_sim {
                p.process(y);
            }
        }

        // A pick, in volts at the voice's own calibration.
        let volts = cal.drive_volts;
        let n = (0.3 * RATE) as usize;

        let mut prev_gain_unsettled = sim.statistics().2;
        let mut prev_power_unsettled = power_sim.as_ref().map(|p| p.statistics().2);

        let mut worst_run = 0usize;
        let mut current_run = 0usize;
        let mut failed_samples: Vec<(usize, f64, f64, f64)> = Vec::new();

        for i in 0..n {
            let t = i as f64 / RATE;
            // Envelope: 1 ms rise, 5 ms hold, 100 ms decay.
            let env = if t < 0.001 {
                t / 0.001
            } else if t < 0.006 {
                1.0
            } else {
                (-(t - 0.006) / 0.100).exp()
            };
            let x = env * volts * (std::f64::consts::TAU * 220.0 * t).sin();

            let y = sim.process(x);
            let out = if let Some(ref mut p) = power_sim {
                p.process(y)
            } else {
                y
            };

            let gain_unsettled = sim.statistics().2;
            let failed_gain = gain_unsettled > prev_gain_unsettled;
            prev_gain_unsettled = gain_unsettled;

            let failed_power = if let Some(ref mut p) = power_sim {
                let now = p.statistics().2;
                let f = now > prev_power_unsettled.unwrap();
                prev_power_unsettled = Some(now);
                f
            } else {
                false
            };

            if failed_gain || failed_power {
                current_run += 1;
                if current_run > worst_run {
                    worst_run = current_run;
                }
                if failed_samples.len() < 40 {
                    failed_samples.push((i, x, y, out));
                }
            } else {
                current_run = 0;
            }
        }

        println!();
        println!("=== {} ===", gain.name());
        println!("  drive volts (input for nominal): {volts:.4} V");
        println!("  worst consecutive unsettled run: {worst_run} samples");
        println!("  total failed samples: {}", failed_samples.len());
        println!();
        println!("  first 40 failed samples: index / input / gain out / power out");
        for (i, x, y, z) in &failed_samples {
            println!("    {i:>6}  in={x:+.5}  gain={y:+.5}  power={z:+.5}");
        }

        let g = sim.statistics();
        println!();
        println!(
            "  gain circuit: solves={} passes={} unsettled={} rebuilds={}",
            g.0, g.1, g.2, g.3
        );
        let (backtracks, fallbacks, nonfinite) = sim.health();
        println!(
            "  gain health:  backtracks={backtracks} fallbacks={fallbacks} nonfinite={nonfinite}"
        );
        let (suppressed, attempts, midpoint_successes, successes) = sim.continuation_health();
        println!(
            "  gain attack:  predictor_suppressions={suppressed} continuations={attempts} midpoint_successes={midpoint_successes} final_successes={successes}"
        );
        if let Some(p) = power_sim.as_ref() {
            let s = p.statistics();
            println!(
                "  power stage:  solves={} passes={} unsettled={} rebuilds={}",
                s.0, s.1, s.2, s.3
            );
            let (b, f, n) = p.health();
            println!("  power health: backtracks={b} fallbacks={f} nonfinite={n}");
            let (suppressed, attempts, midpoint_successes, successes) = p.continuation_health();
            println!(
                "  power attack: predictor_suppressions={suppressed} continuations={attempts} midpoint_successes={midpoint_successes} final_successes={successes}"
            );
        }
    }
}
