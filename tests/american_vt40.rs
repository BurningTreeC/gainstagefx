//! The Ampeg VT-40's valves, before the amplifier itself: the 6CG7 fit
//! against Tung-Sol's sheet, and the 6K11 as the 12AU7 and 12AX7 its sheets
//! say its units are. See `docs/models/american_vt40.md`.
use gainstagefx::dsp::device::Triode;
use gainstagefx::dsp::netlist::{TriodeSpec, GROUND};

fn near(got: f64, want: f64, within: f64) -> bool {
    (got / want - 1.0).abs() <= within
}

/// Plate current and its two slopes at a point.
fn point(spec: TriodeSpec, vp: f64, vg: f64) -> (f64, f64, f64) {
    let valve = Triode::new(0, 1, GROUND, spec);
    let i = |vp: f64, vg: f64| valve.plate(vp, vg);
    let h = 1e-3;
    let gm = (i(vp, vg + h) - i(vp, vg - h)) / (2.0 * h);
    let ga = (i(vp + h, vg) - i(vp - h, vg)) / (2.0 * h);
    (i(vp, vg), gm, ga)
}

/// The 6CG7 fit meets Tung-Sol's 1959 sheet at its six targets within 2 %:
/// 10 mA and 3.0 mA/V at 90 V and 0 V; 9 mA, 2.6 mA/V and 7.7 kohm at
/// 250 V and -8 V; 1.3 mA at -12.5 V. See `tools/tube_fit/fit_6cg7.py`.
#[test]
fn the_6cg7_fit_meets_tung_sols_sheet() {
    let (i90, gm90, _) = point(TriodeSpec::T6CG7, 90.0, 0.0);
    let (i250, gm250, ga250) = point(TriodeSpec::T6CG7, 250.0, -8.0);
    let (i125, _, _) = point(TriodeSpec::T6CG7, 250.0, -12.5);
    println!(
        "90 V: {:.2} mA, {:.2} mA/V; 250 V: {:.2} mA, {:.2} mA/V, {:.0} ohm; -12.5 V: {:.2} mA",
        i90 * 1e3,
        gm90 * 1e3,
        i250 * 1e3,
        gm250 * 1e3,
        1.0 / ga250,
        i125 * 1e3
    );
    assert!(near(i90, 10e-3, 0.02) && near(gm90, 3.0e-3, 0.02));
    assert!(near(i250, 9e-3, 0.02) && near(gm250, 2.6e-3, 0.02));
    assert!(near(1.0 / ga250, 7_700.0, 0.02));
    assert!(near(i125, 1.3e-3, 0.02));
}

/// The 6K11's units, by RCA's and GE's sheets, are the 12AU7 (mu 17,
/// 7.7 kohm, 2.2 mA/V, 10.5 mA at 250 V / -8.5 V) and the 12AX7 (mu 100,
/// 62.5 kohm, 1.6 mA/V, 1.2 mA at 250 V / -2 V) to the figure, and are built
/// from the solver's ECC82 and ECC83. Where those stand at the sheets' points:
/// the ECC82 within 15 %; the ECC83, which is Koren's published 12AX7 and has
/// no fit of its own, 0.95 mA against 1.2 -- the model's known low current at
/// that point -- with its transconductance and amplification factor within
/// 11 %. The same model every 12AX7 in the catalogue uses.
#[test]
fn the_6k11s_units_are_the_12au7_and_the_12ax7() {
    let (i, gm, ga) = point(TriodeSpec::ECC82, 250.0, -8.5);
    println!(
        "medium-mu unit (ECC82): {:.2} mA, {:.2} mA/V, mu {:.1}",
        i * 1e3,
        gm * 1e3,
        gm / ga
    );
    assert!(near(i, 10.5e-3, 0.15) && near(gm, 2.2e-3, 0.15) && near(gm / ga, 17.0, 0.15));
    let (i, gm, ga) = point(TriodeSpec::ECC83, 250.0, -2.0);
    println!(
        "high-mu units (ECC83): {:.2} mA, {:.2} mA/V, mu {:.1}",
        i * 1e3,
        gm * 1e3,
        gm / ga
    );
    assert!(near(i, 1.2e-3, 0.25) && near(gm, 1.6e-3, 0.11) && near(gm / ga, 100.0, 0.11));
}
