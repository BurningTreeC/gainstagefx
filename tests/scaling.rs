//! The panel size, which has to survive a session.
//!
//! This is a plain round trip through the same two calls the host makes when
//! it saves and reloads a session, because the bug it is here to catch was not
//! in the drawing or in the menu -- both of those worked -- but in the figure
//! never reaching the state that gets written down.

use gainstagefx::editor::{default_state, remember_scale};
use gainstagefx::params::GainStageParams;
use nih_plug::params::Params;

/// Setting the size has to change the state the host reads, not just what
/// vizia draws at. `Editor::size` is computed from this, so if it does not
/// move the host sizes the window for the old scale.
#[test]
fn choosing_a_size_reaches_the_state_the_host_reads() {
    let state = default_state();
    assert_eq!(state.user_scale_factor(), 1.0, "a fresh panel opens at 100 %");
    remember_scale(&state, 1.25);
    assert_eq!(
        state.user_scale_factor(),
        1.25,
        "the size was chosen but the state never heard about it"
    );
    let (w, h) = state.inner_logical_size();
    let (sw, sh) = state.scaled_logical_size();
    println!("{w}x{h} logical, {sw}x{sh} at 125 %");
    assert!(sw > w && sh > h, "the window the host is told to make did not grow");
}

/// And it has to come back. This is exactly what the host does: serialise the
/// persistent fields on save, hand them back on load.
#[test]
fn the_size_survives_a_session() {
    for scale in gainstagefx::editor::session::SCALES {
        let saved = GainStageParams::default();
        remember_scale(&saved.editor_state, scale);
        let fields = saved.serialize_fields();

        let restored = GainStageParams::default();
        restored.deserialize_fields(&fields);
        println!("{scale:.2} saved, {:.2} restored", restored.editor_state.user_scale_factor());
        assert_eq!(
            restored.editor_state.user_scale_factor(),
            scale,
            "the panel reopened at a different size than it was left at"
        );
    }
}

/// The default is unchanged by all this: a plugin that has never had its size
/// touched still opens at one hundred per cent.
#[test]
fn an_untouched_panel_opens_at_full_size() {
    let params = GainStageParams::default();
    let restored = GainStageParams::default();
    restored.deserialize_fields(&params.serialize_fields());
    assert_eq!(restored.editor_state.user_scale_factor(), 1.0);
}
