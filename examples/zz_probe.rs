use gainstagefx::acoustics::speaker::{LoadValues, Mounting, SpeakerProfile};
use gainstagefx::circuits::power::{self, Parasitics, PowerSpec};
use gainstagefx::dsp::time::Simulation;
fn run(sim: &mut Simulation, rate: f64, amp: f64, hz: f64) -> String {
    sim.find_operating_point();
    let n = (rate * 0.25) as usize;
    let mut peak = 0.0f64;
    for k in 0..n {
        let y = sim.process(amp * (std::f64::consts::TAU * hz * k as f64 / rate).sin());
        if k > n / 2 { peak = peak.max(y.abs()); }
    }
    let (solves, passes, uns, _) = sim.statistics();
    let (_bt, fb, nf) = sim.health();
    format!("{peak:7.1}V p/s{:5.2} fb{fb:6} uns{uns:5} nf{nf:5}", passes as f64 / solves as f64)
}
fn main() {
    for (sname, spec) in [("EL34", PowerSpec::BRIT_EL34), ("Twin", PowerSpec::TWIN), ("Mark", PowerSpec::MARKIIC), ("5150", PowerSpec::EVH5150)] {
        let d = Parasitics::of(&spec);
        let _ = d; let variants = [("none", Parasitics::NONE), ("pp 470p", Parasitics{primary_cap:470e-12, ..Parasitics::NONE}), ("pp 1n5", Parasitics{primary_cap:1.5e-9, ..Parasitics::NONE}), ("pp 4n7", Parasitics{primary_cap:4.7e-9, ..Parasitics::NONE})];
        let lv = LoadValues::new(&SpeakerProfile::AMERICAN_CERAMIC, &Mounting::BAFFLE, power::speaker_scale(&spec));
        for rate in [48000.0, 192000.0] {
            for (amp, hz) in [(0.5, 82.0), (5.0, 82.0), (40.0, 82.0), (80.0, 1000.0), (40.0, 7000.0)] {
                let mut r = Simulation::new(power::build(&spec, 10_000.0).unwrap(), rate);
                print!("{sname} {rate:6} {amp:2}V {hz:4}Hz | resistor {} ", run(&mut r, rate, amp, hz));
                for (name, par) in &variants {
                    let (c, _) = power::build_with_speaker_and(&spec, 10_000.0, &lv, par).unwrap();
                    let mut sim = Simulation::new(c, rate);
                    print!("| {name} {} ", run(&mut sim, rate, amp, hz));
                }
                println!();
            }
        }
    }
}
