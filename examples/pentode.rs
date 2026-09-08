//! The power tube model against its data sheet.
//!
//! A 6L6GC's published operating points are the only thing that says whether
//! Koren's constants have been transcribed correctly, and getting them wrong
//! is silent: the tube still conducts, still clips, still sounds like
//! something, and is simply not the tube on the drawing.
//!
//! `cargo run --release --example pentode`

use gainstagefx::dsp::device::Pentode;
use gainstagefx::dsp::netlist::PentodeSpec;

fn main() {
    let tube = Pentode::new(0, 1, 2, 3, 1.0, PentodeSpec::T6L6GC);

    println!("6L6GC, one tube. Plate and screen current in milliamps.\n");
    println!(
        "  {:>7}{:>7}{:>7}{:>10}{:>10}",
        "Vp", "Vs", "Vg", "Ip", "Ig2"
    );
    // The data sheet's own rows: class AB1 push-pull, and the single-ended
    // point everyone quotes.
    for (vp, vs, vg) in [
        (250.0, 250.0, -14.0),
        (350.0, 250.0, -18.0),
        (400.0, 300.0, -25.0),
        (450.0, 400.0, -37.0),
        (450.0, 400.0, -20.0),
        (450.0, 400.0, 0.0),
        (100.0, 400.0, 0.0),
        (50.0, 400.0, 0.0),
    ] {
        let (ip, ig2) = tube.currents(vp, vg, vs);
        println!(
            "  {vp:>7.0}{vs:>7.0}{vg:>7.0}{:>9.1}{:>10.1}",
            ip * 1000.0,
            ig2 * 1000.0
        );
    }

    println!("\nTransfer at Vp=Vs=400: the grid swing a push-pull pair uses.\n");
    println!("  {:>7}{:>10}", "Vg", "Ip");
    let mut vg = -50.0;
    while vg <= 0.01 {
        let (ip, _) = tube.currents(400.0, vg, 400.0);
        println!("  {vg:>7.0}{:>9.1}", ip * 1000.0);
        vg += 5.0;
    }
}
