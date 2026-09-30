//! Where the noise floor comes from: the circuits, or what is fed to them.
//!
//! No circuit here has a noise source -- no resistor's thermal noise, no
//! valve's shot noise -- and the solver is deterministic, so a chain fed exact
//! digital silence should put out silence. What an interface records between
//! notes is not silence: the DI take's first second, before any playing, is
//! broadband hiss at about -92 dBFS RMS from the interface's preamp, the
//! pickups and the cable. An amplifier gives that hiss its full small-signal
//! gain while it clips the playing, so the distance between the two shrinks
//! by however much the amplifier compresses. For each preset this prints, all
//! relative to the preset's own output while playing:
//!
//! - `silence`: exact zeros in. What the plugin adds by itself.
//! - `DI noise`: the take's first 0.9 s in (its noise floor, no playing).
//! - `aperiodic`: a 375 Hz sine at the take's playing level, after four
//!   seconds. 375 Hz divides 48 kHz, so everything periodic -- harmonics, and
//!   the aliases of harmonics -- lands on a multiple of 375 Hz; what lands
//!   between them is not periodic.
//! - `broadband`: the same, leaving out eight bins (47 Hz) either side of each
//!   harmonic. A circuit still settling (a supply sagging, a bias shifting)
//!   spreads its harmonics into their neighbours, which counts in
//!   `aperiodic`; noise -- the solver's own error, if it made any -- is spread
//!   over the whole spectrum and counts here. Presets with a chorus, tremolo
//!   or spring reverb modulate or ring between the harmonics legitimately, so
//!   their figures are marked `mod`.
//!
//! ```text
//! cargo run --release --example noisefloor
//! cargo run --release --example noisefloor -- "Brown '84" "Jazz Chorus"
//! ```

use gainstagefx::presets::PRESETS;
use gainstagefx::voice::Chain;

const RATE: f64 = 48_000.0;
const TAKE: &str = "tests/fixtures/01-260912_1044.wav";
/// How long the sine plays before the spectrum is taken: slow settling (a
/// supply sagging, a bias shifting after a hard drive) is not noise.
const SETTLE_SECONDS: usize = 4;
/// Bins either side of each harmonic left out of the `broadband` figure: a
/// slow drift in the envelope spreads a harmonic over its neighbours.
const GUARD: usize = 8;

/// The take, mono, as recorded (24-bit PCM).
fn take() -> Vec<f64> {
    let bytes = std::fs::read(TAKE).expect("the DI take");
    let mut at = 12;
    let mut data = None;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        if &bytes[at..at + 4] == b"data" {
            data = Some(&bytes[at + 8..(at + 8 + len).min(bytes.len())]);
        }
        at += 8 + len + (len & 1);
    }
    data.expect("a data chunk")
        .chunks_exact(3)
        .map(|b| ((b[0] as i32 | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8) as f64)
        .map(|v| v / 8_388_608.0)
        .collect()
}

fn rms(x: &[f64]) -> f64 {
    (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
}

fn db(ratio: f64) -> f64 {
    20.0 * ratio.max(1e-300).log10()
}

/// Energy off the multiples of `bin` in an `x.len()`-point DFT, over all
/// energy but the DC bin. Computed directly at the bins that matter would miss
/// the ones between; this does the whole spectrum with a plain radix-2 FFT
/// on a power-of-two length, so `x.len()` must be one.
fn aperiodic_fraction(x: &[f64], bin: usize, guard: usize) -> f64 {
    let n = x.len();
    assert!(n.is_power_of_two());
    let mut re: Vec<f64> = x.to_vec();
    let mut im = vec![0.0; n];
    // Bit reversal, then iterative butterflies.
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            re.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -std::f64::consts::TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (angle * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
    let (mut periodic, mut other) = (0.0, 0.0);
    for k in 1..=n / 2 {
        let power = re[k] * re[k] + im[k] * im[k];
        // Distance to the nearest harmonic bin: `guard` excludes a slow
        // drift's skirt around each harmonic from the count, leaving the
        // broadband floor.
        let off = (k % bin).min(bin - k % bin);
        if off == 0 {
            periodic += power;
        } else if off > guard {
            other += power;
        }
    }
    other / (periodic + other)
}

fn main() {
    let names: Vec<String> = std::env::args().skip(1).collect();
    let take = take();
    let noise = &take[..(0.9 * RATE) as usize];
    let playing = &take[(2.0 * RATE) as usize..(10.0 * RATE) as usize];
    println!(
        "DI take: noise floor {:.1} dBFS RMS, playing {:.1} dBFS RMS: {:.1} dB apart at the input",
        db(rms(noise)),
        db(rms(playing)),
        db(rms(playing) / rms(noise))
    );
    println!("dB below the preset's output while playing:");
    println!(
        "{:28} {:>9} {:>9} {:>9} {:>9}",
        "preset", "silence", "DI noise", "aperiodic", "broadband"
    );
    // 2^16 samples of a sine whose period is 128 samples: 512 whole periods,
    // so its periodic content sits exactly on every 512th bin.
    const FFT: usize = 1 << 16;
    const PERIOD: usize = 128;
    for preset in PRESETS
        .iter()
        .filter(|p| names.is_empty() || names.iter().any(|n| n == p.name))
    {
        let settings = preset.settings();
        let scale = 10f64.powf(preset.input_trim as f64 / 20.0);
        let fresh = || {
            let mut chain = Chain::new(RATE);
            chain.apply(&settings);
            chain.settle();
            chain.find_operating_point();
            chain
        };
        let run = |chain: &mut Chain, input: &[f64]| -> Vec<f64> {
            input.iter().map(|&x| chain.process_stereo(x * scale).0).collect()
        };
        let mut chain = fresh();
        let reference = rms(&run(&mut chain, playing));

        let mut chain = fresh();
        run(&mut chain, &vec![0.0; RATE as usize]);
        let silence = rms(&run(&mut chain, &vec![0.0; RATE as usize / 2]));

        let mut chain = fresh();
        run(&mut chain, &vec![0.0; RATE as usize / 2]);
        let hiss = rms(&run(&mut chain, noise)[(0.2 * RATE) as usize..]);

        let mut chain = fresh();
        let amplitude = rms(playing) * std::f64::consts::SQRT_2;
        let sine: Vec<f64> = (0..SETTLE_SECONDS * RATE as usize + FFT)
            .map(|k| amplitude * (std::f64::consts::TAU * (k % PERIOD) as f64 / PERIOD as f64).sin())
            .collect();
        let out = run(&mut chain, &sine);
        let settle = SETTLE_SECONDS * RATE as usize;
        let tail = &out[settle..settle + FFT];
        let solver = aperiodic_fraction(tail, FFT / PERIOD, 0).sqrt();
        let floor = aperiodic_fraction(tail, FFT / PERIOD, GUARD).sqrt();
        let modulated = settings.intensity > 0.0 || settings.reverb > 0.0 || settings.chorus > 0.0;

        println!(
            "{:28} {:>9.1} {:>9.1} {:>9.1} {:>9.1}{}",
            preset.name,
            -db(silence / reference),
            -db(hiss / reference),
            -db(solver),
            -db(floor),
            if modulated { " mod" } else { "" }
        );
    }
}
