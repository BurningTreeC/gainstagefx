use gainstagefx::{
    params::{Circuit, GainStageParams, PowerAmp},
    plugin::GainStageFx,
    presets,
};
use nih_plug::prelude::*;
use nih_plug::wrapper::state::ParamValue;
use std::collections::BTreeMap;

#[test]
fn legacy_host_state_resets_power_override_without_changing_old_ids() {
    let mut state = PluginState {
        version: "0.10.0".into(),
        params: BTreeMap::new(),
        fields: BTreeMap::new(),
    };
    state
        .params
        .insert("circuit".into(), ParamValue::String("markiic".into()));
    GainStageFx::filter_state(&mut state);
    assert!(matches!(&state.params["power_amp"], ParamValue::String(s) if s == "matched"));
    assert!(matches!(&state.params["circuit"], ParamValue::String(s) if s == "markiic"));
    state
        .params
        .insert("power_amp".into(), ParamValue::String("bypass".into()));
    GainStageFx::filter_state(&mut state);
    assert!(matches!(&state.params["power_amp"], ParamValue::String(s) if s == "bypass"));
}

#[test]
fn legacy_saved_presets_resolve_matched_and_new_ones_use_stable_ids() {
    let params = GainStageParams::default();
    let mut old: presets::Stored =
        serde_json::from_str(r#"{"name":"old","values":{"circuit":0.75}}"#).unwrap();
    presets::migrate(&mut old, &params);
    assert_eq!(old.values["power_amp"], 0.0);
    assert_eq!(old.model_ids["power_amp"], "matched");
    assert_eq!(old.model_ids["circuit"], "markiic");
    old.model_ids
        .insert("power_amp".into(), "power_twin_ab763".into());
    old.values.insert("power_amp".into(), 0.0); // emulate a different enum length/order
    presets::migrate(&mut old, &params);
    assert_eq!(
        old.values["power_amp"],
        params
            .power_amp
            .preview_normalized(PowerAmp::American6L6Clean)
    );
    // Stored against the thirteen-entry circuit list, and still the same amplifier.
    assert_eq!(
        old.values["circuit"],
        params.circuit.preview_normalized(Circuit::Boogie)
    );
    let encoded = serde_json::to_string(&old).unwrap();
    let mut loaded: presets::Stored = serde_json::from_str(&encoded).unwrap();
    presets::migrate(&mut loaded, &params);
    assert_eq!(loaded.values, old.values);
}

#[test]
fn display_names_change_without_reinterpreting_legacy_enum_positions() {
    // These are the ids in the order a saved session's normalised value is
    // measured against, and they are checked as a **prefix**: the list may grow
    // at the end, and appending is how a circuit is added. What must never
    // happen is one of these being renamed, reordered, or something being
    // inserted among them -- any of which silently moves every automation lane
    // recorded against the parameter. A prefix check fails on all three and
    // passes an append, which is the rule this test exists to state.
    let legacy: &[&str] = &[
        "clean",
        "crunch",
        "highgain",
        "overdrive",
        "distortion",
        "console",
        "studio",
        "ts808",
        "bigmuff",
        "markiic",
        "evh5150",
        "neve",
        "twin",
        "amp_jcm800_2203",
        "pre_api_312",
        "pre_ssl_4000e",
        "pre_ua_610a",
        "amp_marshall_1959",
        "amp_vox_ac30_tb",
        "amp_hiwatt_dr103",
        "amp_dual_rectifier",
    ];
    let ids = Circuit::ids().unwrap();
    assert!(
        ids.len() >= legacy.len(),
        "the circuit list has shrunk: {} ids against {} legacy ones",
        ids.len(),
        legacy.len(),
    );
    assert_eq!(
        &ids[..legacy.len()],
        legacy,
        "a legacy circuit id moved or was renamed"
    );
    assert_eq!(Circuit::Boogie.name(), "Cali IIC+");
    // A preset's power stage is written as its stable position in the list.
    for preset in presets::PRESETS {
        let index = PowerAmp::ALL
            .iter()
            .position(|p| *p == preset.power_amp)
            .unwrap();
        assert!(
            preset.dials().contains(&("power_amp", index as f32)),
            "{}",
            preset.name
        );
    }
}

#[test]
fn every_legacy_circuit_position_survives_the_appended_brit_800() {
    let params = GainStageParams::default();
    for (index, circuit) in Circuit::ALL[..gainstagefx::params::LEGACY_CIRCUIT_COUNT]
        .iter()
        .enumerate()
    {
        let legacy = index as f32 / (gainstagefx::params::LEGACY_CIRCUIT_COUNT - 1) as f32;
        let mut old: presets::Stored = serde_json::from_str(&format!(
            r#"{{"name":"old","values":{{"circuit":{legacy}}}}}"#
        ))
        .unwrap();
        presets::migrate(&mut old, &params);
        assert_eq!(
            old.values["circuit"],
            params.circuit.preview_normalized(*circuit),
            "{}",
            circuit.name()
        );
        assert_eq!(old.model_ids["circuit"], Circuit::ids().unwrap()[index]);
    }
    // A preset that carries its stable id is never remapped.
    let mut new: presets::Stored = serde_json::from_str(
        r#"{"name":"new","model_ids":{"circuit":"amp_jcm800_2203"},"values":{"circuit":0.0}}"#,
    )
    .unwrap();
    presets::migrate(&mut new, &params);
    assert_eq!(
        new.values["circuit"],
        params.circuit.preview_normalized(Circuit::Brit800)
    );
}
