//! A solve that fails is rescued, not guessed.
//!
//! When Newton cannot settle a sample, the simulation hands the sample to a
//! copy of the same circuit built at twice the rate, takes two half steps
//! there, and re-solves the full step from that answer (see
//! `Simulation::enable_half_step_rescue`). Before it, every such sample left
//! the bounded guess of a failed solve in the audio: on the real take of
//! `tests/fixtures`, 69 of them across the catalogue, and on Blizzard '80 one
//! of them was a pulse as tall as the signal's own peak.
//!
//! The JC-120's transistor power stage is where most of them were: 22 in the
//! 19 s take (16 since the source-scaled predictor), where run at twice the
//! rate the same stage has none. So the take is played through the Jazz
//! Chorus, on the audio thread's terms.
#[path = "support/allocations.rs"]
mod allocations;
use allocations::assert_no_heap;
use gainstagefx::presets::PRESETS;
use gainstagefx::voice::Chain;

const RATE: f64 = 48_000.0;

/// The fixture take, summed to mono, at its recorded level.
fn take() -> Vec<f64> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/01-260912_1044.wav"
    );
    let bytes = std::fs::read(path).expect("read the take");
    assert!(&bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE");
    let mut at = 12;
    let mut data = None;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        match &bytes[at..at + 4] {
            b"fmt " => assert_eq!(
                (
                    u16::from_le_bytes([body[2], body[3]]),
                    u32::from_le_bytes(body[4..8].try_into().unwrap()),
                    u16::from_le_bytes([body[14], body[15]])
                ),
                (1, RATE as u32, 24),
                "the take is mono PCM24 at 48 kHz"
            ),
            b"data" => data = Some(body),
            _ => {}
        }
        at += 8 + len + (len & 1);
    }
    data.expect("a data chunk")
        .as_chunks::<3>()
        .0
        .iter()
        .map(|b| ((b[0] as i32 | (b[1] as i32) << 8 | (b[2] as i32) << 16) << 8 >> 8) as f64)
        .map(|v| v / 8_388_608.0)
        .collect()
}

#[test]
fn the_jazz_chorus_take_settles_every_sample_without_allocating() {
    let preset = PRESETS
        .iter()
        .find(|p| p.name == "Jazz Chorus")
        .expect("the preset");
    let scale = 10f64.powf(preset.input_trim as f64 / 20.0);
    let mut chain = Chain::new(RATE);
    chain.apply(&preset.settings());
    chain.settle();
    chain.find_operating_point();
    // The whole take: with the source-scaled predictor the first failed
    // power-stage solves come later than they did, and fewer (16 in 19 s).
    let take = take();
    let samples = &take[..];

    let before = chain.solver_breakdown();
    let mut output = vec![0.0; samples.len()];
    assert_no_heap(|| {
        for (y, &x) in output.iter_mut().zip(samples) {
            *y = chain.process_stereo(x * scale).0;
        }
    });
    let power = chain
        .solver_breakdown()
        .power
        .saturating_delta(before.power);

    assert!(output.iter().all(|y| y.is_finite()));
    assert!(
        power.half_step_attempts >= 1,
        "the take no longer drives the JC-120's power stage into a failing \
         solve, so this test no longer exercises the rescue: give it a harder \
         input (a hotter trim) rather than deleting the check"
    );
    assert_eq!(
        power.half_step_bridged, power.half_step_attempts,
        "at twice the rate every failed step should be bridged"
    );
    assert_eq!(
        power.unsettled, 0,
        "a rescued sample must not end unsettled"
    );
}
