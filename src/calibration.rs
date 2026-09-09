// Measured by `examples/calibrate.rs`, which states the reasoning.
// Do not edit by hand: run the example and paste its output over this
// file. `tests/voice.rs` re-measures every entry and fails on drift.

pub const CALIBRATION: [Calibration; VOICES] = [
    // Clean with a valve: 1.1236 V in, 3.0 % distortion, 0.3 % third.
    Calibration {
        drive_volts: 1.123647,
        make_up_db: [-12.32, -12.32, -12.35, -12.42, -12.57, -12.81, -13.17, -13.68, -14.34, -15.19, -16.25, -17.53, -19.08, -20.94, -23.23, -26.27, -31.22],
    },
    // Crunch with a valve: 0.1220 V in, 30.0 % distortion, 9.1 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-44.40, -44.40, -44.43, -44.51, -44.67, -44.92, -45.30, -45.82, -46.52, -47.41, -48.52, -49.86, -51.46, -53.33, -55.49, -57.51, -58.08],
    },
    // High Gain with a valve: 0.1220 V in, 48.0 % distortion, 31.2 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-58.16, -58.16, -58.16, -58.16, -58.16, -58.18, -58.19, -58.22, -58.27, -58.36, -58.50, -58.74, -59.13, -59.65, -60.04, -60.11, -59.75],
    },
    // Overdrive with silicon diodes: 0.1220 V in, 29.7 % distortion, 23.9 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-10.36, -10.36, -10.38, -10.44, -10.55, -10.73, -11.00, -11.36, -11.81, -12.34, -12.91, -13.46, -13.94, -14.32, -14.62, -14.83, -14.98],
    },
    // Overdrive with germanium diodes: 0.1220 V in, 18.1 % distortion, 14.6 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-5.33, -5.33, -5.33, -5.34, -5.37, -5.41, -5.47, -5.55, -5.64, -5.75, -5.87, -5.99, -6.10, -6.20, -6.29, -6.36, -6.41],
    },
    // Overdrive with led diodes: 0.1220 V in, 25.7 % distortion, 20.4 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-11.09, -11.09, -11.12, -11.20, -11.36, -11.63, -12.03, -12.59, -13.35, -14.34, -15.60, -17.16, -19.05, -21.01, -22.88, -24.56, -25.37],
    },
    // Distortion with silicon diodes: 0.1220 V in, 42.3 % distortion, 32.5 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-8.65, -8.65, -8.67, -8.76, -8.95, -9.26, -9.73, -10.37, -11.22, -12.30, -13.58, -14.93, -16.12, -17.07, -17.83, -18.13, -18.21],
    },
    // Distortion with germanium diodes: 0.1220 V in, 42.5 % distortion, 32.5 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-4.87, -4.87, -4.88, -4.94, -5.05, -5.23, -5.50, -5.87, -6.34, -6.92, -7.57, -8.30, -9.06, -9.83, -10.61, -10.92, -11.01],
    },
    // Distortion with led diodes: 0.1220 V in, 38.3 % distortion, 31.3 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-8.85, -8.85, -8.87, -8.97, -9.18, -9.53, -10.06, -10.83, -11.88, -13.29, -15.13, -17.48, -20.38, -23.21, -25.79, -26.60, -26.81],
    },
    // Console with a valve: 0.3308 V in, 3.0 % distortion, 0.3 % third.
    Calibration {
        drive_volts: 0.330790,
        make_up_db: [-26.56, -26.56, -26.59, -26.66, -26.81, -27.05, -27.40, -27.89, -28.54, -29.37, -30.39, -31.61, -33.05, -34.72, -36.66, -38.94, -41.85],
    },
    // Console with a jfet: 0.0785 V in, 3.0 % distortion, 1.8 % third.
    Calibration {
        drive_volts: 0.078507,
        make_up_db: [-22.72, -22.72, -22.74, -22.82, -22.96, -23.20, -23.55, -24.05, -24.70, -25.52, -26.54, -27.77, -29.21, -30.88, -32.82, -35.12, -37.88],
    },
    // Console with an op-amp: 0.0340 V in, 3.0 % distortion, 2.2 % third.
    Calibration {
        drive_volts: 0.034008,
        make_up_db: [-25.79, -25.79, -25.80, -25.88, -26.04, -26.30, -26.70, -27.29, -28.12, -29.27, -30.81, -32.85, -35.49, -38.81, -42.90, -47.81, -53.34],
    },
    // Studio with a valve: 0.0199 V in, 0.0 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.019861,
        make_up_db: [-12.32, -12.32, -12.35, -12.42, -12.57, -12.81, -13.18, -13.68, -14.34, -15.19, -16.25, -17.54, -19.08, -20.95, -23.24, -26.30, -31.31],
    },
    // Studio with a jfet: 0.0047 V in, 0.0 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.004652,
        make_up_db: [-8.47, -8.47, -8.50, -8.58, -8.72, -8.97, -9.33, -9.83, -10.49, -11.35, -12.40, -13.69, -15.24, -17.10, -19.39, -22.45, -27.47],
    },
    // Studio with an op-amp: 0.1474 V in, 0.0 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.147426,
        make_up_db: [-6.06, -6.06, -6.06, -6.15, -6.34, -6.65, -7.14, -7.86, -8.88, -10.30, -12.22, -14.77, -18.07, -22.20, -27.24, -33.22, -40.17],
    },
    // TS808 with an op-amp: 0.1220 V in, 21.0 % distortion, 19.0 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-0.90, -0.90, -0.91, -0.95, -1.03, -1.15, -1.32, -1.53, -1.79, -2.06, -2.34, -2.60, -2.84, -3.04, -3.20, -3.32, -3.41],
    },
    // Big Muff with transistors: 0.2828 V in, 30.0 % distortion, 18.6 % third. This voice cannot reach
    // its intended figure at any level, so this is its peak.
    Calibration {
        drive_volts: 0.282843,
        make_up_db: [3.74, 3.70, 3.35, 2.74, 2.14, 1.68, 1.36, 1.14, 0.98, 0.87, 0.78, 0.70, 0.64, 0.58, 0.53, 0.50, 0.40],
    },
    // Mark IIC+ with a valve: 0.1220 V in, 39.1 % distortion, 31.2 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [13.53, 13.53, 1.70, -8.86, -16.34, -22.16, -26.97, -31.14, -34.86, -38.16, -40.99, -43.21, -44.39, -45.28, -45.73, -45.87, -45.93],
    },
    // 5150 with a valve: 0.1220 V in, 57.2 % distortion, 38.3 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [-20.41, -20.41, -32.32, -41.95, -43.04, -43.17, -43.22, -43.23, -43.23, -43.21, -43.18, -43.15, -43.13, -43.11, -43.17, -43.19, -43.22],
    },
    // Neve 73P with transistors: 0.0073 V in, 3.0 % distortion, 1.8 % third.
    Calibration {
        drive_volts: 0.007263,
        make_up_db: [-44.67, -44.67, -44.67, -44.68, -44.70, -44.73, -44.78, -44.87, -45.03, -45.30, -45.79, -46.69, -48.23, -50.45, -52.72, -54.19, -54.60],
    },
];

/// Insertion loss of each output transformer, measured where the
/// core is still linear. What it does above that is the sound.
pub const IRON_TRIM: [f64; 3] = [1.089935, 1.089936, 1.089936];
