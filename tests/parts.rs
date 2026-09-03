//! The two parts a preamplifier is made of that a guitar amplifier is not:
//! a junction FET, and iron that saturates.

use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::netlist::{Circuit, CoreSpec, JfetSpec, Netlist};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

fn core_at(spec: CoreSpec, source: f64) -> Circuit {
    let mut net = Netlist::new("core");
    net.input("w", source)
        .core("w", "gnd", spec)
        .resistor("w", "gnd", 100_000.0);
    net.build("w").expect("builds")
}

fn jfet_stage(spec: JfetSpec) -> Circuit {
    let mut net = Netlist::new("jfet stage");
    net.input("in", 10_000.0)
        .capacitor("in", "gate", 100e-9)
        .resistor("gate", "gnd", 1_000_000.0)
        .supply("drain", 22_000.0, 24.0)
        // Self-biased: an n-channel JFET conducts with no bias at all, so the
        // source resistor is what turns it down to where it can swing.
        .resistor("source", "gnd", 1_000.0)
        .capacitor("source", "gnd", 100e-6)
        .jfet("drain", "gate", "source", spec)
        .capacitor("drain", "out", 100e-9)
        .resistor("out", "gnd", 470_000.0);
    net.build("out").expect("builds")
}

fn run(c: &Circuit, hz: f64, volts: f64) -> measure::Measured {
    let mut sim = Simulation::new(c.clone(), RATE);
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 4.0) as usize, |x| sim.process(x))
}

/// A square law has one term, so a JFET run where it is still square makes
/// second harmonic and essentially nothing else. That is the whole reason
/// discrete studio equipment is built from them, and it is the thing that
/// separates a JFET from the diodes elsewhere in this plugin -- a symmetric
/// pair of those cannot make an even harmonic at all.
#[test]
fn a_jfet_makes_second_harmonic_and_little_else() {
    let stage = jfet_stage(JfetSpec::J2N5457);
    for volts in [0.01, 0.05] {
        let m = run(&stage, 220.0, volts);
        assert!(
            m.harmonic_percent(2) > 0.2,
            "{volts} V should bend: {:.2} % second",
            m.harmonic_percent(2)
        );
        assert!(
            m.harmonic_percent(3) < m.harmonic_percent(2) / 10.0,
            "{volts} V made {:.2} % third against {:.2} % second, which is not \
             a square law",
            m.harmonic_percent(3),
            m.harmonic_percent(2)
        );
    }
}

/// More in, more out, until it runs out of supply to swing in.
#[test]
fn a_jfet_bends_further_the_harder_it_is_driven() {
    let stage = jfet_stage(JfetSpec::J2N5457);
    let mut last = 0.0;
    for volts in [0.01, 0.05, 0.2, 0.6] {
        let thd = run(&stage, 220.0, volts).thd_percent();
        assert!(
            thd > last,
            "{volts} V made {thd:.2} %, less than the {last:.2} % before it"
        );
        last = thd;
    }
    assert!(last > 20.0, "driven hard it should be well bent: {last:.1} %");
}

/// The parts differ, and by the thing they are chosen for.
#[test]
fn the_jfets_are_not_the_same_part() {
    let gain = |spec: JfetSpec| run(&jfet_stage(spec), 220.0, 0.01).gain_db();
    let small = gain(JfetSpec::J201);
    let large = gain(JfetSpec::J113);
    assert!(
        (small - large).abs() > 3.0,
        "a J201 and a J113 should not be interchangeable: {small:.1} dB \
         against {large:.1} dB"
    );
}

/// The one that matters, and the reason iron is not a waveshaper.
///
/// It is flux that saturates, and flux is the integral of voltage -- so the
/// same signal makes far more of it at thirty hertz than at five hundred. No
/// curve applied to the samples can behave that way: a waveshaper given the
/// same waveform gives the same answer whatever rate it arrives at.
#[test]
fn a_core_saturates_on_flux_and_so_on_frequency() {
    let steel = core_at(CoreSpec::STEEL, 600.0);
    let bottom = run(&steel, 30.0, 12.0).thd_percent();
    let middle = run(&steel, 120.0, 12.0).thd_percent();
    let top = run(&steel, 2000.0, 12.0).thd_percent();
    println!("30 Hz {bottom:.1} %, 120 Hz {middle:.2} %, 2 kHz {top:.3} %");
    assert!(
        bottom > middle * 5.0,
        "the bottom end should saturate far harder: {bottom:.1} % at 30 Hz \
         against {middle:.2} % at 120"
    );
    assert!(
        top < 0.1,
        "the top of the band should pass clean: {top:.2} % at 2 kHz"
    );
}

/// A core is symmetric, so it works both halves of the wave the same way and
/// cannot make an even harmonic. A single-ended valve stage is the opposite,
/// and that is why the two are worth having in one plugin.
#[test]
fn a_core_makes_odd_harmonics() {
    let m = run(&core_at(CoreSpec::STEEL, 600.0), 40.0, 12.0);
    assert!(
        m.harmonic_percent(2) < 0.5,
        "a symmetric core made {:.2} % second harmonic",
        m.harmonic_percent(2)
    );
    assert!(
        m.harmonic_percent(3) > 5.0,
        "and should be making third: {:.2} %",
        m.harmonic_percent(3)
    );
}

