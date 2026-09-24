//! Fits the JC-120 driver (`SpeakerProfile::JAZZ_12`) to the one published
//! measurement of that driver there is, by reproducing the measurement inside
//! the model.
//!
//! No manufacturer data exists for the Roland 30-103D beyond its size and
//! impedance. What does exist is a measurement: a JC-120 driven through its
//! RETURN jack -- so its own power stage and nothing of the preamp -- into its
//! own speakers in its own cabinet, picked up by a calibrated measurement
//! microphone about 9 cm off the centre of one cone and about 4 cm from the
//! grille, analysed in REW:
//! <https://tomachikeita.hatenablog.com/entry/2024/09/25/140335> (2024-09-25).
//! The JC-120's power amplifier has 26 dB of loop gain to spare and is flat
//! across the band, so the curve is the speakers and cabinet as a close
//! microphone hears them.
//!
//! The Celestion profiles were fitted against far-field manufacturer plots,
//! and that recipe does not apply to a near-field in-cabinet curve. So this
//! runs the plugin's own path at the measurement's own geometry -- the
//! voltage-driven driver's motional response, then `AcousticStage` with an
//! ideal omni at position 9 / 12.5 of the radius and 4 cm, in the Jazz Open
//! 2x12, through the near cone only (see `FIT_CABINET`) -- and fits the
//! breakup voicing until that path draws the measured curve. Fs and Bl are
//! priors: the measurement cannot see them (see `FIT_LOW`). The level is not
//! compared: the measurement's absolute level is a REW target, not a
//! sensitivity. Fitted 2026-09-24 to 1.16 dB rms; the result is
//! `SpeakerProfile::JAZZ_12`.
//!
//! `cargo run --release --example jazz_speaker_fit`

#[path = "../tests/support/jazz_measurement.rs"]
mod measurement;
use gainstagefx::acoustics::speaker::{Breakup, Peak, SpeakerProfile};
use measurement::{modelled, rms_error, target, FIT_HIGH, FIT_LOW, MEASURED};

/// A candidate breakup voicing: three peaks (Hz, dB, Q) and the low-pass (Hz,
/// Q), clamped to the bounds the other profiles were fitted inside. The
/// peaks' ranges overlap, so the fit decides which feature each one takes.
///
/// Fs and Bl are **not** fitted: the measurement cannot see them (see
/// `FIT_LOW`). They are the priors in `SpeakerProfile::JAZZ_12`.
fn profile_of(p: &[f64]) -> SpeakerProfile {
    let peak = |i: usize, lo: f64, hi: f64| Peak {
        hz: p[i].clamp(lo, hi),
        gain_db: p[i + 1].clamp(-12.0, 12.0),
        q: p[i + 2].clamp(0.5, 6.0),
    };
    SpeakerProfile {
        breakup: Breakup {
            peaks: [
                peak(0, 250.0, 2_000.0),
                peak(3, 800.0, 6_000.0),
                peak(6, 3_000.0, 12_000.0),
            ],
            lowpass_hz: p[9].clamp(2_500.0, 15_000.0),
            lowpass_q: p[10].clamp(0.5, 2.5),
        },
        ..SpeakerProfile::JAZZ_12
    }
}

/// The measurement's error for a candidate voicing.
fn error(p: &[f64]) -> f64 {
    rms_error(profile_of(p))
}

/// Plain Nelder-Mead.
fn minimise(
    start: &[f64],
    scale: &[f64],
    f: &dyn Fn(&[f64]) -> f64,
    iterations: usize,
) -> Vec<f64> {
    let n = start.len();
    let mut simplex: Vec<Vec<f64>> = vec![start.to_vec()];
    for i in 0..n {
        let mut v = start.to_vec();
        v[i] += scale[i];
        simplex.push(v);
    }
    let mut values: Vec<f64> = simplex.iter().map(|v| f(v)).collect();
    for _ in 0..iterations {
        let mut order: Vec<usize> = (0..=n).collect();
        order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
        simplex = order.iter().map(|&i| simplex[i].clone()).collect();
        values = order.iter().map(|&i| values[i]).collect();
        let centroid: Vec<f64> = (0..n)
            .map(|j| simplex[..n].iter().map(|v| v[j]).sum::<f64>() / n as f64)
            .collect();
        let along = |t: f64| -> Vec<f64> {
            (0..n)
                .map(|j| centroid[j] + t * (simplex[n][j] - centroid[j]))
                .collect()
        };
        let reflected = along(-1.0);
        let fr = f(&reflected);
        if fr < values[0] {
            let expanded = along(-2.0);
            let fe = f(&expanded);
            if fe < fr {
                simplex[n] = expanded;
                values[n] = fe;
            } else {
                simplex[n] = reflected;
                values[n] = fr;
            }
        } else if fr < values[n - 1] {
            simplex[n] = reflected;
            values[n] = fr;
        } else {
            let contracted = along(0.5);
            let fc = f(&contracted);
            if fc < values[n] {
                simplex[n] = contracted;
                values[n] = fc;
            } else {
                for i in 1..=n {
                    simplex[i] = (0..n)
                        .map(|j| simplex[0][j] + 0.5 * (simplex[i][j] - simplex[0][j]))
                        .collect();
                    values[i] = f(&simplex[i]);
                }
            }
        }
    }
    let best = (0..=n)
        .min_by(|&a, &b| values[a].total_cmp(&values[b]))
        .unwrap();
    simplex[best].clone()
}

