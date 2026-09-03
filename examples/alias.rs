//! What each oversampling setting is worth, measured against the best one.
//!
//! Not by looking at a spectrum and calling some of it aliasing, but by
//! running the same signal through the same circuit at each setting and
//! comparing it with the setting nothing is above. Whatever the difference is,
//! that is what the cheaper setting costs.

use gainstagefx::voice::{self, Chain};

const RATE: f64 = 48_000.0;

fn render(gain: voice::Gain, factor: usize, hz: f64, n: usize) -> Vec<f64> {
    let mut chain = Chain::new(RATE);
    chain.set_voice(gain, voice::Diode::Silicon, voice::Amplifier::Valve);
    chain.set_oversampling(factor);
    chain.set_drive(0.85);
    let w = std::f64::consts::TAU * hz / RATE;
    // Settle, then capture.
    for i in 0..(RATE as usize / 4) {
        chain.process(0.4 * (w * i as f64).sin());
    }
    (0..n)
        .map(|i| chain.process(0.4 * (w * (i + RATE as usize / 4) as f64).sin()))
        .collect()
}

fn main() {
    let n = 16_384;
    for gain in [voice::Gain::HighGain, voice::Gain::Distortion, voice::Gain::Clean] {
        // 3 kHz: its third harmonic is already past Nyquist at this rate, so
        // everything above that has to fold somewhere.
        for hz in [1000.0, 3000.0] {
            let reference = render(gain, 8, hz, n);
            let ref_rms = (reference.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
            print!("{:<12}{hz:>6.0} Hz  ref rms {ref_rms:.5}  ", gain.name());
            for factor in [1, 2, 4] {
                let got = render(gain, factor, hz, n);
                let (mut err, mut sig) = (0.0, 0.0);
                for (a, b) in got.iter().zip(&reference) {
                    err += (a - b) * (a - b);
                    sig += b * b;
                }
                let db = 10.0 * (err / sig.max(1e-30)).max(1e-30).log10();
                let rms = (got.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
                print!("{factor}x {db:>7.1} dB rms {rms:.5}   ");
            }
            println!();
        }
    }
}
