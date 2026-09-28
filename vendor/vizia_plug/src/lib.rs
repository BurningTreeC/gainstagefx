//! [VIZIA](https://github.com/vizia/vizia) editor support for nice-plug.

// See the comment in the main `nice_plug` crate
#![allow(clippy::type_complexity)]

use crossbeam::atomic::AtomicCell;
pub use editor::ViziaEditor;
use nice_plug_core::context::gui::GuiContext;
use nice_plug_core::params::persist::PersistentField;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use vizia::prelude::*;

// Re-export for convenience
pub use vizia;

mod editor;
pub mod widgets;

use widgets::param_registry::ParamRegistry;

/// Create an [`Editor`] instance using a [`vizia`][::vizia] GUI. The [`ViziaState`] passed to this
/// function contains the GUI's intitial size, and this is kept in sync whenever the GUI gets
/// resized. You can also use this to know if the GUI is open, so you can avoid performing
/// potentially expensive calculations while the GUI is not open. If you want this size to be
/// persisted when restoring a plugin instance, then you can store it in a `#[persist = "key"]`
/// field on your parameters struct.
///
/// The [`GuiContext`] is also passed to the app function. This is only meant for saving and
/// restoring state as part of your plugin's preset handling. You should not interact with this
/// directly to set parameters. Use the [`ParamEvent`][widgets::ParamEvent]s to change parameter
/// values, and [`GuiContextEvent`] to trigger window resizes.
///
/// The `theming` argument controls what level of theming to apply. If you use
/// [`ViziaTheming::Custom`], then you **need** to call
/// [`vizia_plug::assets::register_noto_sans_light()`][assets::register_noto_sans_light()] at
/// the start of your app function. Vizia's included fonts are also not registered by default. If
/// you use the Roboto font that normally comes with Vizia or any of its emoji or icon fonts, you
/// also need to register those using the functions in
/// [`vizia_plug::vizia_assets`][crate::vizia_assets].
///
/// See [VIZIA](https://github.com/vizia/vizia)'s repository for examples on how to use this.
pub fn create_vizia_editor<F>(
    vizia_state: Arc<ViziaState>,
    theming: ViziaTheming,
    app: F,
) -> Option<ViziaEditor>
where
    F: Fn(&mut Context, GuiContext) + 'static + Send + Sync,
{
    Some(editor::ViziaEditor {
        vizia_state,
        app: Arc::new(app),
        theming,

        emit_parameters_changed_event: Arc::new(AtomicBool::new(false)),
        param_registry: ParamRegistry::new(),
        key_inject: editor::KeyInjectState::new(),
    })
}

/// Controls what level of theming to apply to the editor.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub enum ViziaTheming {
    /// Disable both `vizia_plug`'s and vizia's built-in theming.
    None,
    /// Disable `vizia_plug`'s custom theming. Vizia's included fonts are also not registered by
    /// default. If you use the Roboto font that normally comes with Vizia or any of its emoji or
    /// icon fonts, you need to register those using the functions in
    /// [`vizia_plug::vizia_assets`][crate::vizia_assets].
    Builtin,
    /// Apply `vizia_plug`'s custom theming. This is the default. You **need** to call
    /// [`vizia_plug::assets::register_noto_sans_light()`][assets::register_noto_sans_light()]
    /// at the start of your app function for the font to work correctly.
    #[default]
    Custom,
}

/// State for an `vizia_plug` editor. The scale factor can be manipulated at runtime using
/// `cx.set_user_scale_factor()`.
#[derive(Serialize, Deserialize)]
pub struct ViziaState {
    /// A function that returns the window's current size in logical pixels, before any sort of
    /// scaling is applied. This size can be computed based on the plugin's current state.
    #[serde(skip, default = "empty_size_fn")]
    size_fn: Box<dyn Fn() -> (u32, u32) + Send + Sync>,
    /// A scale factor that should be applied to `size` separate from from any system HiDPI scaling.
    /// This can be used to allow GUIs to be scaled uniformly.
    #[serde(with = "nice_plug_core::params::persist::serialize_atomic_cell")]
    scale_factor: AtomicCell<f64>,
    /// Application rendering scale at 100% zoom, independent of saved user zoom.
    #[serde(skip, default = "default_base_scale")]
    base_scale_factor: f64,
    #[serde(skip)]
    physical_scale: bool,
    /// Whether the editor's window is currently open.
    #[serde(skip)]
    open: AtomicBool,
    #[serde(skip, default = "default_system_scale")]
    system_scale_factor: AtomicCell<f64>,
}

/// A default implementation for `size_fn` needed to be able to derive the `Deserialize` trait.
fn empty_size_fn() -> Box<dyn Fn() -> (u32, u32) + Send + Sync> {
    Box::new(|| (0, 0))
}

impl Debug for ViziaState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (width, height) = (self.size_fn)();

        f.debug_struct("ViziaState")
            .field("size_fn", &format!("<fn> ({}, {})", width, height))
            .field("scale_factor", &self.scale_factor)
            .field("open", &self.open)
            .finish()
    }
}

