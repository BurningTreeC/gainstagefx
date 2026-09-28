use crate::application::ApplicationRunner;
use baseview::gl::GlConfig;
use baseview::{
    Event, EventStatus, HandlerError, Window, WindowContext, WindowHandler, WindowSettings,
};
use gl::types::GLint;
use gl_rs as gl;
use skia_safe::gpu::gl::FramebufferInfo;
use skia_safe::gpu::{
    self, ContextOptions, SurfaceOrigin, backend_render_targets, ganesh::context_options,
};
use skia_safe::{ColorSpace, ColorType, PixelGeometry, Surface, SurfaceProps, SurfacePropsFlags};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use crate::proxy::BaseviewProxy;
use vizia_core::backend::*;
use vizia_core::prelude::*;

/// Handles a vizia_baseview application
pub(crate) struct ViziaWindow {
    application: RefCell<ApplicationRunner>,
    pending: RefCell<VecDeque<Pending>>,
    text_focused: Cell<bool>,
    #[allow(clippy::type_complexity)]
    on_idle: Option<Box<dyn Fn(&mut Context) + Send>>,
}

enum Pending {
    Event(Event),
    Resize(baseview::WindowSize),
}

impl ViziaWindow {
    fn new(
        mut cx: BackendContext,
        win_desc: WindowDescription,
        window: WindowContext,
        builder: Option<Box<dyn FnOnce(&mut Context) + Send>>,
        on_idle: Option<Box<dyn Fn(&mut Context) + Send>>,
        is_parented: bool,
        physical_scale: bool,
    ) -> Result<ViziaWindow, HandlerError> {
        // Reactive runtime setup — mirrors what `vizia_winit::Application::new` does.
        //
        // Mark this thread as the UI thread so off-thread `SyncSignal` writes correctly
        // enqueue effects via `SYNC_RUNTIME` instead of the thread-local `RUNTIME` (which
        // nobody drains). `on_frame_update` later calls `Runtime::drain_pending_work` to
        // apply those effects on this thread.
        Runtime::init_on_ui_thread();

        // Register a no-op sync waker. baseview doesn't expose a proxy-event primitive we
        // can use to wake the event loop from another thread, but it drives `on_frame` at
        // the host compositor's vsync rate anyway — signal changes from off-UI writes will
        // be observed on the next frame (at most one frame of latency).
        //
        // Registering the waker here — even as a no-op — has two benefits: (1) it matches
        // the pattern in `vizia_winit`, so embedders don't have to know about sync-runtime
        // internals; (2) if a future baseview release gains a window-wake mechanism, only
        // this one line needs to change.
        Runtime::set_sync_effect_waker(|| {});
        let context = window.gl_context().expect("Window was created without OpenGL support");

        unsafe { context.make_current() }?;

        // Build skia renderer
        gl::load_with(|s| context.get_proc_address_from_str(s));
        let interface = skia_safe::gpu::gl::Interface::new_load_with(|name| {
            if name == "eglGetCurrentDisplay" {
                return std::ptr::null();
            }
            context.get_proc_address_from_str(name)
        })
        .expect("Could not create interface");

        // https://github.com/rust-skia/rust-skia/issues/476
        let mut context_options = ContextOptions::new();
        context_options.skip_gl_error_checks = context_options::Enable::Yes;

        let mut gr_context = skia_safe::gpu::direct_contexts::make_gl(interface, &context_options)
            .expect("Could not create direct context");
        gr_context.set_resource_cache_limit(win_desc.skia_resource_cache_limit);

        let fb_info = {
            let mut fboid: GLint = 0;
            unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fboid) };

