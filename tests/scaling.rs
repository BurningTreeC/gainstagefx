//! The panel size, which has to survive a session.
//!
//! This is a plain round trip through the same two calls the host makes when
//! it saves and reloads a session, because the bug it is here to catch was not
//! in the drawing or in the menu -- both of those worked -- but in the figure
//! never reaching the state that gets written down.

use gainstagefx::editor::{default_state, remember_scale, window_height, FIRST_OPEN};
use gainstagefx::params::GainStageParams;
use nice_plug::params::Params;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Every section of the panel open: the size the fixed-size figures below
/// were taken at.
const ALL_OPEN: u8 = 0b111111;

/// Setting the size has to change the state the host reads, not just what
/// vizia draws at. `Editor::size` is computed from this, so if it does not
/// move the host sizes the window for the old scale.
#[test]
fn choosing_a_size_reaches_the_state_the_host_reads() {
    let state = default_state(Arc::new(AtomicU8::new(FIRST_OPEN)));
    assert_eq!(
        state.user_scale_factor(),
        1.0,
        "a fresh panel opens at 100 %"
    );
    remember_scale(&state, 1.25);
    assert_eq!(
        state.user_scale_factor(),
        1.25,
        "the size was chosen but the state never heard about it"
    );
    let (w, h) = state.inner_logical_size();
    let (sw, sh) = state.scaled_logical_size();
    println!("{w}x{h} logical, {sw}x{sh} at 125 %");
    assert!(
        sw > w && sh > h,
        "the window the host is told to make did not grow"
    );
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
        println!(
            "{scale:.2} saved, {:.2} restored",
            restored.editor_state.user_scale_factor()
        );
        assert_eq!(
            restored.editor_state.user_scale_factor(),
            scale,
            "the panel reopened at a different size than it was left at"
        );
    }
}

/// Menu zoom and rendering DPI are distinct: 100% renders at the 1.5 base DPI.
#[test]
fn an_untouched_panel_opens_at_100_percent_with_1_5_base_dpi() {
    let params = GainStageParams::default();
    let restored = GainStageParams::default();
    restored.deserialize_fields(&params.serialize_fields());
    assert_eq!(restored.editor_state.user_scale_factor(), 1.0);
    assert_eq!(restored.editor_state.rendering_scale_factor(), 1.5);
    // With only the first section open, which is how a fresh panel opens.
    assert_eq!(
        restored.editor_state.scaled_logical_size(),
        (1170, (window_height(FIRST_OPEN) * 1.5).round() as u32)
    );
}

/// The panel's sections open and close, and the window follows: which ones
/// are open has to come back with the session, and the size the host is
/// told has to be the size of what is open -- at the time it asks, since the
/// editor changes the set and then asks the host to resize.
#[test]
fn the_open_sections_survive_a_session_and_size_the_window() {
    let fresh = GainStageParams::default();
    assert_eq!(fresh.open_sections.load(Ordering::Relaxed), FIRST_OPEN);
    assert_eq!(
        fresh.editor_state.inner_logical_size(),
        (780, window_height(FIRST_OPEN) as u32)
    );
    for open in [0b000000, 0b010110, ALL_OPEN] {
        let saved = GainStageParams::default();
        saved.open_sections.store(open, Ordering::Relaxed);
        assert_eq!(
            saved.editor_state.inner_logical_size(),
            (780, window_height(open) as u32),
            "the size did not follow the open sections"
        );
        let restored = GainStageParams::default();
        restored.deserialize_fields(&saved.serialize_fields());
        assert_eq!(restored.open_sections.load(Ordering::Relaxed), open);
        assert_eq!(
            restored.editor_state.inner_logical_size(),
            (780, window_height(open) as u32),
            "the panel reopened at a size that does not fit its open sections"
        );
    }
    assert_eq!(window_height(ALL_OPEN), 968.0);
}

/// Nice-plug 0.4 asks for an explicit native size through baseview's host
/// callbacks. The drawing scale is committed only after that transaction succeeds.
mod resize {
    use vizia_plug::vizia::request_user_scale;