fn main() {
    let target = target();
    // Two starts: the measurement's own three peaks, and a resonant low-pass
    // making the 3.6 kHz peak and the fall after it, with the peaks left for
    // the midrange and 9.2 kHz.
    let starts = [
        [
            480.0, 2.0, 2.0, 3_600.0, 6.0, 3.0, 9_200.0, 6.0, 3.0, 5_000.0, 0.7,
        ],
        [
            500.0, 3.0, 2.0, 1_250.0, 3.0, 2.0, 9_200.0, 10.0, 3.0, 4_300.0, 1.5,
        ],
        [
            480.0, 3.0, 2.0, 1_300.0, 2.0, 2.0, 9_200.0, 8.0, 3.0, 3_900.0, 2.0,
        ],
    ];
    let scale = [
        150.0, 3.0, 1.0, 400.0, 3.0, 1.0, 1_500.0, 3.0, 1.0, 1_000.0, 0.3,
    ];
    let objective = |p: &[f64]| error(p);
    let mut best = starts[0].to_vec();
    let mut best_error = f64::INFINITY;
    for start in starts {
        eprintln!("start: {:.2} dB rms", objective(&start));
        // Restarted from its own result, which is what gets Nelder-Mead out
        // of the simplex collapses an eleven-dimensional problem invites.
        let mut here = start.to_vec();
        for round in 0..6 {
            here = minimise(&here, &scale, &objective, 1_500);
            eprintln!("  round {round}: {:.3} dB rms", objective(&here));
        }
        let e = objective(&here);
        if e < best_error {
            (best, best_error) = (here, e);
        }
    }
    let p = profile_of(&best);
    let q = p.qes() * p.qms / (p.qes() + p.qms);
    println!("best {best_error:.3} dB rms from {FIT_LOW} Hz to {FIT_HIGH} Hz");
    println!(
        "fs {:.1} Hz, bl {:.3} T m (priors) -> Qes {:.3}, Qts {:.3}",
        p.fs,
        p.bl,
        p.qes(),
        q
    );
    for (i, pk) in p.breakup.peaks.iter().enumerate() {
        println!(
            "peak {i}: {:.0} Hz, {:+.2} dB, Q {:.2}",
            pk.hz, pk.gain_db, pk.q
        );
    }
    println!(
        "lowpass {:.0} Hz, Q {:.2}",
        p.breakup.lowpass_hz, p.breakup.lowpass_q
    );

    // Measured against modelled, the latter offset to the measurement's level.
    let freqs: Vec<f64> = target.iter().map(|(f, _)| *f).collect();
    let model = modelled(p, &freqs);
    let inband: Vec<(f64, f64)> = model
        .iter()
        .zip(&target)
        .filter(|(_, (f, _))| (FIT_LOW..=FIT_HIGH).contains(f))
        .map(|(m, (_, t))| (*m, *t))
        .collect();
    let offset = inband.iter().map(|(m, t)| m - t).sum::<f64>() / inband.len() as f64;
    println!("\n  Hz      measured  smoothed  model");
    for ((f, measured), ((_, smoothed), m)) in MEASURED.iter().zip(target.iter().zip(&model)) {
        if [
            63.5, 100.8, 201.6, 479.5, 905.1, 1076.3, 2031.9, 3620.4, 5120.0, 6834.4, 9122.8,
            11494.0,
        ]
        .iter()
        .any(|g| (g - f).abs() < 0.5)
        {
            println!(
                "  {f:>7.0}  {measured:>7.1}  {smoothed:>7.1}  {:>7.1}",
                m - offset
            );
        }
    }
}
