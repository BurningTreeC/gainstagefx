//! The plugin's own two-channel path, checked for cancellation and startup.
//!
//! "The meters move but I hear nothing" survives DC, bandwidth and level. What
//! is left is the two channels cancelling when they are summed, and the model
//! taking a long time to say anything. Both are checked here against the
//! actual arrangement `plugin.rs` uses: channel 0 hunts the operating point
//! and hands it to the rest.
//!
//! `cargo run --release --example stereo_check`

use gainstagefx::voice::{Cabinet, Chain, Gain, Iron, Settings, Tone as ToneSection, NOMINAL_DBFS};

const RATE: f64 = 48_000.0;

fn main() {
    let amp = 10f64.powf(NOMINAL_DBFS / 20.0) * std::f64::consts::SQRT_2;
    println!("220 Hz at nominal, two channels as the plugin builds them.\n");
    println!(
        "  {:<12}{:>12}{:>12}{:>14}{:>16}",
        "voice", "L rms", "R rms", "(L+R)/2 rms", "silent lead-in"
    );
    for gain in [
        Gain::Clean,
        Gain::Crunch,
        Gain::Overdrive,
        Gain::Distortion,
        Gain::Screamer,
        Gain::Muff,
        Gain::Boogie,
        Gain::Peavey,
        Gain::Neve,
    ] {
        let make = || {
            let mut c = Chain::new(RATE);
            c.apply(&Settings {
                gain,
                drive: 0.7,
                tone: ToneSection::Scooping,
                cabinet: Cabinet::Stack,
                iron: Iron::Steel,
                oversampling: 4,
                ..Settings::default()
            });
            c
        };
        let mut a = make();
        let mut b = make();
        // Exactly what `plugin.rs` does: channel 0 hunts, the rest are handed it.
        a.find_operating_point();
        b.share_operating_point_from(a.operating_point());
        if let Some(op) = a.iron_operating_point() {
            b.share_iron_operating_point_from(op);
        }
        if let Some(op) = a.power_operating_point() {
            b.share_power_operating_point_from(op);
        }

        let n = RATE as usize;
        let (mut sl, mut sr, mut sm) = (0.0f64, 0.0f64, 0.0f64);
        let mut first_loud = None;
        for k in 0..n {
            let t = k as f64 / RATE;
            let x = amp * (std::f64::consts::TAU * 220.0 * t).sin();
            let l = a.process(x);
            let r = b.process(x);
            let m = 0.5 * (l + r);
            if first_loud.is_none() && l.abs() > 0.01 {
                first_loud = Some(k);
            }
            // Skip the first tenth of a second when totalling.
            if k > n / 10 {
                sl += l * l;
                sr += r * r;
                sm += m * m;
            }
        }
        let c = (n - n / 10) as f64;
        println!(
            "  {:<12}{:>11.1} dB{:>11.1} dB{:>13.1} dB{:>13}",
            gain.name(),
            20.0 * (sl / c).sqrt().max(1e-12).log10(),
            20.0 * (sr / c).sqrt().max(1e-12).log10(),
            20.0 * (sm / c).sqrt().max(1e-12).log10(),
            match first_loud {
                Some(k) => format!("{:.1} ms", 1000.0 * k as f64 / RATE),
                None => "never".into(),
            }
        );
    }
}
