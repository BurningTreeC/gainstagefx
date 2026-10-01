//! The American 4x10 (Gallien-Krueger 410RBH): its vents' tuning, from the
//! impedance the four tens present in the box, and the same box sealed.
//!
//! `cargo run --release --example american_410_op`

use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::speaker::{LoadValues, SpeakerProfile};

fn main() {
    let cab = CabinetProfile::AMERICAN_410;
    let speaker = &SpeakerProfile::CAST_BASS_10;
    let vent = cab.vent.unwrap();
    println!(
        "net volume {:.1} l; vents {} x {:.1} cm2, {:.1} cm deep ({:.1} cm effective)",
        cab.volume() * 1e3,
        vent.count,
        vent.area * 1e4,
        vent.length * 1e2,
        vent.effective_length() * 1e2
    );
    let helmholtz = 343.0 / std::f64::consts::TAU
        * (vent.count as f64 * vent.area / (cab.volume() * vent.effective_length())).sqrt();
    println!("Helmholtz tuning of the box and vents alone: {helmholtz:.1} Hz");
    for (name, c) in [
        ("vented", cab),
        ("sealed", CabinetProfile { vent: None, ..cab }),
    ] {
        let load = LoadValues::new(speaker, &c.mounting(), 1.0);
        let mut curve = Vec::new();
        let mut hz = 15.0;
        while hz < 200.0 {
            curve.push((hz, load.impedance(hz).magnitude()));
            hz *= 1.02;
        }
        let peaks: Vec<(f64, f64)> = curve
            .windows(3)
            .filter(|w| w[1].1 > w[0].1 && w[1].1 > w[2].1)
            .map(|w| w[1])
            .collect();
        let dips: Vec<(f64, f64)> = curve
            .windows(3)
            .filter(|w| w[1].1 < w[0].1 && w[1].1 < w[2].1)
            .map(|w| w[1])
            .collect();
        print!("{name}: peaks");
        for (f, z) in &peaks {
            print!(" {f:.1} Hz {z:.1} ohm;");
        }
        print!(" dips");
        for (f, z) in &dips {
            print!(" {f:.1} Hz {z:.1} ohm;");
        }
        println!();
    }
}
