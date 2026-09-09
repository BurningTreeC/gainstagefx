//! How big each voice's matrix is, since a dense factorisation costs its cube.
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};
fn main() {
    for g in [
        Gain::Peavey,
        Gain::Boogie,
        Gain::Neve,
        Gain::Muff,
        Gain::Screamer,
        Gain::Crunch,
        Gain::Clean,
        Gain::HighGain,
        Gain::Distortion,
    ] {
        let n = voice::build_voice(g, voice::Diode::Silicon, voice::Amplifier::Valve).unwrap();
        let sim = Simulation::new(n, 48_000.0);
        let u = sim.unknowns();
        println!(
            "{:<12} {:>3} unknowns   n^3 = {:>7}",
            g.name(),
            u,
            u * u * u
        );
    }
}
