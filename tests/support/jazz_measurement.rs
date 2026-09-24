//! A published measurement of the Roland JC-120's speakers in their cabinet,
//! and the plugin's own path drawn at the measurement's placement.
//!
//! Shared by `examples/jazz_speaker_fit.rs`, which fits
//! `SpeakerProfile::JAZZ_12`'s breakup voicing to it, and `tests/jazz_cabinet.rs`,
//! which holds the result. The measurement: a JC-120 driven through its RETURN
//! jack -- its own power stage and nothing of the preamp -- into its own
//! speakers, picked up by a calibrated measurement microphone about 9 cm off
//! the centre of one cone and about 4 cm from the grille, analysed in REW:
//! <https://tomachikeita.hatenablog.com/entry/2024/09/25/140335> (2024-09-25).
#![allow(dead_code)]

use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::MicPlacement;
use gainstagefx::acoustics::speaker::{LoadValues, SpeakerProfile};
use gainstagefx::acoustics::stage::{AcousticStage, MicSlot};
use gainstagefx::dsp::complex::C;

pub const RATE: f64 = 48_000.0;
/// 171 ms: the stage's longest memory is its baffle-step shelf, settled well
/// inside it.
const IMPULSE: usize = 1 << 13;

/// The measurement, digitised from the REW plot's own grid (422 px a decade,
/// 14.8 px a dB) at twelfth-octave points: frequency in Hz, SPL in dB.
pub const MEASURED: [(f64, f64); 100] = [
    (40.0, 92.16),
    (42.4, 93.12),
    (44.9, 94.13),
    (47.6, 95.32),
    (50.4, 96.89),
    (53.4, 97.94),
    (56.6, 98.28),
    (59.9, 98.47),
    (63.5, 101.51),
    (67.3, 101.76),
    (71.3, 101.79),
    (75.5, 102.77),
    (80.0, 103.02),
    (84.8, 103.81),
    (89.8, 103.03),
    (95.1, 104.56),
    (100.8, 104.29),
    (106.8, 104.48),
    (113.1, 104.17),
    (119.9, 104.39),
    (127.0, 102.53),
    (134.5, 102.59),
    (142.5, 104.92),
    (151.0, 103.93),
    (160.0, 104.20),
    (169.5, 102.32),
    (179.6, 102.84),
    (190.3, 102.53),
    (201.6, 103.42),
    (213.6, 104.58),
    (226.3, 104.89),
    (239.7, 104.09),
    (254.0, 105.15),
    (269.1, 105.44),
    (285.1, 105.15),
    (302.0, 104.82),
    (320.0, 105.59),
    (339.0, 105.12),
    (359.2, 104.08),
    (380.5, 103.06),
    (403.2, 104.95),
    (427.1, 105.74),
    (452.5, 106.44),
    (479.5, 106.97),
    (508.0, 106.04),
    (538.2, 104.90),
    (570.2, 104.43),
    (604.1, 104.01),
    (640.0, 103.89),
    (678.1, 103.38),
    (718.4, 102.23),
    (761.1, 100.87),
    (806.3, 99.00),
    (854.3, 97.06),
    (905.1, 95.99),
    (958.9, 96.98),
    (1015.9, 98.58),
    (1076.3, 98.89),
    (1140.4, 97.85),
    (1208.2, 96.98),
    (1280.0, 97.20),
    (1356.1, 98.27),
    (1436.8, 98.41),
    (1522.2, 97.42),
    (1612.7, 96.49),
    (1708.6, 95.95),
    (1810.2, 95.44),
    (1917.8, 94.70),
    (2031.9, 94.25),
    (2152.7, 94.28),
    (2280.7, 94.31),
    (2416.3, 94.17),
    (2560.0, 94.20),
    (2712.2, 94.52),
    (2873.5, 95.08),
    (3044.4, 95.71),
    (3225.4, 96.25),
    (3417.2, 96.86),
    (3620.4, 97.09),
    (3835.7, 96.52),
    (4063.7, 95.15),
    (4305.4, 92.95),
    (4561.4, 90.35),
    (4832.6, 87.92),
    (5120.0, 85.39),
    (5424.5, 83.80),
    (5747.0, 82.97),
    (6088.7, 82.23),
    (6450.8, 81.16),
    (6834.4, 80.24),
    (7240.8, 80.45),
    (7671.3, 81.55),
    (8127.5, 82.77),
    (8610.8, 83.66),
    (9122.8, 84.05),
    (9665.3, 83.63),
    (10240.0, 82.09),
    (10848.9, 79.90),
    (11494.0, 76.94),
    (12177.5, 73.65),
];

/// The measurement's placement, in the panel's terms: 9 cm off the centre of a
/// cone of radius 12.5 cm, 4 cm from the grille.
pub const PLACEMENT: MicPlacement = MicPlacement {
    position: 0.09 / 0.125,
    distance: 0.04,
    angle: 0.0,
};

