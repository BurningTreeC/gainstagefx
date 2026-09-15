// Measured by `examples/powertrim.rs`. Do not edit by hand.
/// Level change, dB, of each power override relative to the voice's own path,
/// by `Gain::ALL` row; columns Bypass, Cali 6L6, American 6L6 Clean,
/// American 6L6 High-Gain, Brit EL34. See `Chain::power_trim`.
pub const POWER_TRIM_DB: [[f64; 5]; 13] = [
    [0.00, -7.97, -15.66, 7.35, -6.82], // Clean
    [0.00, -5.63, -12.74, 2.32, -5.64], // Crunch
    [0.00, -14.04, -14.28, -10.42, -14.34], // High Gain
    [0.00, -7.77, -15.63, 6.90, -6.79], // Overdrive
    [0.00, -8.34, -16.08, 6.55, -7.00], // Distortion
    [0.00, -8.32, -16.05, 6.61, -6.99], // Console
    [0.00, -8.33, -16.07, 6.53, -6.99], // Studio
    [0.00, -7.77, -15.63, 6.89, -6.79], // TS808
    [0.00, -8.45, -16.17, 6.47, -7.04], // Big Muff
    [5.63, 0.00, -7.37, 6.18, 0.02], // Mark IIC+
    [-8.04, -15.18, -23.24, 0.00, -14.62], // 5150
    [0.00, -8.31, -16.05, 6.60, -6.98], // Neve 73P
    [-12.13, -20.54, -28.27, -5.42, -19.15], // Twin Reverb
];
