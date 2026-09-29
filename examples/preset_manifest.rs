//! Export factory presets as CLAP parameter values for host-level measurements.
use gainstagefx::{params::GainStageParams, presets::PRESETS};
use nice_plug::prelude::*;
fn main() {
    let params = GainStageParams::default();
    let map = params.param_map();
    let presets: Vec<_> = PRESETS
        .iter()
        .map(|preset| {
            let values: Vec<_> = preset
                .dials()
                .into_iter()
                .map(|(id, plain)| {
                    let (_, param, _) = map.iter().find(|(key, _, _)| key == id).unwrap();
                    let normalized = unsafe { param.preview_normalized(plain) };
                    (id, normalized)
                })
                .collect();
            (preset.name, values)
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&presets).unwrap());
}
