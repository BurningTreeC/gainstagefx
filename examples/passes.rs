use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, CALIBRATION};

fn main() {
    let tol: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1e-6);
    println!("(tolerance is compiled in; this run reports what it is doing) tol arg {tol}");
    for g in [voice::Gain::Clean, voice::Gain::Crunch, voice::Gain::HighGain, voice::Gain::Distortion] {
        let i = voice::voice_index(g, voice::Diode::Silicon, voice::Amplifier::Valve);
        let netlist = voice::build_voice(g, voice::Diode::Silicon, voice::Amplifier::Valve).unwrap();
        let mut sim = Simulation::new(netlist, 192_000.0);
        sim.set_control(0, 0.7);
        let volts = CALIBRATION[i].drive_volts;
        for k in 0..200_000 {
            sim.process(volts * (k as f64 * 0.01).sin());
        }
        let (solves, passes, unsettled, _) = sim.statistics();
        println!("{:<12} {:.2} passes/sample, {unsettled} unsettled", g.name(), passes as f64 / solves as f64);
    }
}
