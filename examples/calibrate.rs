//! Measures the calibration table and prints it as Rust.
//!
//! Two numbers per voice.
//!
//! **The input level.** A circuit works in volts and a plugin is handed
//! numbers near one, and the join between them is a choice rather than a fact.
//! These figures were once half what they are, and the whole catalogue was
//! far cleaner than its names: a preset called Scooped Metal made three per
//! cent distortion on a nominal signal. What a pedal called Distortion does
//! with its knob at the stop is squared off, not mildly bent.
//! The choice made here is that each circuit does what its *name* says with a
//! nominal signal in front of it and the drive control **all the way up**: a
//! Clean voice barely bending, a Distortion voice thoroughly squared off.
//! Turning the knob down then only ever cleans up, which is what a player
//! expects of it and what the hardware does.
//!
//! Defining it at the middle of the knob instead does not work, and the reason
//! is worth keeping: the drive control now spans about eighty decibels, so its
//! middle sits some thirty decibels down, and the Clean voice could not reach
//! its stated figure there at any input level at all -- it peaked at 0.3 %
//! with four volts going in. The figures below are the intent, and they can be
//! argued with.
//!
//! **The make-up.** A curve across the drive control, which moves the gain of
//! these circuits by around eighty decibels end to end. Without it every
//! comparison between two settings is a comparison of loudness.
//!
//! Run with `cargo run --release --example calibrate > src/calibration.rs`.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain, Iron, IRON_VOLTS, NOMINAL_DBFS, POINTS, VOICES};

const RATE: f64 = 96_000.0;

/// What each circuit should be doing, in per cent total harmonic distortion,
/// with a nominal signal and the drive control all the way up.
fn intent(gain: Gain) -> f64 {
    match gain {
        // Barely working. This is the sound of a signal having been through
        // something, not the sound of distortion.
        Gain::Clean => 3.0,
        Gain::Crunch => 15.0,
        // Past this the character stops changing and only the level does.
        Gain::HighGain => 40.0,
        Gain::Overdrive => 20.0,
        // Not higher. A symmetric pair to ground squares the wave off, and a
        // square wave is about 43 per cent distortion and no more -- asking
        // for 45 sent the search to the top of its range looking for a level
        // that does not exist, and would have handed the pedal four volts for
        // a nominal signal. Slammed that hard it never cleans up again when
        // the playing does.
        Gain::Distortion => 38.0,
        // A microphone preamplifier is built not to run out of room, which is
        // the opposite of what a guitar preamplifier is for. These sit where
        // the character is audible and the channel is still doing its job.
        Gain::Console => 3.0,
        // And this one is the case where the answer is "as little as
        // possible": an op-amp on a studio rail contributes nothing of its own
        // until it hits it. Whatever character it has comes from the iron
        // after it, which is a separate control.
        Gain::Studio => 0.05,
        // The modelled circuits are voiced by what the drawing produces, not
        // by a figure chosen here -- so these are what each pedal or amplifier
        // actually does with its own control at the stop.
        //
        // The Screamer does not use this: its level is stated rather than
        // searched for. See `stated_level`.
        Gain::Screamer => 21.0,
        // A Twin Reverb is the clean reference. Its whole identity is that it
        // does not run out of room, so this is the lowest figure in the
        // catalogue and deliberately so: what a Twin does to a guitar is
        // audible and small. See `CLAUDE.md` §51.2, which asks that it stay
        // substantially cleaner than the British amplifiers at a matched
        // output level.
        Gain::Twin => 2.0,
        Gain::Muff => 45.0,
        // Neither of these is used any more -- both amplifiers state their
        // input level instead, see `stated_level` -- and they are left here
        // because the reasoning is the reason they are not used.
        //
        // They were briefly set to 46 and 57, taken from `--peak`. Both
        // figures were the top of that sweep's own range rather than a peak,
        // and calibrating to them cost the drive control most of its travel:
        // measured by `examples/drivecheck.rs`, a 5150 handed 0.66 V is
        // squared off at every setting and its make-up curve spans ten
        // decibels instead of fifty-three. Asking an amplifier to hit a
        // distortion figure is how its input ended up thirty decibels away
        // from a guitar in one direction and twenty-five in the other. See
        // `docs/experiments/high-gain-calibration-targets.md`.
        Gain::Boogie => 40.0,
        Gain::Peavey => 45.0,
        // The Neve 73P is a microphone preamplifier built not to run out of
        // room. Like Console, it should be barely working at nominal.
        Gain::Neve => 3.0,
    }
}

