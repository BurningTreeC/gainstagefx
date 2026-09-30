use crate::window::ViziaWindow;
use crate::window::create_surface;
use baseview::{Window, WindowContext, WindowSettings};
use gl_rs as gl;
use gl_rs::types::GLint;
use raw_window_handle::HasWindowHandle;
use skia_safe::gpu::gl::FramebufferInfo;
use vizia_core::events::EventManager;

use crate::proxy::queue_get;
use vizia_core::backend::*;
use vizia_core::prelude::*;

pub type ApplicationError = baseview::Error;

///Creating a new application creates a root `Window` and a `Context`. Views declared within the closure passed to `Application::new()` are added to the context and rendered into the root window.
///
/// # Example
/// ```no_run
/// # use vizia_core::prelude::*;
/// # use vizia_baseview::Application;
///
/// Application::new(|cx|{
///    // Content goes here
/// })
/// .run();
///```
/// Calling `run()` on the `Application` causes the program to enter the event loop and for the main window to display.
pub struct Application<F>
where
    F: Fn(&mut Context) + Send + 'static,
{
    app: F,
    window_description: WindowDescription,
    fallback_scale_factor: Option<f64>,
    on_idle: Option<Box<dyn Fn(&mut Context) + Send>>,
    ignore_default_theme: bool,
    physical_scale: bool,
}

impl<F> Application<F>
where
    F: Fn(&mut Context),
    F: 'static + Send,
{
    pub fn new(app: F) -> Self {
        Self {
            app,
            window_description: WindowDescription::new(),
            fallback_scale_factor: None,
            on_idle: None,
            ignore_default_theme: false,
            physical_scale: false,
        }
    }

    /// Sets the default built-in theming to be ignored.
    pub fn ignore_default_theme(mut self) -> Self {
        self.ignore_default_theme = true;
        self
    }

    /// Interpret user scale as physical pixels per design unit, independent of
    /// the host/OS DPI suggestion. Useful for an explicit fixed base DPI policy.
    pub fn use_physical_scale(mut self) -> Self {
        self.physical_scale = true;
        self
    }

    pub fn with_fallback_scale_factor(mut self, factor: Option<f64>) -> Self {
        self.fallback_scale_factor = factor;
        self
    }

    /// Sets the window title from a value that can be localized.
    ///
    /// Unlike the winit backend this resolves the title once during application setup.
    pub fn title<T: ToStringLocalized>(mut self, title: impl Res<T>) -> Self {
        let cx = Context::default();
        self.window_description.title = title.get_value(&cx).to_string_local(&cx);

        self
    }

    pub fn inner_size(mut self, size: impl Into<WindowSize>) -> Self {
        self.window_description.inner_size = size.into();

        self
    }

    /// A scale factor applied on top of any DPI scaling, defaults to 1.0.
    pub fn user_scale_factor(mut self, factor: f64) -> Self {
        self.window_description.user_scale_factor = factor;

        self
    }

    /// Sets the maximum number of bytes Skia may use for cached GPU resources.
    pub fn skia_resource_cache_limit(mut self, limit: usize) -> Self {
        self.window_description.skia_resource_cache_limit = limit;

        self
    }

    /// Open a new window that blocks the current thread until the window is destroyed.
    ///
    /// Do **not** use this in the context of audio plugins, unless it is compiled as a
    /// standalone application.
    ///
    /// * `app` - The Vizia application builder.
    pub fn run(self) -> Result<(), ApplicationError> {
        self.create(WindowSettings::new(), None)?.run_until_closed()
    }

    pub fn open_parented<P: HasWindowHandle>(self, parent: &P) -> Result<Window, ApplicationError> {
        let window = self.create(WindowSettings::new().with_parent(parent), None)?;
        window.show()?;
        Ok(window)
    }

    /// Create a host-managed window. The caller owns showing, hiding and reparenting it.
    pub fn create(
        self,
        settings: WindowSettings,
        host: Option<baseview::host::Host>,
    ) -> Result<Window, ApplicationError> {
        ViziaWindow::create(
            self.window_description,
            self.fallback_scale_factor,
            self.app,
            self.on_idle,
            self.ignore_default_theme,
            self.physical_scale,
            settings,
            host,
        )
    }

    /// Takes a closure which will be called at the end of every loop of the application.
    ///
    /// The callback provides a place to run 'idle' processing and happens at the end of each loop but before drawing.
    /// If the callback pushes events into the queue in context then the event loop will re-run. Care must be taken not to
    /// push events into the queue every time the callback runs unless this is intended.
    ///
    /// # Example
    /// ```no_run
    /// # use vizia_core::prelude::*;
    /// # use vizia_baseview::Application;
    /// Application::new(|cx|{
    ///     // Build application here
    /// })
    /// .on_idle(|cx|{
    ///     // Code here runs at the end of every event loop after OS and vizia events have been handled
    /// })
    /// .run();
    /// ```
    pub fn on_idle<I: 'static + Fn(&mut Context) + Send>(mut self, callback: I) -> Self {
        self.on_idle = Some(Box::new(callback));

        self
    }
}

