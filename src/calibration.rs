// Measured by `examples/calibrate.rs`, which states the reasoning.
// Do not edit by hand: run the example and paste its output over this
// file. `tests/voice.rs` re-measures every entry and fails on drift.

pub const CALIBRATION: [Calibration; VOICES] = [
    // Clean with a valve: 1.1236 V in, 3.0 % distortion, 0.3 % third.
    Calibration {
        drive_volts: 1.123647,
        make_up_db: [-12.32, -14.34, -16.34, -18.33, -20.34, -22.44, -24.73, -27.47, -31.22],
    },
    // Crunch with a valve: 0.0851 V in, 15.0 % distortion, 6.2 % third.
    Calibration {
        drive_volts: 0.085131,
        make_up_db: [-44.42, -46.54, -48.65, -50.74, -52.82, -54.92, -57.05, -59.28, -60.84],
    },
    // High Gain with a valve: 0.0021 V in, 40.0 % distortion, 8.0 % third.
    Calibration {
        drive_volts: 0.002121,
        make_up_db: [-84.37, -86.53, -88.49, -90.28, -91.91, -93.22, -93.94, -94.14, -94.11],
    },
    // Overdrive with silicon diodes: 0.0218 V in, 20.0 % distortion, 18.6 % third.
    Calibration {
        drive_volts: 0.021768,
        make_up_db: [-11.13, -13.34, -15.61, -17.82, -19.75, -21.20, -22.18, -22.81, -23.23],
    },
    // Overdrive with germanium diodes: 0.0980 V in, 14.5 % distortion, 12.9 % third. This voice cannot reach
    // its intended figure at any level, so this is its peak.
    Calibration {
        drive_volts: 0.098025,
        make_up_db: [-4.47, -4.67, -4.81, -4.91, -4.99, -5.05, -5.09, -5.12, -5.14],
    },
    // Overdrive with led diodes: 0.0672 V in, 20.0 % distortion, 19.5 % third.
    Calibration {
        drive_volts: 0.067173,
        make_up_db: [-11.18, -13.44, -15.80, -18.23, -20.70, -23.21, -25.29, -26.32, -26.84],
    },
    // Distortion with silicon diodes: 0.0557 V in, 38.0 % distortion, 30.6 % third.
    Calibration {
        drive_volts: 0.055657,
        make_up_db: [-9.62, -12.59, -15.61, -17.87, -19.27, -20.17, -20.83, -21.34, -21.55],
    },
    // Distortion with germanium diodes: 0.0617 V in, 38.0 % distortion, 30.5 % third.
    Calibration {
        drive_volts: 0.061678,
        make_up_db: [-5.97, -7.51, -8.88, -10.04, -11.02, -11.86, -12.59, -13.21, -13.41],
    },
    // Distortion with led diodes: 0.0825 V in, 38.0 % distortion, 31.5 % third.
    Calibration {
        drive_volts: 0.082515,
        make_up_db: [-9.66, -12.69, -16.11, -19.79, -23.60, -25.75, -26.58, -26.91, -27.02],
    },
    // Console with a valve: 0.3308 V in, 3.0 % distortion, 0.3 % third.
    Calibration {
        drive_volts: 0.330790,
        make_up_db: [-26.56, -28.54, -30.48, -32.36, -34.19, -36.01, -37.83, -39.73, -41.85],
    },
    // Console with a jfet: 0.0785 V in, 3.0 % distortion, 1.8 % third.
    Calibration {
        drive_volts: 0.078507,
        make_up_db: [-22.72, -24.70, -26.63, -28.51, -30.35, -32.17, -34.00, -35.92, -37.88],
    },
    // Console with an op-amp: 0.0340 V in, 3.0 % distortion, 2.2 % third.
    Calibration {
        drive_volts: 0.034008,
        make_up_db: [-25.79, -28.12, -30.95, -34.19, -37.73, -41.51, -45.43, -49.46, -53.34],
    },
    // Studio with a valve: 0.0199 V in, 0.1 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.019861,
        make_up_db: [-12.32, -14.34, -16.34, -18.34, -20.35, -22.45, -24.75, -27.51, -31.31],
    },
    // Studio with a jfet: 0.0047 V in, 0.1 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.004652,
        make_up_db: [-8.47, -10.49, -12.50, -14.49, -16.50, -18.60, -20.90, -23.66, -27.47],
    },
    // Studio with an op-amp: 0.1474 V in, 0.0 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.147426,
        make_up_db: [-6.06, -8.88, -12.40, -16.45, -20.87, -25.52, -30.33, -35.23, -40.17],
    },
];

/// Insertion loss of each output transformer, measured where the
/// core is still linear. What it does above that is the sound.
pub const IRON_TRIM: [f64; 3] = [1.089935, 1.089936, 1.089936];
