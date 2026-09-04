//! The 5150 lead preamp, measured stage by stage.
use gainstagefx::circuits::evh5150::{self, PRE, TONE_STACK_INPUT};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

fn at(node: &str, hz: f64, volts: f64, pre: f64) -> measure::Measured {
    let c = evh5150::tap(10_000.0, TONE_STACK_INPUT, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(PRE, pre);
    let tone = Tone::near(RATE, 16_384, hz, volts);
    measure::run(tone, (RATE / 2.0) as usize, |x| sim.process(x))
}

const NODES: [&str; 7] =
    ["v1a_p", "pre_w", "v1b_p", "v2a_p", "v2b_p", "v5b_p", "stack"];

fn main() {
    println!("=== gain through the chain, small signal, pre gain at noon ===");
    let mut last = 0.0;
    for node in NODES {
        let g = at(node, 1000.0, 1e-6, 0.5).gain_db();
        println!("  {node:<8}{g:>9.1} dB   ({:+.1} on the last)", g - last);
        last = g;
    }

    println!("\n=== the same with the pre gain wide open ===");
    let mut last = 0.0;
    for node in NODES {
        let g = at(node, 1000.0, 1e-6, 1.0).gain_db();
        println!("  {node:<8}{g:>9.1} dB   ({:+.1} on the last)", g - last);
        last = g;
    }

    println!("\n=== the pre gain control, at the output, playing level ===");
    for pre in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let m = at("stack", 1000.0, 0.0003, pre);
        println!(
            "  pre {pre:.2}: {:>7.1} dB, {:>5.1} % distortion, 2nd {:>4.1} 3rd {:>4.1} 5th {:>4.1}",
            m.gain_db(),
            m.thd_percent(),
            m.harmonic_percent(2),
            m.harmonic_percent(3),
            m.harmonic_percent(5)
        );
    }

    println!("\n=== across the band, wide open (what the tone stack is handed) ===");
    print!("{:<12}", "");
    for f in [60.0, 120.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0] {
        print!("{f:>8.0}");
    }
    println!();
    print!("{:<12}", "small signal");
    for f in [60.0, 120.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0] {
        print!("{:>8.1}", at("stack", f, 1e-6, 1.0).gain_db());
    }
    println!();
    print!("{:<12}", "playing");
    for f in [60.0, 120.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0] {
        print!("{:>8.1}", at("stack", f, 0.0003, 1.0).gain_db());
    }
    println!();
}
