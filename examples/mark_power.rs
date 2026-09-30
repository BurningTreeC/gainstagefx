//! The Mark IIC+'s inverter and loop against the redraws (2026-09-30).
//!
//! What to ask of a rebuilt inverter, in order: where it sits, whether its two
//! phases come out the same size, what the loop does to the whole stage, and
//! what the Presence knob does at the speaker. The loop's gain is the stage's
//! gain against the same stage with the loop cut (R61 made a gigohm and C61
//! taken out), both small-signal.
//!
//! `cargo run --release --example mark_power`
use gainstagefx::circuits::power::{self, PowerSpec, SeriesLoop, PRESENCE};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;

const RATE: f64 = 96_000.0;

/// The loop cut: C61 as good as absent (R61 goes to a gigohm below).
const CUT: SeriesLoop = SeriesLoop {
    shelf_cap: 1e-18,
    ..SeriesLoop::MARK_IIC_PLUS
};

fn gain_db(spec: &PowerSpec, at: &str, presence: f64, volts: f64, hz: f64) -> (f64, f64) {
    let circuit = power::tap(spec, 1_000.0, at).expect("builds");
    let mut sim = Simulation::new(circuit, RATE);
    sim.set_control(PRESENCE, presence);
    sim.find_operating_point();
    let tone = Tone::near(RATE, 16_384, hz, volts);
    let m = measure::run(tone, (RATE / 8.0) as usize, |x| sim.process(x));
    (m.gain_db(), m.thd_percent())
}

fn main() {
    let spec = &PowerSpec::MARKIIC;
    let circuit = power::build(spec, 1_000.0).expect("builds");
    let names = circuit.names.clone();
    let mut sim = Simulation::new(circuit, RATE);
    println!("operating point settled: {}", sim.find_operating_point());
    let v = sim.operating_point();
    let at = |n: &str| v[names.iter().position(|x| x == n).expect(n)];
    for n in [
        "pi_a", "pi_b", "pi_k", "tail", "tail_k", "pi_g2", "pi_p1", "pi_p2", "ht",
    ] {
        println!("  {n:<7}{:>9.2} V", at(n));
    }
    println!(
        "  V5A grid to cathode {:+.2} V, V5B {:+.2} V; tail {:.2} mA",
        at("pi_a") - at("pi_k"),
        at("pi_g2") - at("pi_k"),
        (at("pi_b") - at("tail")) / spec.pi_tail * 1e3
    );

    println!("\nthe two phases, 1 kHz at 10 mV, presence at half:");
    let (a, _) = gain_db(spec, "pi_p1", 0.5, 0.01, 1_000.0);
    let (b, _) = gain_db(spec, "pi_p2", 0.5, 0.01, 1_000.0);
    println!(
        "  V5A plate {a:+.1} dB, V5B plate {b:+.1} dB: {:+.2} dB apart",
        a - b
    );

    let mut open = *spec;
    open.feedback = 1e9;
    open.series_loop = Some(&CUT);
    let open_loop = |hz: f64| gain_db(&open, "spk", 0.5, 0.01, hz).0;
    println!("\nat the speaker, small signal (10 mV), dB:");
    println!(
        "  {:>12}{:>9}{:>9}{:>9}{:>9}{:>9}",
        "presence", "100 Hz", "500 Hz", "1 kHz", "4 kHz", "8 kHz"
    );
    let bands = [100.0, 500.0, 1_000.0, 4_000.0, 8_000.0];
    for p in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let row: Vec<String> = bands
            .iter()
            .map(|&hz| format!("{:>9.1}", gain_db(spec, "spk", p, 0.01, hz).0))
            .collect();
        println!("  {p:>12.2}{}", row.concat());
    }
    let row: Vec<String> = bands
        .iter()
        .map(|&hz| format!("{:>9.1}", open_loop(hz)))
        .collect();
    println!("  {:>12}{}", "no loop", row.concat());

    println!("\ndriven, 220 Hz, presence at half:");
    for volts in [1.0, 3.0, 10.0, 30.0] {
        let (g, thd) = gain_db(spec, "spk", 0.5, volts, 220.0);
        let rms = volts * 10f64.powf(g / 20.0) / 2f64.sqrt();
        println!(
            "  {volts:>5.1} V in: {rms:>6.2} V rms, {:>5.1} W, {thd:>5.1} % THD",
            rms * rms / spec.speaker
        );
    }
}