pub(crate) struct ApplicationRunner {
    cx: BackendContext,
    event_manager: EventManager,
    window_context: WindowContext,
    pub gr_context: skia_safe::gpu::DirectContext,
    should_redraw: bool,

    /// Native DPI scaling, multiplied by the user zoom for layout and drawing.
    window_scale_factor: f64,
    pub surface: skia_safe::Surface,
    pub dirty_surface: skia_safe::Surface,
    window_description: WindowDescription,
    requested_user_scale: f64,
    /// The unzoomed size before a `WindowEvent::SetSize` still waiting for
    /// the native size to confirm it; restored if the host keeps that size.
    previous_inner_size: Option<WindowSize>,
    /// `true` when the underlying baseview window was opened via
    /// a parent or `wait_for_parent` setting (i.e. the application is embedded
    /// inside a host such as a DAW). Gates lifecycle decisions that
    /// should be left to the host: when parented, vizia_baseview must
    /// not interpret Cmd+Q as a close request, because closing the
    /// child window without the host's knowledge leaves the host with
    /// an empty plug-in shell.
    is_parented: bool,
    physical_scale: bool,
}

impl ApplicationRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cx: BackendContext,
        gr_context: skia_safe::gpu::DirectContext,
        window_scale_factor: f64,
        surface: skia_safe::Surface,
        dirty_surface: skia_safe::Surface,
        window_description: WindowDescription,
        is_parented: bool,
        physical_scale: bool,
        window_context: WindowContext,
    ) -> Self {
        ApplicationRunner {
            should_redraw: true,
            gr_context,
            event_manager: EventManager::new(),
            window_context,
            window_scale_factor,
            cx,
            surface,
            dirty_surface,
            requested_user_scale: window_description.user_scale_factor,
            previous_inner_size: None,
            window_description,
            is_parented,
            physical_scale,
        }
    }

    /// Request the native resize. X11 reports its result asynchronously, so the
    /// drawing scale is committed by `handle_resized`, including a host rollback.
    fn apply_user_scale(&mut self, scale: f64) {
        let requested = crate::request_user_scale(
            self.requested_user_scale,
            scale,
            (self.window_description.inner_size.width, self.window_description.inner_size.height),
            |size| {
                if self.physical_scale {
                    self.window_context.resize(size.to_physical::<u32>(1.0)).is_ok()
                } else {
                    self.window_context.resize(size).is_ok()
                }
            },
        );
        if let Some(scale) = requested {
            self.requested_user_scale = scale;
        }
    }

    /// Change the window's unzoomed size and keep its zoom: the content has
    /// grown or shrunk, the way an editor with collapsible sections does.
    /// Like a zoom, the new size is committed by `handle_resized`, and a host
    /// that keeps the old one puts the old one back.
    fn apply_inner_size(&mut self, size: WindowSize) {
        let previous = self.window_description.inner_size;
        if size == previous || size.width == 0 || size.height == 0 {
            return;
        }
        let scale = self.requested_user_scale;
        let logical = baseview::dpi::LogicalSize::new(
            size.width as f64 * scale,
            size.height as f64 * scale,
        );
        self.window_description.inner_size = size;
        let requested = if self.physical_scale {
            self.window_context.resize(logical.to_physical::<u32>(1.0)).is_ok()
        } else {
            self.window_context.resize(logical).is_ok()
        };
        if requested {
            self.previous_inner_size.get_or_insert(previous);
        } else {
            self.window_description.inner_size = previous;
        }
    }

    /// Handle all reactivity within a frame. The window instance is used to resize the window when
    /// needed.
    pub fn on_frame_update(&mut self) -> Result<(), baseview::HandlerError> {
        // Pick up any effects enqueued by off-UI-thread `SyncSignal` writes since the last
        // frame. These sit in `SYNC_RUNTIME` until a UI-thread call processes them — this
        // is the analogue of the `drain_pending_work` call in `vizia_winit`'s frame loop.
        Runtime::drain_pending_work();

        while let Some(event) = queue_get() {
            self.cx.send_event(event.into_event());
        }

        // Events. The flush callback can't borrow `&mut self`, so size /
        // scale change requests get latched into locals here and applied
        // after the drain.
        let mut pending_user_scale: Option<f64> = None;
        let mut pending_inner_size: Option<WindowSize> = None;
        self.event_manager.flush_events(self.cx.context(), |window_event| {
            // For some reason calling window.close() crashes baseview on macos
            // WindowEvent::WindowClose => *should_close = true,
            match window_event {
                WindowEvent::FocusIn => {
                    #[cfg(not(target_os = "linux"))] // not implemented for linux yet
                    if !self.window_context.has_focus() {
                        let _ = self.window_context.focus();
                    }
                }
                WindowEvent::SetUserScale(factor) => {
                    pending_user_scale = Some(*factor);
                }
                WindowEvent::SetSize(size) => {
                    pending_inner_size = Some(*size);
                }
                _ => {}
            }
        });

        if let Some(new_user_scale) = pending_user_scale {
            self.apply_user_scale(new_user_scale);
        }
        if let Some(size) = pending_inner_size {
            self.apply_inner_size(size);
        }

        let context =
            self.window_context.gl_context().expect("Window was created without OpenGL support");
        unsafe { context.make_current() }?;
        self.cx.process_style_updates();
        unsafe { context.make_not_current() }?;

        self.cx.process_animations();

        self.cx.process_visual_updates();

        if self.cx.0.windows.iter().any(|(_, window_state)| !window_state.redraw_list.is_empty()) {
            self.should_redraw = true;
        }

        self.cx.process_timers();
        Ok(())
    }

    pub fn render(&mut self) -> Result<(), baseview::HandlerError> {
        if self.should_redraw {
            let context = self
                .window_context
                .gl_context()
                .expect("Window was created without OpenGL support");
            unsafe { context.make_current() }?;
            self.cx.draw(Entity::root(), &mut self.surface, &mut self.dirty_surface);
            self.gr_context.flush_and_submit();
            self.should_redraw = false;
            context.swap_buffers()?;
            unsafe { context.make_not_current() }?;
        }
        Ok(())
    }

    pub fn handle_event(&mut self, event: baseview::Event) {
        if requests_exit(&event, self.is_parented) {
            self.cx.send_event(Event::new(WindowEvent::WindowClose));
            self.window_context.request_close();
        }

        let mut update_modifiers = |modifiers: vizia_input::KeyboardModifiers| {
            self.cx
                .modifiers()
                .set(Modifiers::SHIFT, modifiers.contains(vizia_input::KeyboardModifiers::SHIFT));
            self.cx
                .modifiers()
                .set(Modifiers::CTRL, modifiers.contains(vizia_input::KeyboardModifiers::CONTROL));
            self.cx
                .modifiers()
                .set(Modifiers::SUPER, modifiers.contains(vizia_input::KeyboardModifiers::META));
            self.cx
                .modifiers()
                .set(Modifiers::ALT, modifiers.contains(vizia_input::KeyboardModifiers::ALT));
        };

        match event {
            baseview::Event::Mouse(event) => match event {
                baseview::MouseEvent::CursorMoved { position, modifiers } => {
                    update_modifiers(modifiers);

                    // baseview delivers cursor coordinates in physical pixels, so no
                    // additional DPI scaling should be applied here.
                    let cursor_x = position.x as f32;
                    let cursor_y = position.y as f32;
                    self.cx.emit_origin(WindowEvent::MouseMove(cursor_x, cursor_y));
                }
                baseview::MouseEvent::ButtonPressed { button, modifiers } => {
                    update_modifiers(modifiers);

                    let b = translate_mouse_button(button);
                    self.cx.emit_origin(WindowEvent::MouseDown(b));
                }
                baseview::MouseEvent::ButtonReleased { button, modifiers } => {
                    update_modifiers(modifiers);

                    let b = translate_mouse_button(button);
                    self.cx.emit_origin(WindowEvent::MouseUp(b));
                }
                baseview::MouseEvent::WheelScrolled { delta, modifiers } => {
                    update_modifiers(modifiers);

                    let (lines_x, lines_y) = match delta {
                        baseview::ScrollDelta::Lines { x, y } => (x, y),
                        baseview::ScrollDelta::Pixels { x, y } => (
                            if x < 0.0 {
                                -1.0
                            } else if x > 1.0 {
                                1.0
                            } else {
                                0.0
                            },
                            if y < 0.0 {
                                -1.0
                            } else if y > 1.0 {
                                1.0
                            } else {
                                0.0
                            },
                        ),
                    };

                    self.cx.emit_origin(WindowEvent::MouseScroll(lines_x, lines_y));
                }

                baseview::MouseEvent::CursorEntered => {
                    self.cx.emit_origin(WindowEvent::MouseEnter);
                }

                baseview::MouseEvent::CursorLeft => {
                    self.cx.emit_origin(WindowEvent::MouseLeave);
                }

                _ => {}
            },
            baseview::Event::Keyboard(event) => {
                let (s, pressed) = match event.state {
                    vizia_input::KeyState::Down => (MouseButtonState::Pressed, true),
                    vizia_input::KeyState::Up => (MouseButtonState::Released, false),
                };

                match event.code {
                    Code::ShiftLeft | Code::ShiftRight => {
                        self.cx.modifiers().set(Modifiers::SHIFT, pressed)
                    }
                    Code::ControlLeft | Code::ControlRight => {
                        self.cx.modifiers().set(Modifiers::CTRL, pressed)
                    }
                    Code::AltLeft | Code::AltRight => {
                        self.cx.modifiers().set(Modifiers::ALT, pressed)
                    }
                    Code::MetaLeft | Code::MetaRight => {
                        self.cx.modifiers().set(Modifiers::SUPER, pressed)
                    }
                    _ => (),
                }

                match s {
                    MouseButtonState::Pressed => {
                        if let vizia_input::Key::Character(written) = &event.key {
                            for chr in written.chars() {
                                self.cx.emit_origin(WindowEvent::CharInput(chr));
                            }
                        }

                        self.cx.emit_origin(WindowEvent::KeyDown(event.code, Some(event.key)));
                    }

                    MouseButtonState::Released => {
                        self.cx.emit_origin(WindowEvent::KeyUp(event.code, Some(event.key)));
                    }
                }
            }
            baseview::Event::Window(event) => match event {
                baseview::WindowEvent::Focused => self.cx.needs_refresh(Entity::root()),
                baseview::WindowEvent::WillClose => {
                    self.cx.send_event(Event::new(WindowEvent::WindowClose));
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn handle_resized(
        &mut self,
        new_size: baseview::WindowSize,
    ) -> Result<(), baseview::HandlerError> {
        let context =
            self.window_context.gl_context().expect("Window was created without OpenGL support");
        unsafe { context.make_current() }?;

        let fb_info = {
            let mut fboid: GLint = 0;
            unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid) };

            FramebufferInfo {
                fboid: fboid.try_into().unwrap(),
                format: skia_safe::gpu::gl::Format::RGBA8.into(),
                ..Default::default()
            }
        };

        let width = new_size.physical.width.max(1) as i32;
        let height = new_size.physical.height.max(1) as i32;

        self.surface = create_surface((width, height), fb_info, &mut self.gr_context);

        self.dirty_surface = self.surface.new_surface_with_dimensions((width, height)).unwrap();

        unsafe { context.make_not_current() }?;

        let committed = if self.physical_scale {
            (new_size.physical.width as f64, new_size.physical.height as f64)
        } else {
            (new_size.logical.width, new_size.logical.height)
        };
        // A new unzoomed size in flight: confirmed when the window arrives at
        // it, undone when the host kept the old one.
        if let Some(previous) = self.previous_inner_size {
            let requested = self.window_description.inner_size;
            if let Some((width, height)) = crate::settle_inner_size(
                committed,
                self.requested_user_scale,
                (requested.width, requested.height),
                (previous.width, previous.height),
            ) {
                self.window_description.inner_size = WindowSize { width, height };
                self.previous_inner_size = None;
            }
        }
        let zoom = crate::resolve_user_scale(
            committed,
            (self.window_description.inner_size.width, self.window_description.inner_size.height),
            self.requested_user_scale,
        );
        self.window_description.user_scale_factor = zoom;
        self.requested_user_scale = zoom;
        self.cx
            .send_event(Event::new(crate::UserScaleChanged(zoom)).propagate(Propagation::Subtree));
        self.cx.send_event(
            Event::new(crate::WindowScaleChanged(new_size.scale_factor))
                .propagate(Propagation::Subtree),
        );
        self.window_scale_factor = new_size.scale_factor;

        self.cx.set_scale_factor(if self.physical_scale {
            self.window_description.user_scale_factor
        } else {
            self.window_scale_factor * self.window_description.user_scale_factor
        });

        self.cx.set_window_size(
            Entity::root(),
            new_size.physical.width as f32,
            new_size.physical.height as f32,
        );

        self.cx.needs_refresh(Entity::root());
        Ok(())
    }

    pub fn handle_idle(&mut self, on_idle: &Option<Box<dyn Fn(&mut Context) + Send>>) {
        if let Some(idle_callback) = on_idle {
            self.cx.set_current(Entity::root());
            (idle_callback)(self.cx.context());
        }
    }

    /// Element name of the view that currently has keyboard focus.
    pub fn focused_element(&self) -> Option<&'static str> {
        self.cx.focused_element()
    }
}

