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

/// The default is unchanged by all this: a plugin that has never had its size
/// touched still opens at one hundred per cent.
#[test]
fn an_untouched_panel_opens_at_full_size() {
    let params = GainStageParams::default();
    let restored = GainStageParams::default();
    restored.deserialize_fields(&params.serialize_fields());
    assert_eq!(restored.editor_state.user_scale_factor(), 1.0);
}

/// Choosing a size has to *ask the host to resize the window*, which is a
/// different thing from storing the number and was the half that was missing.
///
/// Only `GuiContext::request_resize` moves a plugin window. nih-plug calls it
/// from one place -- its `WindowModel`, on a `GeometryChanged` -- behind a
/// guard that returns early when the unscaled size and the stored scale are
/// both unchanged. The panel's size function is a constant, so that guard
/// rested entirely on the scale, and the scale had already been written by
/// `remember_scale` before the event arrived. Both halves equal, early return,
/// no request, and a panel drawn at the new size inside a window still at the
/// old one.
///
/// So the request is now made directly, and this stands over it with a host
/// that records being asked.
mod resize {
    use super::*;
    use nih_plug::prelude::{GuiContext, ParamPtr, PluginApi, PluginState};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct CountingHost {
        resizes: AtomicUsize,
    }

    impl GuiContext for CountingHost {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        fn request_resize(&self) -> bool {
            self.resizes.fetch_add(1, Ordering::Relaxed);
            true
        }
        unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
        unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            unimplemented!("the panel never asks the host for its state")
        }
        fn set_state(&self, _: PluginState) {
            unimplemented!("the panel never hands the host a state")
        }
    }

    #[test]
    fn choosing_a_size_asks_the_host_to_resize_the_window() {
        let state = default_state();
        let host = Arc::new(CountingHost::default());

        gainstagefx::editor::apply_scale(&state, &*host, 1.5);

        assert_eq!(
            host.resizes.load(Ordering::Relaxed),
            1,
            "the size was stored but the host was never asked for a window to \
             put it in, which leaves the panel drawn larger than its window"
        );
        assert_eq!(
            state.user_scale_factor(),
            1.5,
            "the host was asked to resize before the size it would read was set"
        );
    }

    /// And the host must be asked for the size that was just chosen, not the
    /// one before it. `Editor::size` reads `scaled_logical_size`, so the store
    /// has to happen first; this pins that order.
    #[test]
    fn the_host_is_asked_for_the_new_size_and_not_the_old_one() {
        let state = default_state();
        let (before, _) = state.scaled_logical_size();

        struct CheckingHost {
            state: Arc<nih_plug_vizia::ViziaState>,
            saw: AtomicUsize,
        }
        impl GuiContext for CheckingHost {
            fn plugin_api(&self) -> PluginApi {
                PluginApi::Clap
            }
            fn request_resize(&self) -> bool {
                // What the host would read the moment it is asked.
                self.saw.store(
                    self.state.scaled_logical_size().0 as usize,
                    Ordering::Relaxed,
                );
                true
            }
            unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
            unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
            unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
            fn get_state(&self) -> PluginState {
                unimplemented!()
            }
            fn set_state(&self, _: PluginState) {
                unimplemented!()
            }
        }

        let host = Arc::new(CheckingHost {
            state: state.clone(),
            saw: AtomicUsize::new(0),
        });
        gainstagefx::editor::apply_scale(&state, &*host, 2.0);

        let saw = host.saw.load(Ordering::Relaxed);
        assert!(
            saw > before as usize,
            "the host was asked for {saw} px, the width before the change was \
             {before} -- it was asked for the old size"
        );
        assert_eq!(saw, state.scaled_logical_size().0 as usize);
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
                (uw as f64 * scale).round() as u32,
                (uh as f64 * scale).round() as u32
            ),
            "the window the host is told to make at {scale:.2} does not match \
             the scale the panel will draw at"
        );
    }
}