/// Where the fit is judged. Below about 90 Hz a near-field measurement of a
/// combo standing on a floor is the floor as much as the speaker -- a first
/// fit that included it drove Fs to its 45 Hz bound, which is not a guitar
/// driver -- and above 12 kHz the curve has left the plot.
pub const FIT_LOW: f64 = 90.0;
pub const FIT_HIGH: f64 = 12_000.0;

/// The measured curve with its room ripple averaged out over a sixth of an
/// octave, which three breakup peaks should not be asked to chase.
pub fn target() -> Vec<(f64, f64)> {
    MEASURED
        .iter()
        .map(|&(f, _)| {
            let near: Vec<f64> = MEASURED
                .iter()
                .filter(|(g, _)| (g / f).log2().abs() <= 1.0 / 12.0)
                .map(|&(_, d)| d)
                .collect();
            (f, near.iter().sum::<f64>() / near.len() as f64)
        })
        .collect()
}

/// The Jazz cabinet with only the cone the microphone is in front of.
///
/// At 4 cm from one 12-inch cone the other, 42 cm away and about 80 degrees
/// off its axis, is more than 10 dB down at low frequencies and far more above
/// 2 kHz, where a piston that size is sharply directional. The stage models
/// that directivity as one pole, which leaves the far cone only about 8 dB
/// down with 0.76 ms more delay -- and so draws comb-filter notches at 0.6,
/// 2.0 and 3.4 kHz that the measurement does not have, and a fit through the
/// two-driver cabinet spends its peaks filling them. The measurement is the
/// near cone and its own rear wave out of the open back, and that is what the
/// voicing is fitted through. See `docs/models/speakers.md`.
pub const FIT_CABINET: CabinetProfile = CabinetProfile {
    drivers: 1,
    ..CabinetProfile::JAZZ_OPEN_212
};

/// The plugin's path at the measurement's placement, in dB, at each frequency.
pub fn modelled(profile: SpeakerProfile, freqs: &[f64]) -> Vec<f64> {
    modelled_in(&FIT_CABINET, profile, freqs)
}

/// The same through a chosen cabinet.
pub fn modelled_in(
    cab: &'static CabinetProfile,
    profile: SpeakerProfile,
    freqs: &[f64],
) -> Vec<f64> {
    let profile: &'static SpeakerProfile = Box::leak(Box::new(profile));
    let load = LoadValues::new(profile, &cab.mounting(), 1.0);
    let mut stage = AcousticStage::new(RATE);
    stage.configure(Some(cab), profile, MicSlot::Ideal, MicSlot::Off);
    stage.set_placement(PLACEMENT, PLACEMENT, 0.0, 0.0, 0.0, false, false);
    stage.reset();
    let impulse: Vec<f64> = (0..IMPULSE)
        .map(|k| stage.process(if k == 0 { 1.0 } else { 0.0 }))
        .collect();
    freqs
        .iter()
        .map(|&hz| {
            // The stage, from its impulse response: one DFT bin, by rotation
            // rather than a sine and cosine a sample.
            let w = std::f64::consts::TAU * hz / RATE;
            let step = C::new(w.cos(), -w.sin());
            let mut phasor = C::real(1.0);
            let mut sum = C::ZERO;
            for &y in &impulse {
                sum = sum + phasor * C::real(y);
                phasor = phasor * step;
            }
            let stage_h = sum;
            // The driver: motional voltage over terminal voltage, voltage driven,
            // differentiated -- which is what the chain hands the stage.
            let s = C::new(0.0, std::f64::consts::TAU * hz);
            let lossy = |l: f64, r: f64| {
                let sl = s * C::real(l);
                sl * C::real(r) / (C::real(r) + sl)
            };
            let coil = C::real(load.re) + lossy(load.l1, load.r1) + lossy(load.l2, load.r2);
            let admittance = C::real(1.0 / load.res)
                + (s * C::real(load.lces)).recip()
                + s * C::real(load.cmes)
                + (s * C::real(load.lbox) + C::real(load.rbox)).recip();
            let motional = admittance.recip();
            let cone = motional * (coil + motional).recip() * s;
            20.0 * (cone * stage_h).magnitude().max(1e-30).log10()
        })
        .collect()
}

/// RMS difference in dB over the fitted band between `profile` drawn at the
/// measurement's placement and the smoothed measurement, after the best level
/// offset (the measurement's absolute level is a REW target, not a
/// sensitivity).
pub fn rms_error(profile: SpeakerProfile) -> f64 {
    let band: Vec<(f64, f64)> = target()
        .into_iter()
        .filter(|(f, _)| (FIT_LOW..=FIT_HIGH).contains(f))
        .collect();
    let freqs: Vec<f64> = band.iter().map(|(f, _)| *f).collect();
    let model = modelled(profile, &freqs);
    let diff: Vec<f64> = model.iter().zip(&band).map(|(m, (_, t))| m - t).collect();
    let offset = diff.iter().sum::<f64>() / diff.len() as f64;
    (diff.iter().map(|d| (d - offset).powi(2)).sum::<f64>() / diff.len() as f64).sqrt()
}
