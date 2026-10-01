// Measured by `examples/directout.rs`. Do not edit by hand.
/// The small-signal gain, dB, from each circuit's input to its own direct out
/// (`Gain::direct_out`), at 1 kHz with every other control at its rest, at the
/// drive positions `knot_position` gives. What levels the Preamp DI of a
/// circuit whose direct out is ahead of its last stage; see `Chain` and
/// `docs/DI.md`.
pub const DIRECT_OUT_DB: &[(Gain, [f64; POINTS])] = &[
    (
        Gain::American800RB,
        [-52.88, -52.88, -52.88, -48.54, -41.05, -35.25, -30.51, -26.51, -23.05, -20.01, -17.30, -14.87, -12.65, -10.63, -8.77, -7.05, -5.46, -3.98, -2.61, -1.32, -0.11, 1.01, 2.08, 3.07, 4.01, 4.90, 8.73, 12.23, 14.75, 17.00, 19.38, 22.24, 26.25],
    ),
];