    /// A section opened or closed changes the unzoomed size at the same zoom:
    /// kept when the window arrives at it, undone when the host keeps the old
    /// one, and left pending while an earlier resize is still reporting.
    #[test]
    fn a_new_unzoomed_size_settles_or_rolls_back() {
        use vizia_plug::vizia::settle_inner_size;
        let (old, new) = ((780, 306), (780, 466));
        assert_eq!(settle_inner_size((1170.0, 699.0), 1.5, new, old), Some(new));
        assert_eq!(settle_inner_size((1170.0, 459.0), 1.5, new, old), Some(old));
        assert_eq!(settle_inner_size((1170.0, 600.0), 1.5, new, old), None);
        assert_eq!(settle_inner_size((975.3, 582.8), 1.25, new, old), Some(new));
    }

    #[test]
    fn native_resize_receives_the_requested_logical_size() {
        let accepted = request_user_scale(1.0, 1.5, (780, 968), |size| {
            assert_eq!((size.width, size.height), (1170.0, 1452.0));
            true
        });
        assert_eq!(accepted, Some(1.5));
    }

    #[test]
    fn failed_native_request_does_not_record_a_pending_zoom() {
        let result = request_user_scale(1.25, 2.0, (780, 968), |size| {
            assert_eq!((size.width, size.height), (1560.0, 1936.0));
            false
        });
        assert_eq!(result, None);
    }

    #[test]
    fn repeated_zoom_does_not_make_redundant_host_requests() {
        let mut current = 1.0;
        let mut calls = 0;
        for requested in [0.5, 2.0, 0.75, 1.5, 1.0] {
            current = request_user_scale(current, requested, (780, 968), |_| {
                calls += 1;
                true
            })
            .unwrap();
            assert_eq!(
                request_user_scale(current, requested, (780, 968), |_| panic!(
                    "duplicate resize"
                )),
                None
            );
        }
        assert_eq!(calls, 5);
    }
}

/// Reopening the plugin has to come up at the size that was chosen, which is a
/// third thing again: not what vizia draws at now, and not what the host is
/// asked for now, but what both are set from the next time the editor is
/// spawned.
///
/// `ViziaEditor::spawn` reads exactly two things off the stored state --
/// `user_scale_factor()`, which it hands to the window description, and
/// `Editor::size()`, which is `scaled_logical_size()`. So the contract this
/// stands over is that after choosing a size, both of those already read back
/// the chosen one. If they did not, the panel would reopen drawing at one
/// scale inside a window built for another.
#[test]
fn the_panel_reopens_at_the_size_it_was_left_at() {
    for scale in gainstagefx::editor::session::SCALES {
        // Chosen in one session...
        let saved = GainStageParams::default();
        remember_scale(&saved.editor_state, scale);
        let fields = saved.serialize_fields();

        // ...and read back in the next, by the two calls `spawn` makes.
        let restored = GainStageParams::default();
        restored.deserialize_fields(&fields);
        let state = &restored.editor_state;

        assert_eq!(
            state.user_scale_factor(),
            scale,
            "the window would be built to draw at a different scale than chosen"
        );

        let (uw, uh) = state.inner_logical_size();
        let (sw, sh) = state.scaled_logical_size();
        assert_eq!(
            (sw, sh),
            (
                (uw as f64 * scale * 1.5).round() as u32,
                (uh as f64 * scale * 1.5).round() as u32
            ),
            "the window the host is told to make at {scale:.2} does not match \
             the scale the panel will draw at"
        );
    }
}

#[test]
fn asynchronous_native_resize_and_host_rollback_restore_zoom() {
    use vizia_plug::vizia::resolve_user_scale;
    assert_eq!(resolve_user_scale((1170.0, 1452.0), (780, 968), 1.5), 1.5);
    assert_eq!(resolve_user_scale((780.0, 968.0), (780, 968), 1.5), 1.0);
    assert_eq!(resolve_user_scale((975.3, 1209.7), (780, 968), 1.25), 1.25);
}

#[test]
fn large_menu_scales_are_relative_to_the_base_dpi() {
    let state = default_state(Arc::new(AtomicU8::new(ALL_OPEN)));
    for (zoom, dpi, dimensions) in [
        (1.0, 1.5, (1170, 1452)),
        (1.75, 2.625, (2048, 2541)),
        (2.0, 3.0, (2340, 2904)),
    ] {
        assert!(gainstagefx::editor::session::SCALES.contains(&zoom));
        remember_scale(&state, zoom);
        assert_eq!(state.user_scale_factor(), zoom);
        assert_eq!(state.rendering_scale_factor(), dpi);
        assert_eq!(state.scaled_logical_size(), dimensions);
    }
}