impl<'a> PersistentField<'a, ViziaState> for Arc<ViziaState> {
    fn set(&self, new_value: ViziaState) {
        self.scale_factor.store(new_value.scale_factor.load());
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&ViziaState) -> R,
    {
        f(self)
    }
}

impl ViziaState {
    /// Initialize the GUI's state. This value can be passed to [`create_vizia_editor()`]. The
    /// callback always returns the window's current size is in logical pixels, so before it is
    /// multiplied by the DPI scaling factor. This size can be computed based on the plugin's
    /// current state.
    pub fn new(size_fn: impl Fn() -> (u32, u32) + Send + Sync + 'static) -> Arc<ViziaState> {
        Arc::new(ViziaState {
            size_fn: Box::new(size_fn),
            scale_factor: AtomicCell::new(1.0),
            base_scale_factor: 1.0,
            physical_scale: false,
            open: AtomicBool::new(false),
            system_scale_factor: default_system_scale(),
        })
    }

    /// The same as [`new()`][Self::new()], but with a separate initial scale factor. This scale
    /// factor gets applied on top of any HiDPI scaling, and it can be modified at runtime by
    /// changing `cx.set_user_scale_factor()`.
    pub fn new_with_default_scale_factor(
        size_fn: impl Fn() -> (u32, u32) + Send + Sync + 'static,
        default_scale_factor: f64,
    ) -> Arc<ViziaState> {
        Arc::new(ViziaState {
            size_fn: Box::new(size_fn),
            scale_factor: AtomicCell::new(default_scale_factor),
            base_scale_factor: 1.0,
            physical_scale: false,
            open: AtomicBool::new(false),
            system_scale_factor: default_system_scale(),
        })
    }

    /// Start at 100% user zoom with a fixed physical rendering scale, independent
    /// of host/OS DPI suggestions.
    /// Saved zoom replaces `scale_factor` without replacing this application setting.
    pub fn new_with_base_scale_factor(
        size_fn: impl Fn() -> (u32, u32) + Send + Sync + 'static,
        base_scale_factor: f64,
    ) -> Arc<ViziaState> {
        assert!(base_scale_factor.is_finite() && base_scale_factor > 0.0);
        Arc::new(ViziaState {
            size_fn: Box::new(size_fn),
            scale_factor: AtomicCell::new(1.0),
            base_scale_factor,
            physical_scale: true,
            open: AtomicBool::new(false),
            system_scale_factor: default_system_scale(),
        })
    }

    pub fn base_scale_factor(&self) -> f64 {
        self.base_scale_factor
    }

    /// Rendering scale; the fixed-base constructor expresses this in physical pixels.
    pub fn rendering_scale_factor(&self) -> f64 {
        self.base_scale_factor * self.user_scale_factor()
    }

    /// Returns a `(width, height)` pair for the current size of the GUI in logical pixels, after
    /// applying the user scale factor.
    pub fn scaled_logical_size(&self) -> (u32, u32) {
        let (logical_width, logical_height) = self.inner_logical_size();
        let scale_factor = self.rendering_scale_factor();

        (
            (logical_width as f64 * scale_factor).round() as u32,
            (logical_height as f64 * scale_factor).round() as u32,
        )
    }

    /// Returns a `(width, height)` pair for the current size of the GUI in logical pixels before
    /// applying the user scale factor.
    pub fn inner_logical_size(&self) -> (u32, u32) {
        (self.size_fn)()
    }

    /// Get the non-DPI related uniform scaling factor the GUI's size will be multiplied with. This
    /// can be changed by changing `cx.user_scale_factor`.
    pub fn user_scale_factor(&self) -> f64 {
        self.scale_factor.load()
    }

    /// Set the non-DPI related uniform scaling factor the GUI's size will be multiplied with.
    ///
    /// `Editor::size()` reads this via [`Self::scaled_logical_size`], so updating it before
    /// calling [`nice_plug::context::gui::GuiContext::request_resize`] makes the host see the
    /// freshly-zoomed dimensions and resize its parent window to match.
    ///
    /// On its own, this only changes what the host is told. To make the embedded vizia window
    /// actually re-lay-out at the new scale, emit
    /// [`vizia::prelude::WindowEvent::SetUserScale`] from the same handler. The
    /// `vizia_baseview` backend handles that event by resizing the embedded window, bumping
    /// `style.dpi_factor`, and refreshing the render surface in lockstep.
    pub fn set_user_scale_factor(&self, factor: f64) {
        self.scale_factor.store(factor);
    }

    /// Whether the GUI is currently visible.
    // Called `is_open()` instead of `open()` to avoid the ambiguity.
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}

fn default_system_scale() -> AtomicCell<f64> {
    AtomicCell::new(1.0)
}

fn default_base_scale() -> f64 {
    1.0
}