/// What a guitar puts out, in volts, for a nominal digital signal.
///
/// One figure, used by every circuit with a guitar in front of it, because a
/// guitar does not change when it is plugged into a different amplifier. It
/// was measured as the level where the TS808's diode clipping peaks and it is
/// also simply where a guitar is: a hundred millivolts and up, depending on
/// the pickups and how hard it is hit. Nominal here is -18 dBFS, so a
/// full-scale strum arrives at about 0.97 V.
///
/// Before this the four guitar circuits disagreed about a guitar by
/// fifty-four decibels -- 0.004 V at the 5150, 2.13 V at the Mark IIC+ --
/// because each level was inferred from how much its own circuit distorted
/// rather than stated. See `docs/experiments/guitar-input-levels.md`.
const GUITAR_VOLTS: f64 = 0.122;

/// The input level a circuit is simply *told*, rather than one searched for
/// from a distortion figure.
///
/// The search below assumes distortion rises to one peak and falls away, and
/// bisects on the rising side of it. That held for everything here until the
/// TS808 was measured properly, because the TS808 has two clipping regimes and
/// therefore two peaks: its diodes are in the feedback loop, so its minimum
/// gain is one and it reaches 21 % around a tenth of a volt and falls back to
/// 12 % by a volt -- and then the op-amp starts meeting its own 9 V rail and
/// it climbs again. Asked for 21 %, the bisection walked straight past the
/// first peak and calibrated the pedal at four volts.
///
/// A stated level is also the better answer on its own terms. `CLAUDE.md`
/// §24.6 asks for explicit reference points -- input connector level, stage
/// levels, output level -- and a guitar's output is a fact about a guitar
/// rather than something to be inferred from how much a pedal distorts.
///
/// Every circuit with a guitar in front of it now states one. The two
/// amplifiers could not until their gain structure had been traced stage by
/// stage against their drawings, which `docs/experiments/markiic-5150-trace.md`
/// records; that is done, so they do.
fn stated_level(gain: Gain) -> Option<f64> {
    match gain {
        // A guitar into a Tube Screamer. It was 15 mV, inferred from a target
        // of 12 %, and at 15 mV three quarters of the Drive control did
        // nothing at all: 0.0 % at a quarter turn, 0.1 % at half, 0.7 % at
        // three quarters.
        Gain::Screamer => Some(GUITAR_VOLTS),
        Gain::Twin => Some(GUITAR_VOLTS),
        // A guitar into the front of an amplifier. The same guitar.
        //
        // The Mark IIC+ was at 2.13 V, which is not a guitar and is not
        // anything else either -- it is the level at which a preamplifier
        // whose treble capacitor was four times too large and whose V4A was
        // fully bypassed happened to make forty per cent distortion. Both are
        // corrected, and the corrected model at a guitar's level uses the
        // whole of its Lead Drive: measured by `examples/drivecheck.rs`, 0 %
        // shut and about 17 % open, rising smoothly through the middle.
        Gain::Boogie => Some(GUITAR_VOLTS),
        // The 5150 was at 0.004 V, thirty decibels below a guitar. At a
        // guitar's level this amplifier is squared off from about an eighth
        // of a turn of ULTRA PRE upwards and stays there, which a sine's
        // total harmonic distortion reports as one flat number from there to
        // the stop. That is not a defect being calibrated away: six triodes
        // with 118 dB of small-signal gain between the jack and the tone
        // stack is what the drawing has, each stage agrees with the
        // arithmetic on its own components, and a 5150 lead channel is the
        // byword for an amplifier that is fully saturated at 3 on the dial.
        // What a single tone cannot see is that the *spectrum* keeps moving
        // after the total stops. Noted as a remaining discrepancy rather than
        // tuned out.
        Gain::Peavey => Some(GUITAR_VOLTS),
        // And the four typology voices, which are guitar circuits with a
        // guitar in front of them like everything above.
        //
        // These were the last ones still having their input level inferred
        // from a distortion figure, and it had starved every one of them.
        // High Gain was handed 0.0021 V -- thirty-five decibels below a
        // guitar -- because that is the level at which three cascaded valve
        // stages happen to read forty per cent on a sine. Crunch got 0.085 V
        // and spent three quarters of its control going from 0.8 % to 4.7 %.
        // Overdrive got 0.025 V, fourteen decibels down, and was clean for
        // the first quarter of its knob. Measured in
        // `examples/starved.rs`; at a guitar's level Crunch's control is
        // worth 29.5 points of distortion instead of 12.3 and reaches 30.6 %
        // rather than 13 %.
        //
        // The reasoning is the same one written above for the Screamer and
        // the two amplifiers, and it is `CLAUDE.md` section 24.6: a guitar's
        // output is a fact about a guitar, not something to be inferred from
        // how much a circuit distorts. Nothing in a guitar changes when it is
        // plugged into a different pedal.
        Gain::Crunch => Some(GUITAR_VOLTS),
        Gain::Overdrive => Some(GUITAR_VOLTS),
        Gain::Distortion => Some(GUITAR_VOLTS),
        // Fully saturated across most of its control at a guitar's level, and
        // its total harmonic distortion then falls as the knob comes up --
        // 81.9 % shut, 49.1 % open -- because past saturation a sine's THD
        // stops measuring how distorted the circuit is and starts measuring
        // what has happened to the fundamental. Three cascaded valve stages
        // run hot *are* saturated by a guitar; the same is recorded for the
        // 5150 above. Noted as a remaining discrepancy in what the figure can
        // see, not tuned out by starving the circuit.
        Gain::HighGain => Some(GUITAR_VOLTS),
        _ => None,
    }
}

