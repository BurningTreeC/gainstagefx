// Measured by `examples/calibrate.rs`, which states the reasoning.
// Do not edit by hand: run the example and paste its output over this
// file. `tests/voice.rs` re-measures every entry and fails on drift.

pub const CALIBRATION: [Calibration; VOICES] = [
    // Clean with a valve: 1.1236 V in, 3.0 % distortion, 0.3 % third.
    Calibration {
        drive_volts: 1.123647,
        make_up_db: [-12.32, -12.32, -12.35, -12.42, -12.57, -12.81, -13.17, -13.68, -14.34, -15.19, -16.25, -17.53, -19.08, -20.94, -23.23, -26.27, -31.22],
    },
    // Crunch with a valve: 0.0851 V in, 15.0 % distortion, 6.2 % third.
    Calibration {
        drive_volts: 0.085131,
        make_up_db: [-44.42, -44.42, -44.44, -44.52, -44.68, -44.93, -45.31, -45.84, -46.54, -47.44, -48.55, -49.91, -51.52, -53.43, -55.68, -58.36, -60.84],
    },
    // High Gain with a valve: 0.0021 V in, 40.0 % distortion, 8.0 % third.
    Calibration {
        drive_volts: 0.002121,
        make_up_db: [-84.37, -84.37, -84.40, -84.48, -84.64, -84.90, -85.28, -85.82, -86.53, -87.41, -88.40, -89.57, -90.91, -92.34, -93.56, -94.10, -94.11],
    },
    // Overdrive with silicon diodes: 0.0249 V in, 20.0 % distortion, 15.0 % third.
    Calibration {
        drive_volts: 0.024897,
        make_up_db: [-11.14, -11.14, -11.17, -11.25, -11.41, -11.67, -12.06, -12.62, -13.36, -14.33, -15.55, -17.03, -18.73, -20.56, -22.39, -23.99, -25.12],
    },
    // Overdrive with germanium diodes: 0.1170 V in, 18.1 % distortion, 14.6 % third. This voice cannot reach
    // its intended figure at any level, so this is its peak.
    Calibration {
        drive_volts: 0.116961,
        make_up_db: [-5.52, -5.52, -5.53, -5.54, -5.57, -5.61, -5.68, -5.76, -5.86, -5.97, -6.10, -6.22, -6.34, -6.45, -6.54, -6.61, -6.67],
    },
    // Overdrive with led diodes: 0.0809 V in, 20.0 % distortion, 12.2 % third.
    Calibration {
        drive_volts: 0.080908,
        make_up_db: [-11.18, -11.18, -11.21, -11.29, -11.45, -11.71, -12.11, -12.68, -13.44, -14.43, -15.69, -17.25, -19.15, -21.43, -23.92, -26.16, -28.08],
    },
    // Distortion with silicon diodes: 0.0650 V in, 38.0 % distortion, 30.7 % third.
    Calibration {
        drive_volts: 0.065013,
        make_up_db: [-9.63, -9.63, -9.64, -9.75, -9.96, -10.30, -10.83, -11.57, -12.59, -13.91, -15.52, -17.37, -19.37, -21.12, -22.38, -23.28, -23.63],
    },
    // Distortion with germanium diodes: 0.0643 V in, 38.0 % distortion, 30.5 % third.
    Calibration {
        drive_volts: 0.064308,
        make_up_db: [-7.26, -7.26, -7.27, -7.35, -7.50, -7.75, -8.11, -8.62, -9.29, -10.11, -11.07, -12.11, -13.16, -14.20, -15.18, -16.11, -16.51],
    },
    // Distortion with led diodes: 0.1090 V in, 38.0 % distortion, 31.0 % third.
    Calibration {
        drive_volts: 0.108979,
        make_up_db: [-9.66, -9.66, -9.68, -9.78, -9.99, -10.34, -10.87, -11.64, -12.69, -14.10, -15.94, -18.29, -21.20, -24.04, -26.66, -27.63, -27.87],
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
    // Studio with a valve: 0.0199 V in, 0.1 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.019861,
        make_up_db: [-12.32, -12.32, -12.35, -12.42, -12.57, -12.81, -13.18, -13.68, -14.34, -15.19, -16.25, -17.54, -19.08, -20.95, -23.24, -26.30, -31.31],
    },
    // Studio with a jfet: 0.0047 V in, 0.1 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.004652,
        make_up_db: [-8.47, -8.47, -8.50, -8.58, -8.72, -8.97, -9.33, -9.83, -10.49, -11.35, -12.40, -13.69, -15.24, -17.10, -19.39, -22.45, -27.47],
    },
    // Studio with an op-amp: 0.1474 V in, 0.0 % distortion, 0.0 % third.
    Calibration {
        drive_volts: 0.147426,
        make_up_db: [-6.06, -6.06, -6.06, -6.15, -6.34, -6.65, -7.14, -7.86, -8.88, -10.30, -12.22, -14.77, -18.07, -22.20, -27.24, -33.22, -40.17],
    },
    // TS808 with an op-amp: 0.1220 V in, 21.1 % distortion, 19.1 % third.
    Calibration {
        drive_volts: 0.122000,
        make_up_db: [12.15, 12.15, 12.14, 12.11, 12.04, 11.95, 11.81, 11.65, 11.46, 11.25, 11.05, 10.85, 10.67, 10.24, 9.84, 9.70, 9.63],
    },
    // Big Muff with transistors: 0.2828 V in, 30.0 % distortion, 18.5 % third. This voice cannot reach
    // its intended figure at any level, so this is its peak.
    Calibration {
        drive_volts: 0.282843,
        make_up_db: [16.68, 16.64, 16.29, 15.68, 15.08, 14.62, 14.30, 14.08, 13.92, 13.81, 13.72, 13.64, 13.58, 13.52, 13.47, 13.44, 13.34],
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
    // Neve 73P with transistors: 0.0075 V in, 3.0 % distortion, 1.8 % third.
    Calibration {
        drive_volts: 0.007462,
        make_up_db: [-29.03, -29.03, -29.03, -29.04, -29.06, -29.09, -29.14, -29.23, -29.38, -29.65, -30.14, -31.03, -32.56, -34.76, -37.02, -38.50, -38.90],
    },
];

/// Insertion loss of each output transformer, measured where the
/// core is still linear. What it does above that is the sound.
pub const IRON_TRIM: [f64; 3] = [1.089935, 1.089936, 1.089936];