            FramebufferInfo {
                fboid: fboid.try_into().unwrap(),
                format: skia_safe::gpu::gl::Format::RGBA8.into(),
                ..Default::default()
            }
        };

        let initial_size = window.size();
        let initial_width = initial_size.physical.width.max(1) as i32;
        let initial_height = initial_size.physical.height.max(1) as i32;

        let mut surface = create_surface((initial_width, initial_height), fb_info, &mut gr_context);

        let dirty_surface =
            surface.new_surface_with_dimensions((initial_width, initial_height)).unwrap();

        // Scaling is a combination of the window's current scale factor (system-provided unless
        // explicitly overridden by the hosting application) and a custom user scale factor.
        let window_scale_factor = window.scale_factor();
        let dpi_factor = if physical_scale {
            win_desc.user_scale_factor
        } else {
            window_scale_factor * win_desc.user_scale_factor
        };

        cx.add_main_window(Entity::root(), &win_desc, dpi_factor as f32);
        cx.add_window(WindowView {});

        cx.0.windows.insert(
            Entity::root(),
            WindowState { window_description: win_desc.clone(), ..Default::default() },
        );

        TextInputModel(window.clone()).build(cx.context());
        cx.context().add_built_in_styles();
        if let Some(builder) = builder {
            (builder)(cx.context());
        }

        let application = ApplicationRunner::new(
            cx,
            gr_context,
            window_scale_factor,
            surface,
            dirty_surface,
            win_desc,
            is_parented,
            physical_scale,
            window,
        );
        unsafe { context.make_not_current() }?;

        Ok(ViziaWindow {
            application: RefCell::new(application),
            on_idle,
            pending: RefCell::new(VecDeque::new()),
            text_focused: Cell::new(false),
        })
    }

    pub fn create<F: Fn(&mut Context) + Send + 'static>(
        win_desc: WindowDescription,
        fallback_scale_factor: Option<f64>,
        app: F,
        on_idle: Option<Box<dyn Fn(&mut Context) + Send>>,
        ignore_default_theme: bool,
        physical_scale: bool,
        settings: WindowSettings,
        host: Option<baseview::host::Host>,
    ) -> Result<Window, baseview::Error> {
        let is_parented = settings.parent.is_some() || settings.wait_for_parent;
        let size = baseview::dpi::LogicalSize::new(
            win_desc.inner_size.width as f64 * win_desc.user_scale_factor,
            win_desc.inner_size.height as f64 * win_desc.user_scale_factor,
        );
        let size: baseview::dpi::Size =
            if physical_scale { size.to_physical::<u32>(1.0).into() } else { size.into() };
        let settings = settings
            .with_title(win_desc.title.clone())
            .with_size(size)
            .with_fallback_scale_factor(fallback_scale_factor)
            .with_gl_config(GlConfig::default());
        Window::create_with_host(
            settings,
            move |window| {
                Runtime::init_on_ui_thread();
                let mut cx = Context::new();
                cx.ignore_default_theme = ignore_default_theme;
                cx.add_built_in_styles();
                let mut cx = BackendContext::new(cx);
                cx.set_event_proxy(Box::new(BaseviewProxy));
                Self::new(
                    cx,
                    win_desc,
                    window,
                    Some(Box::new(app)),
                    on_idle,
                    is_parented,
                    physical_scale,
                )
            },
            host,
        )
    }

    // Native callbacks may reenter during resize/focus. Retain their order instead
    // of discarding releases (which would leave parameter gestures open).
    fn drain(&self, app: &mut ApplicationRunner) -> Result<(), HandlerError> {
        loop {
            let pending = self.pending.borrow_mut().pop_front();
            match pending {
                Some(Pending::Event(event)) => app.handle_event(event),
                Some(Pending::Resize(size)) => app.handle_resized(size)?,
                None => break,
            }
        }
        Ok(())
    }
}

impl WindowHandler for ViziaWindow {
    fn on_frame(&self) -> Result<(), HandlerError> {
        Runtime::init_on_ui_thread();
        let Ok(mut app) = self.application.try_borrow_mut() else {
            return Ok(());
        };
        self.drain(&mut app)?;
        app.on_frame_update()?;
        app.handle_idle(&self.on_idle);
        self.drain(&mut app)?;
        self.text_focused.set(app.focused_element() == Some("textbox"));
        app.render()
    }

    fn resized(&self, size: baseview::WindowSize) -> Result<(), HandlerError> {
        Runtime::init_on_ui_thread();
        self.pending.borrow_mut().push_back(Pending::Resize(size));
        if let Ok(mut app) = self.application.try_borrow_mut() {
            self.drain(&mut app)?;
        }
        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        Runtime::init_on_ui_thread();
        let captured = matches!(event, Event::Keyboard(_)) && self.text_focused.get();
        self.pending.borrow_mut().push_back(Pending::Event(event));
        if let Ok(mut app) = self.application.try_borrow_mut() {
            if let Err(error) = self.drain(&mut app) {
                log::error!("Vizia event dispatch: {error}");
            }
            app.handle_idle(&self.on_idle);
            self.text_focused.set(app.focused_element() == Some("textbox"));
        }
        if captured { EventStatus::Captured } else { EventStatus::Ignored }
    }
}

impl Drop for ViziaWindow {
    fn drop(&mut self) {
        Runtime::deinit_on_ui_thread();
    }
}

pub struct WindowView {}

impl View for WindowView {}

pub fn create_surface(
    size: (i32, i32),
    fb_info: FramebufferInfo,
    gr_context: &mut skia_safe::gpu::DirectContext,
) -> Surface {
    let backend_render_target = backend_render_targets::make_gl(size, None, 8, fb_info);

    let surface_props = SurfaceProps::new_with_text_properties(
        SurfacePropsFlags::default(),
        PixelGeometry::default(),
        0.5,
        0.0,
    );

    gpu::surfaces::wrap_backend_render_target(
        gr_context,
        &backend_render_target,
        SurfaceOrigin::BottomLeft,
        ColorType::RGBA8888,
        ColorSpace::new_srgb(),
        Some(surface_props).as_ref(),
        // None,
    )
    .expect("Could not create skia surface")
}

struct TextInputModel(WindowContext);
impl Model for TextInputModel {
    fn event(&mut self, _cx: &mut EventContext, event: &mut vizia_core::prelude::Event) {
        event.map(|input: &crate::TextInputActive, _| self.0.set_text_input(input.0));
    }
}
