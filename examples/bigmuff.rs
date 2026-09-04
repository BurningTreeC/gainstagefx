use gainstagefx::circuits::bigmuff::{self, SUSTAIN, TONE, VOLUME, Voicing};
use gainstagefx::dsp::measure::{self, Tone};
use gainstagefx::dsp::time::Simulation;
const RATE: f64 = 96_000.0;

fn at(v: &Voicing, node: &str, hz: f64, volts: f64, sus: f64, tone: f64) -> measure::Measured {
    let c = bigmuff::tap(v, 10_000.0, 470_000.0, node).expect("builds");
    let mut sim = Simulation::new(c, RATE);
    sim.set_control(SUSTAIN, sus);
    sim.set_control(TONE, tone);
    sim.set_control(VOLUME, 1.0);
    let t = Tone::near(RATE, 16_384, hz, volts);
    measure::run(t, (RATE / 2.0) as usize, |x| sim.process(x))
}

fn main() {
    let v = bigmuff::RAMS_HEAD;
    println!("=== through the stages, small signal ===");
    for n in ["q4c", "sus", "q3c", "q2c", "wiper", "out"] {
        println!("  {n:<8}{:>8.1} dB", at(&v, n, 1000.0, 1e-4, 1.0, 0.5).gain_db());
    }

    println!("\n=== the tone control: the scoop is the control itself ===");
    print!("{:<12}", "");
    for f in [80.0, 200.0, 500.0, 1000.0, 2000.0, 4000.0] { print!("{f:>8.0}"); }
    println!();
    for (name, t) in [("tone 0", 0.0), ("tone 0.5", 0.5), ("tone 1", 1.0)] {
        print!("{name:<12}");
        let mut vals = Vec::new();
        for f in [80.0, 200.0, 500.0, 1000.0, 2000.0, 4000.0] {
            vals.push(at(&v, "out", f, 1e-4, 1.0, t).gain_db());
        }
        let peak = vals.iter().cloned().fold(f64::MIN, f64::max);
        for x in &vals { print!("{:>8.1}", x - peak); }
        println!();
    }

    println!("\n=== sustain, at playing level ===");
    for s in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let m = at(&v, "out", 1000.0, 0.03, s, 0.5);
        println!("  sustain {s:.2}: {:>6.1} dB, {:>5.1} % distortion, 2nd {:>4.1} 3rd {:>4.1}",
                 m.gain_db(), m.thd_percent(), m.harmonic_percent(2), m.harmonic_percent(3));
    }

    println!("\n=== the versions ===");
    for (name, vv) in [("Ram's Head", bigmuff::RAMS_HEAD), ("Triangle", bigmuff::TRIANGLE),
                       ("Supa Tonebender", bigmuff::SUPA)] {
        let m = at(&vv, "out", 1000.0, 0.03, 1.0, 0.5);
        println!("  {name:<18}{:>6.1} dB, {:>5.1} % distortion", m.gain_db(), m.thd_percent());
    }
}
