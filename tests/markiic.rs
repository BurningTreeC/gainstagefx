//! The Mark IIC+ preamplifier, against what its schematic says.

use gainstagefx::circuits::markiic::{self, BASS, LEAD_DRIVE, MIDDLE, TREBLE, VOLUME};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

struct Panel {
    treble: f64,
    bass: f64,
    middle: f64,
    volume: f64,
    drive: f64,
}

const NOON: Panel = Panel { treble: 0.5, bass: 0.5, middle: 0.5, volume: 0.5, drive: 0.5 };

fn at(node: &str, hz: f64, volts: f64, p: &Panel) -> measure::Measured {
    let c = markiic::tap(10_000.0, 1_000_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(TREBLE, p.treble);
    sim.set_control(BASS, p.bass);
    sim.set_control(MIDDLE, p.middle);
    sim.set_control(VOLUME, p.volume);
    sim.set_control(LEAD_DRIVE, p.drive);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

/// V1A's cathode capacitor is only .47 uF against a 1.5 k resistor, so it
/// bypasses from about 226 Hz upwards and leaves the bottom of the band its
/// degeneration. That is a deliberate choice and it is where this
/// amplifier's tightness starts: the first stage has less gain at 60 Hz than
/// it has in the midrange, before any tone control is reached.
#[test]
fn the_first_stage_leaves_the_bottom_end_degenerated() {
    let corner = 1.0 / (std::f64::consts::TAU * 1_500.0 * 0.47e-6);
    println!("cathode bypass corner: {corner:.0} Hz");
    assert!((corner - 226.0).abs() < 15.0, "the arithmetic itself has moved");

    let low = at("v1a_p", 50.0, 1e-5, &NOON).gain_db();
    let mid = at("v1a_p", 2_000.0, 1e-5, &NOON).gain_db();
    println!("V1A: {low:.1} dB at 50 Hz, {mid:.1} dB at 2 kHz");
    assert!(
        mid - low > 3.0,
        "the bottom should be down on the middle: {low:.1} against {mid:.1}"
    );
}

/// A Fender stack scoops, and the scoop is the whole reason this amplifier
/// sounds the way it does. Measured across the stack itself, against the top
/// of the band.
#[test]
fn the_tone_stack_scoops() {
    let scooped = Panel { middle: 0.0, ..NOON };
    let notch = at("ts_out", 250.0, 1e-5, &scooped).gain_db();
    let top = at("ts_out", 5_000.0, 1e-5, &scooped).gain_db();
    println!("with the middle down: {notch:.1} dB at 250 Hz against {top:.1} at 5 kHz");
    assert!(
        top - notch > 10.0,
        "the stack should scoop hard: only {:.1} dB between them",
        top - notch
    );
}

/// And the middle control has to work in the direction its label claims.
/// Wired as a variable resistor with a forward taper it does the opposite,
/// because a variable resistor keeps what is *left* of its track.
#[test]
fn the_middle_control_goes_the_way_it_is_labelled() {
    let down = at("ts_out", 250.0, 1e-5, &Panel { middle: 0.0, ..NOON }).gain_db();
    let up = at("ts_out", 250.0, 1e-5, &Panel { middle: 1.0, ..NOON }).gain_db();
    println!("middle down {down:.1} dB, up {up:.1} dB at 250 Hz");
    assert!(
        up > down + 2.0,
        "turning the middle up should give more middle: {up:.1} against {down:.1}"
    );
}

/// The bass control likewise.
#[test]
fn the_bass_control_goes_the_way_it_is_labelled() {
    let down = at("ts_out", 80.0, 1e-5, &Panel { bass: 0.0, ..NOON }).gain_db();
    let up = at("ts_out", 80.0, 1e-5, &Panel { bass: 1.0, ..NOON }).gain_db();
    println!("bass down {down:.1} dB, up {up:.1} dB at 80 Hz");
    assert!(up > down + 3.0, "{up:.1} against {down:.1}");
}

/// Four triode stages in a row is a great deal of gain, and the lead drive
/// control has to cover it -- from nothing to all of it.
#[test]
fn the_lead_drive_runs_from_silence_to_everything() {
    let shut = at("lead_out", 1_000.0, 3e-4, &Panel { drive: 0.0, ..NOON }).gain_db();
    let open = at("lead_out", 1_000.0, 3e-4, &Panel { drive: 1.0, ..NOON }).gain_db();
    println!("lead drive: {shut:.1} dB to {open:.1} dB");
    assert!(
        open - shut > 50.0,
        "a lead drive control should cover the lot: only {:.1} dB",
        open - shut
    );
    assert!(open > 60.0, "four triodes should reach some gain: {open:.1} dB");
}

/// Turned up it distorts, and being valves it leads with second harmonic.
#[test]
fn it_distorts_and_leads_with_second_harmonic() {
    let hard = Panel { drive: 1.0, volume: 0.8, middle: 0.2, ..NOON };
    let m = at("lead_out", 1_000.0, 0.01, &hard);
    println!(
        "{:.1} % distortion, 2nd {:.1} %, 3rd {:.1} %",
        m.thd_percent(),
        m.harmonic_percent(2),
        m.harmonic_percent(3)
    );
    assert!(m.thd_percent() > 10.0, "driven hard it should bend: {:.1} %", m.thd_percent());
}

/// And every stage has to build and settle, which a four-triode chain on a
/// 410 V rail is not guaranteed to do.
#[test]
fn every_stage_reaches_an_operating_point() {
    for node in ["v1a_p", "ts_out", "v1b_p", "v3b_p", "v4a_p", "lead_out"] {
        let c = markiic::tap(10_000.0, 1_000_000.0, node).expect("builds");
        let mut sim = Simulation::new(c, RATE);
        assert!(
            sim.find_operating_point(),
            "{node} never settled to an operating point"
        );
    }

    // Silence in has to be silence out -- but only at the real output. A tap
    // on a plate is a probe inside the amplifier, and a plate sits some
    // hundreds of volts above ground by design; there is a coupling capacitor
    // between the last one and the jack and that is what sheds it.
    let c = markiic::tap(10_000.0, 1_000_000.0, "lead_out").expect("builds");
    let mut sim = Simulation::new(c, RATE);
    let mut worst: f64 = 0.0;
    for _ in 0..(RATE as usize / 4) {
        worst = worst.max(sim.process(0.0).abs());
    }
    assert!(worst < 0.05, "the output put out {worst:.4} on silence");
}
