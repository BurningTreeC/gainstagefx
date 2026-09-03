//! Saved presets: the part that touches the filesystem.
//!
//! One test rather than several, deliberately. `preset_dir` reads the
//! environment, and Rust runs the tests in a file on threads of one process --
//! so two tests each pointing the config directory somewhere else would race,
//! and the one that failed would be whichever lost.

use gainstagefx::params::GainStageParams;
use gainstagefx::presets::{self, PRESETS, SAVED};

#[test]
fn saved_presets_survive_the_round_trip() {
    let dir = std::env::temp_dir().join(format!("gainstagefx-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    std::env::set_var("XDG_CONFIG_HOME", &dir);

    let params = GainStageParams::default();

    // --- nothing saved yet -------------------------------------------------
    let all = presets::load_all(&params);
    assert_eq!(
        all.len(),
        PRESETS.len(),
        "an empty preset directory should give exactly the shipped catalogue"
    );
    assert!(
        all.iter().all(|p| p.built_in),
        "nothing is saved, so nothing should be deletable"
    );
    // Every shipped preset has to survive conversion to normalised values with
    // its settings intact, or loading one would silently give something else.
    for preset in &all {
        assert!(
            !preset.values.is_empty(),
            "'{}' converted to no values at all",
            preset.name
        );
    }

    // --- save --------------------------------------------------------------
    let captured = presets::capture(&params, "  Test Sound  ");
    assert_eq!(captured.name, "Test Sound", "the name should be trimmed");
    assert!(
        !captured.values.contains_key("oversampling"),
        "oversampling is about what the plugin costs, not what it sounds like, \
         and has no business in a preset"
    );
    presets::save(&captured).expect("saves");

    let all = presets::load_all(&params);
    assert_eq!(all.len(), PRESETS.len() + 1);
    let saved = all.last().expect("the saved one is last");
    assert_eq!(saved.name, "Test Sound");
    assert!(!saved.built_in, "a saved preset is yours to delete");
    assert_eq!(saved.group, SAVED, "it belongs under its own heading");
    assert_eq!(
        saved.values, captured.values,
        "what came back off disk is not what went on to it"
    );

    // --- what is and is not taken ------------------------------------------
    assert!(presets::name_taken("test sound", &all), "matched case-insensitively");
    assert!(
        !presets::name_taken("Scooped Metal", &all),
        "saving under a shipped preset's name writes a new file beside it and \
         replaces nothing, so warning about it would describe something that \
         does not happen"
    );

    // --- a saved preset matches the panel it came from ---------------------
    // What the comparison decides is tested exhaustively in `tests/presets.rs`,
    // driven from both sides. What matters here is that the values that came
    // back off disk are still the ones that panel would be compared against.
    assert!(
        presets::matches(&params, &saved.values),
        "a preset written and read back should not read as edited"
    );
    let mut differing = saved.values.clone();
    let drive = differing["drive"];
    differing.insert(String::from("drive"), drive + 0.3);
    assert!(
        !presets::matches(&params, &differing),
        "and one that differs should"
    );

    // --- a name that is not a file name ------------------------------------
    let awkward = presets::capture(&params, "Lead / Solo: \"hot\"");
    presets::save(&awkward).expect("an awkward name still saves");
    let all = presets::load_all(&params);
    assert!(
        all.iter().any(|p| p.name == "Lead / Solo: \"hot\""),
        "a preset should come back under the name it was given, whatever the \
         file had to be called"
    );

    // --- delete -------------------------------------------------------------
    presets::delete("Test Sound").expect("deletes");
    presets::delete("Lead / Solo: \"hot\"").expect("deletes the awkward one too");
    let all = presets::load_all(&params);
    assert_eq!(
        all.len(),
        PRESETS.len(),
        "the shipped catalogue should be all that is left"
    );
    assert!(
        presets::delete("Test Sound").is_err(),
        "deleting what is not there should say so rather than succeed quietly"
    );

    // --- the things that should refuse --------------------------------------
    let unnamed = presets::capture(&params, "   ");
    assert!(
        presets::save(&unnamed).is_err(),
        "a preset with no name would be saved as 'preset' and be impossible to \
         find again"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