/// Returns true if the provided event should cause an [`Application`] to
/// exit.
///
/// `WindowEvent::WillClose` is honoured in both standalone and parented
/// modes — it's a legitimate close signal from baseview / the host.
///
/// On macOS, Cmd+Q is recognized as a quit shortcut **only when the
/// application is standalone** (`is_parented == false`). When
/// vizia_baseview is embedded as a child window inside a host (the
/// usual audio-plug-in setup), the host owns the application
/// lifecycle: pressing Cmd+Q should quit the host, not close the
/// plug-in's child window. Most hosts on macOS bind Cmd+Q to their
/// own menu's "Quit" item, so AppKit's `performKeyEquivalent:`
/// dispatch claims the key before the plug-in's NSView sees it.
/// Hosts with looser key dispatch (Bitwig observed 2026-04-28) do
/// forward it through, in which case the previous unconditional
/// match would tear down the plug-in's GL surface and leave the
/// host with an empty plug-in shell.
pub fn requests_exit(
    event: &baseview::Event,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] is_parented: bool,
) -> bool {
    match event {
        baseview::Event::Window(baseview::WindowEvent::WillClose) => true,
        #[cfg(target_os = "macos")]
        baseview::Event::Keyboard(event) if !is_parented => {
            if event.code == vizia_input::Code::KeyQ
                && event.modifiers == vizia_input::KeyboardModifiers::META
                && event.state == vizia_input::KeyState::Down
            {
                return true;
            }

            false
        }
        _ => false,
    }
}

fn translate_mouse_button(button: baseview::MouseButton) -> MouseButton {
    match button {
        baseview::MouseButton::Left => MouseButton::Left,
        baseview::MouseButton::Right => MouseButton::Right,
        baseview::MouseButton::Middle => MouseButton::Middle,
        baseview::MouseButton::Other(id) => MouseButton::Other(id as u16),
        baseview::MouseButton::Back => MouseButton::Other(4),
        baseview::MouseButton::Forward => MouseButton::Other(5),
        _ => MouseButton::Other(0),
    }
}
