// Measured by `examples/calibrate.rs`, which states the reasoning.
// Do not edit by hand: run the example and paste its output over this
// file. `tests/voice.rs` re-measures every entry and fails on drift.

pub const CALIBRATION: [Calibration; VOICES] = [
    // Clean with silicon diodes: 0.5871 V in, 1.5 % distortion, 0.1 % third.
    Calibration {
        drive_volts: 0.587050,
        make_up_db: [48.61, 25.87, 15.34, 6.88, -0.87, -8.21, -15.17, -21.97, -31.28],
    },
    // Crunch with silicon diodes: 0.0670 V in, 8.0 % distortion, 2.0 % third.
    Calibration {
        drive_volts: 0.066994,
        make_up_db: [17.13, -5.61, -16.16, -24.65, -32.48, -39.98, -47.23, -54.28, -61.92],
    },
    // High Gain with silicon diodes: 0.0015 V in, 25.0 % distortion, 11.7 % third.
    Calibration {
        drive_volts: 0.001480,
        make_up_db: [-22.44, -45.19, -55.74, -64.25, -72.11, -79.69, -87.07, -93.64, -97.22],
    },
    // Overdrive with silicon diodes: 0.0110 V in, 12.0 % distortion, 11.8 % third.
    Calibration {
        drive_volts: 0.010971,
        make_up_db: [-11.14, -13.37, -15.69, -18.06, -20.40, -22.62, -24.51, -25.93, -26.90],
    },
    // Overdrive with germanium diodes: 0.0398 V in, 12.0 % distortion, 11.3 % third.
    Calibration {
        drive_volts: 0.039782,
        make_up_db: [-6.44, -6.98, -7.39, -7.70, -7.93, -8.10, -8.23, -8.32, -8.39],
    },
    // Overdrive with led diodes: 0.0479 V in, 12.0 % distortion, 11.8 % third.
    Calibration {
        drive_volts: 0.047903,
        make_up_db: [-11.18, -13.44, -15.80, -18.23, -20.70, -23.21, -25.74, -27.92, -29.03],
    },
    // Distortion with silicon diodes: 0.0393 V in, 30.0 % distortion, 25.9 % third.
    Calibration {
        drive_volts: 0.039253,
        make_up_db: [-5.45, -8.05, -11.20, -14.71, -18.20, -20.77, -22.28, -23.22, -23.88],
    },
    // Distortion with germanium diodes: 0.0340 V in, 30.0 % distortion, 24.9 % third.
    Calibration {
        drive_volts: 0.034024,
        make_up_db: [-4.03, -6.31, -8.81, -11.11, -13.00, -14.48, -15.65, -16.61, -17.41],
    },
    // Distortion with led diodes: 0.0755 V in, 30.0 % distortion, 27.3 % third.
    Calibration {
        drive_volts: 0.075498,
        make_up_db: [-5.46, -8.07, -11.23, -14.79, -18.60, -22.56, -25.84, -27.07, -27.65],
    },
];