/// Past the knee the curve has to keep going. Clamped instead, more drive
/// gives *less* distortion once everything is against the stop -- which is
/// how the previous attempt at this ended up with a saturation control that
/// worked backwards at the top of its range.
#[test]
fn a_core_goes_on_bending_past_the_knee() {
    let steel = core_at(CoreSpec::STEEL, 600.0);
    let mut last = 0.0;
    for volts in [4.0, 12.0, 30.0, 60.0] {
        let thd = run(&steel, 40.0, volts).thd_percent();
        assert!(
            thd > last,
            "{volts} V made {thd:.1} %, less than the {last:.1} % before it: \
             the curve is clamping rather than continuing"
        );
        last = thd;
    }
}

/// And it limits what comes out, which is the other half of what saturating
/// means: the output rises far more slowly than the input does.
#[test]
fn a_core_holds_its_output_back() {
    let steel = core_at(CoreSpec::STEEL, 600.0);
    let quiet = run(&steel, 40.0, 4.0).fundamental().magnitude();
    let loud = run(&steel, 40.0, 60.0).fundamental().magnitude();
    let ratio = loud / quiet;
    println!("fifteen times the input gave {ratio:.2} times the output");
    assert!(
        ratio < 5.0,
        "a saturating core should hold its output back: fifteen times in gave \
         {ratio:.1} times out"
    );
}

/// The materials have to differ, and they differ by more than how much.
///
/// Measured across level rather than at one point, because the ordering is not
/// the same everywhere and the point of having three is that they are not one
/// another turned up. Nickel is the only one that does anything at all at a
/// volt, and it is also the softest when pushed -- it bends earliest and least
/// steeply. Steel stays out of the way and then arrives hard. Amorphous stays
/// out of the way for much longer.
///
/// A single measurement at one level would have called nickel "less
/// distortion than steel" and stopped there, which is true at eight volts and
/// false at one.
#[test]
fn the_cores_are_not_the_same_iron() {
    let at = |spec: CoreSpec, volts: f64| run(&core_at(spec, 600.0), 40.0, volts).thd_percent();

    // Quietly, nickel is the only one working.
    let quiet = (
        at(CoreSpec::STEEL, 1.0),
        at(CoreSpec::NICKEL, 1.0),
        at(CoreSpec::AMORPHOUS, 1.0),
    );
    println!("at 1 V: steel {:.3} %, nickel {:.3} %, amorphous {:.3} %", quiet.0, quiet.1, quiet.2);
    assert!(
        quiet.1 > quiet.0 * 5.0 && quiet.1 > quiet.2 * 5.0,
        "nickel should be the one that colours a quiet signal: {:.3} % against \
         steel's {:.3} % and amorphous's {:.3} %",
        quiet.1,
        quiet.0,
        quiet.2
    );

    // Driven hard, steel overtakes it: a harder corner, and it goes further.
    let loud = (
        at(CoreSpec::STEEL, 32.0),
        at(CoreSpec::NICKEL, 32.0),
        at(CoreSpec::AMORPHOUS, 32.0),
    );
    println!("at 32 V: steel {:.1} %, nickel {:.1} %, amorphous {:.1} %", loud.0, loud.1, loud.2);
    assert!(
        loud.0 > loud.1 * 1.5,
        "steel should go further than nickel when pushed: {:.1} % against {:.1} %",
        loud.0,
        loud.1
    );

    // And amorphous is the one you put in when you want the iron to stay out
    // of it: still clean where the other two are well into it.
    assert!(
        at(CoreSpec::AMORPHOUS, 8.0) < at(CoreSpec::STEEL, 8.0) / 10.0,
        "amorphous should still be clean at eight volts: {:.2} % against \
         steel's {:.2} %",
        at(CoreSpec::AMORPHOUS, 8.0),
        at(CoreSpec::STEEL, 8.0)
    );
}

/// A device that remembers something has to be told when the sample is over.
///
/// `advance` sat on the trait uncalled for a long time and nothing noticed,
/// because every device before the core was memoryless -- a diode's current
/// depends on its voltage now and on nothing else. The core integrates, and
/// unadvanced its flux never accumulated past a single sample: iron in the
/// signal path did nothing whatsoever, silently and at every setting.
#[test]
fn a_core_accumulates_flux_across_samples() {
    let steel = core_at(CoreSpec::STEEL, 600.0);
    let mut sim = Simulation::new(steel, RATE);
    // Half a cycle of 40 Hz, which should build real flux and pull the
    // terminal voltage down as the core runs out of room.
    let samples = (RATE / 80.0) as usize;
    let mut lowest: f64 = f64::MAX;
    for n in 0..samples {
        let phase = std::f64::consts::PI * n as f64 / samples as f64;
        let out = sim.process(30.0 * phase.sin());
        lowest = lowest.min(out / (30.0 * phase.sin()).max(1e-9));
    }
    assert!(
        lowest < 0.5,
        "the core never loaded the source, so its flux is not accumulating: \
         the worst the winding was pulled down to was {lowest:.3} of the drive"
    );
}
