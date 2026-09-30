//! # Vizia

#![allow(clippy::uninlined_format_args)]

extern crate self as vizia;

#[cfg(all(not(feature = "baseview"), feature = "winit"))]
pub use vizia_winit::application::{Application, ApplicationError};

#[cfg(all(not(feature = "winit"), feature = "baseview"))]
pub use vizia_baseview::{Application, ApplicationError, ParentWindow, Window, WindowSettings};

pub use vizia_core::*;

pub mod prelude {
    pub use vizia_core::prelude::*;

    #[cfg(all(not(feature = "baseview"), feature = "winit"))]
    pub use vizia_winit::{
        ModifyWindow,
        application::{Application, ApplicationError},
        window::Window,
        window_modifiers::WindowModifiers,
    };

    #[cfg(all(not(feature = "winit"), feature = "baseview"))]
    pub use vizia_baseview::{Application, ApplicationError, Window, WindowSettings};
}

#[cfg(feature = "baseview")]
pub use vizia_baseview::{
    TextInputActive, UserScaleChanged, WindowScaleChanged, request_user_scale, resolve_user_scale, settle_inner_size,
};
