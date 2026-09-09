//! The Twin Reverb's power amplifier, which is where the deadline misses are.
//!
//! Every other voice meets its deadline on real playing. This one misses 3.6 %
//! of callbacks with a worst of seven and a half milliseconds, and the reason
//! is not that it is large -- it is 27 unknowns like the others -- but that its
//! solve *fails*: 3713 solves a second run out of passes, and the line search
//! backtracks 120,193 times to find a step that improves the residual, giving
//! up on 5450 of them.
//!
//! An unsettled solve is the worst of both worlds. It costs the whole
//! iteration budget and returns the least accurate answer of any sample, and
//! then hands the next sample a worse place to start from -- which is how one
//! bad sample becomes a bad block.
//!
//! `cargo run --release --example thrash`

use gainstagefx::circuits::power::{self, PowerSpec};
use gainstagefx::dsp::time::Simulation;
use gainstagefx::voice::{self, Gain};

const RATE: f64 = 48_000.0;

fn read(path: &str) -> Vec<f64> {
    let b = std::fs::read(path).expect("the take");
    let mut at = 12usize;
    let mut d = (0usize, 0usize);
    while at + 8 <= b.len() {
        let size = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
        if &b[at..at + 4] == b"data" {
            d = (at + 8, size.min(b.len() - at - 8));
        }
        at += 8 + size + (size & 1);
    }
    (0..d.1 / 3)
        .map(|i| {
            let j = d.0 + i * 3;
            (i32::from_le_bytes([0, b[j], b[j + 1], b[j + 2]]) >> 8) as f64 / 8_388_608.0
        })
        .collect()
}

/// What the channel sends the power amplifier, at the level the chain sends it.
fn channel(gain: Gain, x: &[f64]) -> Vec<f64> {
    let net = voice::build_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve).unwrap();
    let mut sim = Simulation::new(net, RATE);
    sim.set_control(gain.drive_control(), 1.0);
    sim.find_operating_point();
    // `Chain::into` for these voices: 0.122 V for a nominal -18 dBFS signal.
    x.iter().map(|&v| sim.process(v * 0.969)).collect()
}

fn run(label: &str, spec: &PowerSpec, drive: &[f64]) {
    // The source impedance the chain builds it with. Getting this wrong is a
    // trap worth naming: at 100 k this stage shows two unsettled solves and at
    // the 10 k it actually sees, 3713.
    let mut sim = Simulation::new(power::build(spec, 10_000.0).unwrap(), RATE);
    sim.find_operating_point();
    for &v in drive.iter().take(2000) {
        std::hint::black_box(sim.process(v));
    }
    let (s0, p0, u0, _) = sim.statistics();
    let (b0, f0, n0) = sim.health();
    let start = std::time::Instant::now();
    for &v in drive.iter().skip(2000) {
        std::hint::black_box(sim.process(v));
    }
    let seconds = start.elapsed().as_secs_f64();
    let (s1, p1, u1, _) = sim.statistics();
    let (b1, f1, n1) = sim.health();
    let solves = (s1 - s0).max(1);
    println!(
        "  {label:<16}{:>7.1} % rt{:>8.2} passes{:>9} unsettled{:>10} backtr{:>8} fallb{:>6} nan",
        100.0 * seconds / (solves as f64 / RATE),
        (p1 - p0) as f64 / solves as f64,
        u1 - u0,
        b1 - b0,
        f1 - f0,
        n1 - n0,
    );
}

fn main() {
    let x = read("/home/simon/Documents/REAPER Media/Experiments/Media/02-260904_2255.wav");
    // The loudest six seconds, which is where the misses are.
    let want = 6 * RATE as usize;
    let mut best = (0usize, -1.0f64);
    let mut a = 0;
    while a + want <= x.len() {
        let e: f64 = x[a..a + want].iter().map(|v| v * v).sum();
        if e > best.1 {
            best = (a, e);
        }
        a += 12_000;
    }
    let cut = &x[best.0..best.0 + want];
    println!("Power amplifiers, driven by their own channel at Drive 10.\n");
    run("Twin Reverb", &PowerSpec::TWIN, &channel(Gain::Twin, cut));
    run(
        "Mark IIC+",
        &PowerSpec::MARKIIC,
        &channel(Gain::Boogie, cut),
    );
    run("5150", &PowerSpec::EVH5150, &channel(Gain::Peavey, cut));
}