/// A voice, which since the amplifiers gained power stages is two circuits
/// rather than one. Measuring only the first would describe a preamplifier
/// that no longer reaches the output on its own.
struct Voice {
    gain: Simulation,
    power: Option<Simulation>,
}

impl Voice {
    fn new(gain: Gain, diode: voice::Diode, amplifier: voice::Amplifier) -> Self {
        Self {
            gain: Simulation::new(
                voice::build_voice(gain, diode, amplifier).expect("catalogue builds"),
                RATE,
            ),
            power: voice::build_power(gain)
                .map(|built| Simulation::new(built.expect("catalogue builds"), RATE)),
        }
    }

    fn set_control(&mut self, which: usize, position: f64) {
        self.gain.set_control(which, position);
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.gain.process(x);
        match self.power {
            Some(ref mut sim) => sim.process(y),
            None => y,
        }
    }
}

fn measured(sim: &mut Voice, volts: f64) -> measure::Measured {
    let tone = Tone::near(RATE, 16_384, 220.0, volts);
    measure::run(tone, (RATE / 10.0) as usize, |x| sim.process(x))
}

fn main() {
    // --peak mode: measure the actual peak THD for Boogie and Peavey, then exit.
    if std::env::args().any(|a| a == "--peak") {
        println!("// Peak THD measurement for circuits with inter-stage attenuation.");
        println!("// These are the actual maximum distortion values, not targets.");
        println!();
        for gain in [Gain::Boogie, Gain::Peavey] {
            let thd_at = |volts: f64| {
                let mut sim = Voice::new(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
                sim.set_control(gain.drive_control(), 1.0);
                measured(&mut sim, volts).thd_percent()
            };
            // The same ceiling the calibration sweep below uses. Looking
            // higher finds figures the calibration cannot reach, and the
            // calibration then falls back to its own top step -- which is not
            // a measurement of anything.
            const STEPS: usize = 100;
            let (lowest, highest) = (1e-4f64, 4.0f64);
            let level = |i: usize| lowest * (highest / lowest).powf(i as f64 / STEPS as f64);
            let mut peak_volts = 0.0f64;
            let mut peak_thd = 0.0f64;
            for i in 0..=STEPS {
                let volts = level(i);
                let thd = thd_at(volts);
                if thd > peak_thd {
                    peak_thd = thd;
                    peak_volts = volts;
                }
            }
            // A peak on the last step is not a peak. Both of these circuits
            // turned out to creep upwards all the way to the ceiling, so the
            // premise this mode was written on -- that inter-stage attenuation
            // makes distortion turn over -- does not hold for either of them,
            // and the figure has to be read as "still rising here".
            let at_ceiling = (peak_volts - highest).abs() < highest * 1e-6;
            println!(
                "{:<12} peak THD: {:.1}% at {:.4} V{}",
                gain.name(),
                peak_thd,
                peak_volts,
                if at_ceiling {
                    "  -- still rising at the ceiling, not a peak"
                } else {
                    ""
                }
            );
        }
        return;
    }

    println!("// Measured by `examples/calibrate.rs`, which states the reasoning.");
    println!("// Do not edit by hand: run the example and paste its output over this");
    println!("// file. `tests/voice.rs` re-measures every entry and fails on drift.");
    println!();
    println!("pub const CALIBRATION: [Calibration; VOICES] = [");

    for index in 0..VOICES {
        let (gain, diode, amplifier) = voice::voice_at(index);
        let target = intent(gain);

        // Find the level that gives the intended distortion.
        //
        // Not by bisection from nothing to plenty, which assumes more level
        // always means more distortion. That is true of a clipper to ground,
        // whose ceiling squares the wave off harder the further past it you
        // go, and it is *false* of one in the loop: its minimum gain is one,
        // so a large enough input arrives at the output nearly untouched with
        // the clipped part added on top, and the clipped part is then a
        // shrinking fraction of what comes out. Measured, germanium diodes in
        // a loop peak near 14 % around a tenth of a volt and are back to 6 %
        // by four volts -- so a plain bisection walked off the top of its
        // range and calibrated the voice at four volts.
        //
        // So: sweep first, find the peak, and only then bisect on the rising
        // side of it, where the search is sound. Where a voice cannot reach
        // its intended figure at all, take the peak and say so.
        let thd_at = |volts: f64| {
            let mut sim = Voice::new(gain, diode, amplifier);
            sim.set_control(gain.drive_control(), 1.0);
            measured(&mut sim, volts).thd_percent()
        };
        const STEPS: usize = 60;
        let (lowest, highest) = (1e-4f64, 4.0f64);
        let level = |i: usize| lowest * (highest / lowest).powf(i as f64 / STEPS as f64);
        let mut peak = (0usize, 0.0f64);
        for i in 0..=STEPS {
            let thd = thd_at(level(i));
            if thd > peak.1 {
                peak = (i, thd);
            }
        }
        let mut reachable = peak.1 >= target;
        let drive_volts = if let Some(stated) = stated_level(gain) {
            reachable = true;
            stated
        } else {
            let (mut lo, mut hi) = (lowest, level(peak.0));
            if reachable {
                for _ in 0..30 {
                    let mid = (lo * hi).sqrt();
                    if thd_at(mid) < target {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
            } else {
                lo = level(peak.0);
            }
            (lo * hi).sqrt()
        };

        // With the level fixed, what does the circuit do to it across the
        // control?
        let mut make_up = [0.0f64; POINTS];
        for (i, slot) in make_up.iter_mut().enumerate() {
            let mut sim = Voice::new(gain, diode, amplifier);
            sim.set_control(gain.drive_control(), voice::knot_position(i));
            *slot = -measured(&mut sim, drive_volts).gain_db();
        }

        let mut check = Voice::new(gain, diode, amplifier);
        check.set_control(gain.drive_control(), 1.0);
        let got = measured(&mut check, drive_volts);
        let part = if gain.has_diodes() {
            format!("{} diodes", diode.name().to_lowercase())
        } else if gain.has_amplifier() {
            {
                let name = amplifier.name().to_lowercase();
                let article = if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
                    "an"
                } else {
                    "a"
                };
                format!("{article} {name}")
            }
        } else {
            // Whatever does the amplifying, for the voices where it is not a
            // control. Saying "a valve" for all of them was wrong for four of
            // the five: a Screamer is an op-amp, a Muff and a 73P are
            // transistors, and only the two amplifiers have bottles in them.
            String::from(match gain {
                Gain::Screamer => "an op-amp",
                Gain::Muff | Gain::Neve => "transistors",
                _ => "a valve",
            })
        };
        println!(
            "    // {} with {}: {:.4} V in, {:.1} % distortion, {:.1} % third.{}",
            gain.name(),
            part,
            drive_volts,
            got.thd_percent(),
            got.harmonic_percent(3),
            if reachable {
                ""
            } else {
                " This voice cannot reach\n    // its intended figure at any level, so this is its peak."
            },
        );
        println!("    Calibration {{");
        println!("        drive_volts: {drive_volts:.6},");
        print!("        make_up_db: [");
        for (i, m) in make_up.iter().enumerate() {
            print!("{}{m:.2}", if i > 0 { ", " } else { "" });
        }
        println!("],");
        println!("    }},");
    }
    println!("];");

    // The iron stage sits after the make-up and is handed volts, so its trim
    // is what brings a small signal back to where it went in. Measured small
    // deliberately: at a level where the core is still linear, this is the
    // transformer's own insertion loss and nothing else. Anything it does
    // above that is the sound and must not be normalised away.
    println!();
    println!("/// Insertion loss of each output transformer, measured where the");
    println!("/// core is still linear. What it does above that is the sound.");
    print!("pub const IRON_TRIM: [f64; 3] = [");
    for (i, material) in [Iron::Nickel, Iron::Steel, Iron::Amorphous]
        .into_iter()
        .enumerate()
    {
        let netlist = voice::build_iron(material).expect("catalogue builds");
        let mut sim = Simulation::new(netlist, RATE);
        let volts = 0.001 * IRON_VOLTS;
        let tone = Tone::near(RATE, 16_384, 1000.0, volts);
        let m = measure::run(tone, (RATE / 4.0) as usize, |x| sim.process(x));
        let trim = 1.0 / 10f64.powf(m.gain_db() / 20.0);
        print!("{}{trim:.6}", if i > 0 { ", " } else { "" });
    }
    println!("];");

    eprintln!("nominal is {NOMINAL_DBFS} dBFS");
}
