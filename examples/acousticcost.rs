//! What the speaker load, cabinet and microphones cost, and how loud they are,
//! against the legacy resistor load and baked cabinet filter. One channel, 48 kHz.
//!
//! Each row renders two seconds of a two-tone guitar-range signal at the nominal
//! level (then a third with the level +12 dB for the drive-dependent rows) and
//! reports realtime load and the RMS of the second second relative to the legacy
//! Stack row.

use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::{MicPlacement, MicProfile};
use gainstagefx::acoustics::stage::{AcousticStage, MicSlot};
use gainstagefx::acoustics::speaker::SpeakerProfile;
use gainstagefx::voice::{
    AcousticSettings, CabinetChoice, Cabinet, Chain, Gain, PowerAmp, Settings, SpeakerChoice, Tone, NOMINAL_DBFS,
};

const RATE: f64 = 48_000.0;

fn run(label: &str, settings: Settings, reference: Option<f64>) -> f64 {
    let mut chain = Chain::new(RATE);
    chain.apply(&settings);
    chain.settle();
    chain.find_operating_point();
    let amplitude = 10f64.powf(NOMINAL_DBFS / 20.0);
    let n = (RATE * 2.0) as usize;
    let mut sum = 0.0;
    let start = std::time::Instant::now();
    for i in 0..n {
        let t = i as f64 / RATE;
        let x = amplitude
            * ((std::f64::consts::TAU * 110.0 * t).sin() * 0.7 + (std::f64::consts::TAU * 330.0 * t).sin() * 0.3);
        let y = chain.process(x);
        if i >= n / 2 {
            sum += y * y;
        }
    }
    let load = start.elapsed().as_secs_f64() / 2.0 * 100.0;
    let rms = (sum / (n / 2) as f64).sqrt();
    let health = chain.solver_breakdown();
    let level = reference.map(|r| format!("{:+6.1} dB", 20.0 * (rms / r).log10())).unwrap_or_else(|| "   ref".into());
    println!(
        "{label:<46}{load:>7.1}%  {level}  power passes/solve {:.2} unsettled {}",
        health.power.passes_per_solve(),
        health.power.unsettled
    );
    rms
}

fn main() {
    let physical = |cab: &'static CabinetProfile| AcousticSettings {
        cabinet: CabinetChoice::Model(cab),
        speaker: SpeakerChoice::Matched,
        mic_a: MicSlot::Profile(&MicProfile::DYNAMIC_57),
        place_a: MicPlacement { position: 0.3, distance: 0.025, angle: 0.0 },
        ..AcousticSettings::default()
    };
    println!("{:<46}{:>8}  {:>9}", "", "load", "level");
    for (gain, power_amp, name) in [
        (Gain::Boogie, PowerAmp::Matched, "Cali IIC+"),
        (Gain::Boogie, PowerAmp::BritEL34, "Cali IIC+ -> Brit EL34"),
        (Gain::Twin, PowerAmp::Matched, "American Twin"),
        (Gain::Peavey, PowerAmp::Matched, "American 5150"),
        (Gain::Screamer, PowerAmp::Matched, "Green 808 (no power stage)"),
    ] {
        let base = Settings { gain, power_amp, tone: Tone::Off, drive: 0.6, ..Settings::default() };
        let reference = run(&format!("{name}: legacy Stack"), Settings { cabinet: Cabinet::Stack, ..base }, None);
        run(&format!("{name}: legacy Off (resistor DI)"), base, Some(reference));
        run(&format!("{name}: Brit V30 4x12, Dynamic 57"), Settings { acoustic: physical(&CabinetProfile::BRIT_V30), ..base }, Some(reference));
        run(
            &format!("{name}: + Ribbon 121 at 30 cm"),
            Settings {
                acoustic: AcousticSettings {
                    mic_b: MicSlot::Profile(&MicProfile::RIBBON_121),
                    place_b: MicPlacement { position: 0.5, distance: 0.3, angle: 0.0 },
                    ..physical(&CabinetProfile::BRIT_V30)
                },
                ..base
            },
            Some(reference),
        );
        run(&format!("{name}: American Open 2x12, Dynamic 57"), Settings { acoustic: physical(&CabinetProfile::AMERICAN_OPEN_212), ..base }, Some(reference));
        println!();
    }

    // The acoustic stage alone, per sample, single and dual microphone.
    for (label, b) in [("stage alone, one mic", MicSlot::Off), ("stage alone, two mics", MicSlot::Profile(&MicProfile::RIBBON_121))] {
        let mut stage = AcousticStage::new(RATE);
        stage.configure(Some(&CabinetProfile::BRIT_CLOSED), &SpeakerProfile::BRIT_T75, MicSlot::Profile(&MicProfile::DYNAMIC_57), b);
        let p = MicPlacement { position: 0.3, distance: 0.5, angle: 20.0 };
        stage.set_placement(p, p, 0.5, false, false);
        let n = (RATE * 4.0) as usize;
        let start = std::time::Instant::now();
        let mut acc = 0.0;
        for i in 0..n {
            acc += stage.process((i as f64 * 0.01).sin());
        }
        let load = start.elapsed().as_secs_f64() / 4.0 * 100.0;
        println!("{label:<46}{load:>7.2}%  ({:.0} ns/sample){}", load / 100.0 / RATE * 1e9, if acc.is_finite() { "" } else { " NaN" });
    }
}
