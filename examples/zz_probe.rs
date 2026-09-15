use gainstagefx::voice::{Chain, Gain, PowerAmp, Settings, Tone, Cabinet};
const RATE: f64 = 48_000.0;
const BLOCK: usize = 128;
fn stimulus(k: usize, amp: f64) -> f64 {
    let t = k as f64 / RATE;
    let env = (-((t % 0.5) * 6.0)).exp();
    amp * env * ((std::f64::consts::TAU * 110.0 * t).sin() + 0.6 * (std::f64::consts::TAU * 330.0 * t).sin() + 0.3 * (std::f64::consts::TAU * 1757.0 * t).sin())
}
fn run(gain: Gain, power_amp: PowerAmp, drive: f64, master: f64, amp: f64) {
    let s = Settings { gain, power_amp, drive, master, tone: Tone::Wide, cabinet: Cabinet::Stack, ..Settings::default() };
    let mut chain = Chain::new(RATE);
    chain.apply(&s); chain.settle(); chain.find_operating_point();
    let blocks = (RATE * 4.0) as usize / BLOCK;
    let deadline = BLOCK as f64 / RATE * 1e6;
    let mut times = Vec::new();
    let mut k = 0; let mut peak = 0.0f64;
    let before = chain.solver_breakdown();
    for _ in 0..blocks {
        chain.apply(&s);
        let start = std::time::Instant::now();
        for _ in 0..BLOCK { let y = chain.process(stimulus(k, amp)); peak = peak.max(y.abs()); k += 1; }
        times.push(start.elapsed().as_secs_f64() * 1e6);
    }
    let w = chain.solver_breakdown().saturating_delta(before);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let mut sorted = times.clone(); sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let misses = times.iter().filter(|t| **t > deadline).count();
    println!("{gain:?}->{power_amp:?} drive {drive} master {master} amp {amp}: mean {:.0}% p99 {:.0}us max {:.0}us misses {misses}/{} | power p/s {:.2} unsettled {} fallbacks {} backtracks {} | gain p/s {:.2} uns {} | peak {peak:.2}",
        mean / deadline * 100.0, sorted[sorted.len()*99/100], sorted[sorted.len()-1], times.len(),
        w.power.passes_per_solve(), w.power.unsettled, w.power.fallbacks, w.power.backtracks, w.gain.passes_per_solve(), w.gain.unsettled);
}
fn main() {
    for amp in [0.1, 0.9] {
        run(Gain::Peavey, PowerAmp::American6L6Clean, 0.5, 0.5, amp);
        run(Gain::Peavey, PowerAmp::American6L6Clean, 0.9, 0.9, amp);
        run(Gain::HighGain, PowerAmp::American6L6Clean, 0.9, 0.5, amp);
        run(Gain::Boogie, PowerAmp::American6L6Clean, 0.9, 0.9, amp);
        run(Gain::Twin, PowerAmp::Matched, 0.5, 0.5, amp);
    }
}
