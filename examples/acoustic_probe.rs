//! Cabinet and microphone processing: filters, delay, polar patterns, placement
//! behaviour, geometry, dual microphones, automation safety and rate independence.
use gainstagefx::acoustics::cabinet::CabinetProfile;
use gainstagefx::acoustics::mic::MicPlacement;
use gainstagefx::acoustics::speaker::SpeakerProfile;
use gainstagefx::acoustics::stage::{AcousticStage, MicSlot};
use std::f64::consts::TAU;

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

struct Rig {
    stage: AcousticStage,
    rate: f64,
}

impl Rig {
    fn new(
        rate: f64,
        cab: Option<&'static CabinetProfile>,
        speaker: &'static SpeakerProfile,
        mic: MicSlot,
    ) -> Self {
        let mut stage = AcousticStage::new(rate);
        stage.configure(cab, speaker, mic, MicSlot::Off);
        Self { stage, rate }
    }

    fn place(&mut self, position: f64, distance: f64, angle: f64) -> &mut Self {
        let p = MicPlacement {
            position,
            distance,
            angle,
        };
        self.stage.set_placement(p, p, 0.0, 0.0, 0.0, false, false);
        self.stage.reset();
        self
    }

    /// Steady-state level of a sine through the stage, dB.
    fn level(&mut self, hz: f64) -> f64 {
        self.stage.reset();
        let n = (self.rate * 0.25) as usize + (self.rate * 8.0 / hz) as usize;
        let mut peak: f64 = 0.0;
        for k in 0..n {
            let y = self
                .stage
                .process(0.1 * (TAU * hz * k as f64 / self.rate).sin());
            if k > n * 3 / 4 {
                peak = peak.max(y.abs());
            }
        }
        db(peak / 0.1)
    }
}

fn main() {
    for cab in [
        &CabinetProfile::CLOSED_112,
        &CabinetProfile::AMERICAN_OPEN_112,
    ] {
        let mut r = Rig::new(
            48000.0,
            Some(cab),
            &SpeakerProfile::AMERICAN_CERAMIC,
            MicSlot::Ideal,
        );
        r.place(0.0, 1.0, 0.0);
        println!(
            "{}: {:?}",
            cab.id,
            [10.0, 30.0, 50.0, 70.0, 100.0, 300.0, 1000.0].map(|f| r.level(f))
        );
    }
}
