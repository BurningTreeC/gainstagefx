//! Octave-band balance of the shipped presets, playing a struck E5 chord through the
//! whole chain. The last column is the two lowest octaves against 1-4 kHz: how heavy
//! a preset is at the bottom.
//!
//! `cargo run --release --example octaves [group or name ...]`

use gainstagefx::presets::{Preset, PRESETS};
use gainstagefx::voice::{Chain, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;

struct Band {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
    energy: f64,
}

impl Band {
    fn new(centre: f64) -> Self {
        let w = std::f64::consts::TAU * centre / RATE;
        let q = std::f64::consts::SQRT_2; // one octave
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Band {
            b: [alpha / a0, 0.0, -alpha / a0],
            a: [-2.0 * w.cos() / a0, (1.0 - alpha) / a0],
            z: [0.0; 2],
            energy: 0.0,
        }
    }
    fn run(&mut self, x: f64) {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        self.energy += y * y;
    }
}

const CENTRES: [f64; 8] = [63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];

fn pluck(seconds: f64) -> Vec<f64> {
    // Karplus-Strong E2, B2, E3, restruck every half second.
    let n = (RATE * seconds) as usize;
    let mut out = vec![0.0; n];
    let mut seed = 12345u64;
    let mut noise = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64) - 1.0
    };
    for hz in [82.41, 123.47, 164.81] {
        let len = (RATE / hz) as usize;
        let mut line = vec![0.0; len];
        for (i, v) in out.iter_mut().enumerate() {
            if i % (RATE as usize / 2) == 0 {
                for s in line.iter_mut() {
                    *s = noise();
                }
            }
            let j = i % len;
            let next = line[(j + 1) % len];
            let y = line[j];
            line[j] = 0.996 * 0.5 * (y + next);
            *v += y / 3.0;
        }
    }
    let peak = out.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    out.iter().map(|v| v / peak).collect()
}

fn balance(preset: &Preset, input: &[f64]) -> Vec<f64> {
    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();
    // Nominal level is an RMS-ish playing level; a pluck peaks about 10 dB above.
    let amplitude = 10f64.powf((NOMINAL_DBFS + 10.0 + preset.input_trim as f64) / 20.0);
    let mut bands: Vec<Band> = CENTRES.iter().map(|&c| Band::new(c)).collect();
    for (i, &x) in input.iter().enumerate() {
        let y = chain.process(amplitude * x);
        if i > RATE as usize / 2 {
            for b in bands.iter_mut() {
                b.run(y);
            }
        }
    }
    let total: f64 = bands.iter().map(|b| b.energy).sum();
    bands
        .iter()
        .map(|b| 10.0 * (b.energy / total).log10())
        .collect()
}

fn row(name: &str, v: &[f64]) {
    print!("{name:<34}");
    for x in v {
        print!("{x:>7.1}");
    }
    // Low (63+125) against the presence region (1k-4k).
    let e = |i: usize| 10f64.powf(v[i] / 10.0);
    let low = e(0) + e(1);
    let high = e(4) + e(5) + e(6);
    println!("{:>9.1}", 10.0 * (low / high).log10());
}

fn main() {
    let input = pluck(3.0);
    print!("{:<34}", "");
    for c in CENTRES {
        print!("{c:>7}");
    }
    println!("{:>9}", "low/hi");
    let only: Vec<String> = std::env::args().skip(1).collect();
    for p in PRESETS {
        if !only.is_empty()
            && !only
                .iter()
                .any(|o| p.group.contains(o.as_str()) || p.name.contains(o.as_str()))
        {
            continue;
        }
        row(p.name, &balance(p, &input));
    }
}
