#![allow(clippy::type_complexity)]
mod application;
mod parent_window;
pub(crate) mod proxy;
mod window;

pub use parent_window::ParentWindow;

pub use application::{Application, ApplicationError};

pub use baseview::{Window, WindowSettings};

/// Emitted when native geometry changes, including a rejected resize rollback.
#[derive(Clone, Copy, Debug)]
pub struct UserScaleChanged(pub f64);
/// Request exclusive text entry while a text dialog is open.
#[derive(Clone, Copy, Debug)]
pub struct TextInputActive(pub bool);

/// Submit a native resize request. This records the requested zoom only; drawing
/// and persistent state are updated by the subsequent size notification.
/// Duplicate requests leave the host alone.
pub fn request_user_scale(
    current: f64,
    requested: f64,
    logical_size: (u32, u32),
    resize: impl FnOnce(baseview::dpi::LogicalSize<f64>) -> bool,
) -> Option<f64> {
    if requested == current || !requested.is_finite() || requested <= 0.0 {
        return None;
    }
    let size = baseview::dpi::LogicalSize::new(
        logical_size.0 as f64 * requested,
        logical_size.1 as f64 * requested,
    );
    resize(size).then_some(requested)
}

#[derive(Clone, Copy, Debug)]
pub struct WindowScaleChanged(pub f64);

/// Settle a change of the unzoomed size (`WindowEvent::SetSize`) against the
/// size the OS committed at `zoom`: the requested size if the window arrived
/// at it, the previous one if the host kept that, `None` while it is still
/// neither -- an earlier resize can report in between.
pub fn settle_inner_size(
    committed: (f64, f64),
    zoom: f64,
    requested: (u32, u32),
    previous: (u32, u32),
) -> Option<(u32, u32)> {
    let at = |(width, height): (u32, u32)| {
        (committed.0 - width as f64 * zoom).abs() <= 1.0
            && (committed.1 - height as f64 * zoom).abs() <= 1.0
    };
    if at(requested) {
        Some(requested)
    } else if at(previous) {
        Some(previous)
    } else {
        None
    }
}

/// Resolve zoom from the size the OS actually committed. Preserve the requested
/// value when only pixel rounding differs; a refused host resize reports the old size.
pub fn resolve_user_scale(logical: (f64, f64), base: (u32, u32), requested: f64) -> f64 {
    if (logical.0 - base.0 as f64 * requested).abs() <= 1.0
        && (logical.1 - base.1 as f64 * requested).abs() <= 1.0
    {
        return requested;
    }
    (logical.0 / base.0 as f64).min(logical.1 / base.1 as f64).max(0.01)
}
