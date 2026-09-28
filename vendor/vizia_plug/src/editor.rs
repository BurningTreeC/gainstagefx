//! The [`Editor`] trait implementation for Vizia editors.

use nice_plug_core::context::gui::GuiContext;
use nice_plug_core::debug::*;
use nice_plug_core::editor::dpi::{LogicalSize, NativeSize};
use nice_plug_core::editor::{
    Editor, EditorHandle, HostMethods, Modifiers, ParentWindowHandle, SpawnedEditor, VirtualKeyCode,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use vizia::input::NamedKey;
use vizia::prelude::*;
use vizia::views::TextEvent;

use crate::widgets::RawParamEvent;
use crate::widgets::param_registry::ParamRegistry;
use crate::{ViziaState, ViziaTheming, widgets};

/// A key-down event queued by the host-thread
/// `Editor::on_virtual_key_from_host` callback, waiting for the next
/// `on_idle` tick to dispatch on the GUI thread.
///
/// Split between character input (goes through `TextEvent::InsertText`)
/// and non-printable control keys (goes through `WindowEvent::KeyDown`)
/// so the GUI-thread drain stays trivially correct without having to
/// re-guess a key's semantics: the classification is made on the host
/// thread from the host's virtual key code.
pub(crate) enum KeyInject {
    /// A printable character (derived from a virtual key that maps 1:1
    /// to a printable character: Space, Numpad0..Numpad9, the numpad
    /// operator keys, Equals). Dispatched as `TextEvent::InsertText`.
    Char(char),
    /// A non-printable key (Backspace, Enter, Tab, Escape, arrows,
    /// Home/End, Delete, F-keys, etc.). Dispatched as
    /// `WindowEvent::KeyDown(code, Some(key))` so the target view's
    /// own key handling (e.g. textbox's `WindowEvent::KeyDown`
    /// match arm) runs.
    ControlKey(Code, Key),
}

/// State shared between [`ViziaEditor`] (invoked from the host UI
/// thread) and the vizia `on_idle` callback (invoked on the GUI thread).
///
/// The `Editor::on_virtual_key_from_host` callback runs on the host
/// thread and cannot reach into the live vizia `Context`. Instead, it
/// consults `text_focused` (kept in sync from `on_idle`) to decide
/// whether to claim the key, and pushes a [`KeyInject`] into `pending`.
/// The next `on_idle` tick drains the queue and dispatches each entry
/// to the currently focused entity.
pub(crate) struct KeyInjectState {
    /// `true` while the vizia focused view reports element name `"textbox"`.
    /// Updated on every `on_idle` tick.
    text_focused: AtomicBool,
    /// Keys the host delivered via the virtual-key hook that still need
    /// to be dispatched into the focused view on the next idle tick.
    pending: Mutex<VecDeque<KeyInject>>,
}

impl KeyInjectState {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            text_focused: AtomicBool::new(false),
            pending: Mutex::new(VecDeque::new()),
        })
    }
}

/// An [`Editor`] implementation that calls a vizia draw loop.
pub struct ViziaEditor {
    pub(crate) vizia_state: Arc<ViziaState>,
    /// The user's app function.
    pub(crate) app: Arc<dyn Fn(&mut Context, GuiContext) + 'static + Send + Sync>,
    /// What level of theming to apply. See [`ViziaEditorTheming`].
    pub(crate) theming: ViziaTheming,

    /// Whether to emit a parameters changed event during the next idle callback. This is set in the
    /// `params_changed()` implementation and it can be used by widgets to explicitly
    /// check for new parameter values. This is useful when the parameter value is (indirectly) used
    /// to compute a property in an event handler. Like when positioning an element based on the
    /// display value's width.
    pub(crate) emit_parameters_changed_event: Arc<AtomicBool>,

    /// Shared registry of `SyncSignal<f32>`s tracking each parameter's live value. Widgets
    /// subscribe via `cx.data::<ParamRegistry>()`. The GUI idle callback flushes
    /// values after host notifications set the atomic dirty flag.
    pub(crate) param_registry: ParamRegistry,

    /// Shared state bridging the host-thread
    /// `on_virtual_key_from_host` callback with the GUI-thread
    /// `on_idle` callback. See [`KeyInjectState`].
    pub(crate) key_inject: Arc<KeyInjectState>,
}

impl Editor for ViziaEditor {
    type Handle = ViziaEditorHandle;

    fn spawn(
        &self,
        parent: Option<ParentWindowHandle>,
        wait_for_parent: bool,
        fallback_scale_factor: Option<f64>,
        context: GuiContext,
        host: Option<HostMethods>,
    ) -> Result<SpawnedEditor<Self::Handle>, Box<dyn std::error::Error>> {
        let app = self.app.clone();
        let vizia_state = self.vizia_state.clone();
        let theming = self.theming;
        let param_registry = self.param_registry.clone();
        let param_registry_for_build = param_registry.clone();

        // Prevent stale runtime-bound signals from previous editor instances from being reused.
        param_registry.clear_signals();

        let (unscaled_width, unscaled_height) = vizia_state.inner_logical_size();
        let user_scale_factor = vizia_state.rendering_scale_factor();

        let mut application = Application::new(move |cx| {
            // Set some default styles to match the iced integration
            //if theming >= ViziaTheming::Custom {
            // NOTE: `Context::set_default_font` was removed upstream as a deprecated API
            // (vizia commit ff943a0b, "Context: remove deprecated APIs and clarify docs").
            // The default font is now controlled through stylesheets — `theme.css` below
            // can set `* { font-family: ...; }` if a specific font is required.
            if let Err(err) = cx.add_stylesheet(include_style!("src/assets/theme.css")) {
                nice_error!("Failed to load stylesheet: {err:?}");
                panic!();
            }

            // There doesn't seem to be any way to bundle styles with a widget, so we'll always
            // include the style sheet for our custom widgets at context creation
            widgets::register_theme(cx);
            //}

            // Install the parameter signal registry so widgets can find it via
            // `cx.data::<ParamRegistry>()`. `ParamRegistry` is a cheap handle (Arc internally),
            // so the editor keeps a clone for flushing on parameter changes.
            param_registry_for_build.clone().build(cx);

            // Any widget can change the parameters by emitting `ParamEvent` events. This model will
            // handle them automatically.
            widgets::ParamModel {
                context: context.clone(),
            }
            .build(cx);

            GeometryModel(vizia_state.clone()).build(cx);
            app(cx, context.clone())
        })
        .with_fallback_scale_factor(if self.vizia_state.physical_scale {
            None
        } else {
            fallback_scale_factor
        })
        .inner_size((unscaled_width, unscaled_height))
        .user_scale_factor(user_scale_factor)
        .on_idle({
            let emit_parameters_changed_event = self.emit_parameters_changed_event.clone();
            let key_inject = self.key_inject.clone();
            let param_registry = self.param_registry.clone();
            move |cx| {
                if emit_parameters_changed_event
                    .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
                {
                    param_registry.flush_all();
                    cx.emit_custom(
                        Event::new(RawParamEvent::ParametersChanged)
                            .propagate(Propagation::Subtree),
                    );
                }

                // Keep `text_focused` in sync so the host-thread
                // `on_virtual_key_from_host` callback can decide
                // synchronously whether to claim a key. The element
                // name `"textbox"` is set by
                // `vizia::views::Textbox::element()`.
                key_inject
                    .text_focused
                    .store(cx.focused_element() == Some("textbox"), Ordering::Release);

                // Drain any keys queued by `on_virtual_key_from_host`
                // and dispatch them to the focused view. Buffered
                // inside a short-lived lock to keep the critical
                // section bounded.
                //
                // `on_virtual_key_from_host` has already classified
                // each item on the host thread using the VST3
                // `key_code`, so the drain just mechanically
                // dispatches each variant: chars via
                // `TextEvent::InsertText`, control keys via
                // `WindowEvent::KeyDown` so the focused view's own
                // key handler (e.g. textbox's `KeyDown` match arm)
                // runs.
                let drained: Vec<KeyInject> = {
                    let mut q = key_inject.pending.lock().unwrap_or_else(|e| e.into_inner());
                    q.drain(..).collect()
                };
                if !drained.is_empty() {
                    let mut ec = EventContext::new(cx);
                    let target = ec.focused();
                    for entry in drained {
                        match entry {
                            KeyInject::Char(c) => {
                                ec.emit_to(target, TextEvent::InsertText(c.to_string()));
                            }
                            KeyInject::ControlKey(code, key) => {
                                ec.emit_to(target, WindowEvent::KeyDown(code, Some(key)));
                            }
                        }
                    }
                }
            }
        });

        // This way the plugin can decide to use none of the built in theming
        if theming == ViziaTheming::None {
            application = application.ignore_default_theme();
        }

        if self.vizia_state.physical_scale {
            application = application.use_physical_scale();
        }

        let host = host.map(|host| {
            baseview::host::Host::new()
                .with_callbacks(HostCallbackAdapter(host.callbacks))
                .with_main_thread(HostMainThreadAdapter(host.main_thread_caller))
        });
        let window = application.create(
            baseview::WindowSettings::new()
                .with_parent(parent.as_ref())
                .with_wait_for_parent(wait_for_parent)
                // Permit programmatic zoom; Editor::resize_hint still prevents
                // arbitrary host resizing of the fixed-aspect panel.
                .with_resizable(true)
                // The host's viewport may be smaller than the requested zoom.
                // Only an explicit host resize may change this editor's size.
                .with_resize_to_parent(false),
            host,
        )?;
        self.vizia_state
            .system_scale_factor
            .store(window.size().scale_factor);
        self.vizia_state.open.store(true, Ordering::Release);
        Ok(SpawnedEditor {
            handle: ViziaEditorHandle {
                vizia_state: self.vizia_state.clone(),
                emit_parameters_changed_event: self.emit_parameters_changed_event.clone(),
                key_inject: self.key_inject.clone(),
            },
            window,
        })
    }

    fn size(&self) -> NativeSize<u32> {
        let (width, height) = self.vizia_state.scaled_logical_size();
        let size = LogicalSize::new(width, height);
        let size = if self.vizia_state.physical_scale {
            size.to_physical::<u32>(1.0).into()
        } else {
            size.into()
        };
        NativeSize::from_size(size, self.vizia_state.system_scale_factor.load())
    }
}

pub struct ViziaEditorHandle {
    vizia_state: Arc<ViziaState>,
    emit_parameters_changed_event: Arc<AtomicBool>,
    key_inject: Arc<KeyInjectState>,
}

impl EditorHandle for ViziaEditorHandle {
    type Window = baseview::Window;
    type Error = baseview::Error;

    fn run_until_closed(window: Self::Window) -> Result<(), Self::Error> {
        window.run_until_closed()
    }
    fn set_parent(
        &self,
        parent: ParentWindowHandle,
        window: &Self::Window,
    ) -> Result<(), Self::Error> {
        window.set_parent(&parent)
    }
    fn show(&self, window: &Self::Window) -> Result<(), Self::Error> {
        window.show()
    }
    fn hide(&self, window: &Self::Window) -> Result<(), Self::Error> {
        window.hide()
    }
    fn host_main_thread_callback(&self, window: &Self::Window) {
        window.host_main_thread_callback();
    }
    fn set_size(&self, size: NativeSize<u32>, window: &Self::Window) -> Result<(), Self::Error> {
        window.resize(baseview::dpi::NativeSize::new(size.width, size.height))
    }
    fn set_fallback_scale_factor(
        &self,
        scale: f64,
        window: &Self::Window,
    ) -> Result<(), Self::Error> {
        if self.vizia_state.physical_scale {
            // This editor defines zoom in physical pixels. A host DPI hint must
            // not resize it; native platform DPI is still used for coordinates.
            Ok(())
        } else {
            window.suggest_fallback_scale_factor(scale)
        }
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        // Push the new value into the registry's signals — observers bound via `Binding::new`
        // wake up and rebuild. Also flag a `ParametersChanged` idle event for any widgets that
        // still rely on the older (pre-signal) notification path.
        self.emit_parameters_changed_event
            .store(true, Ordering::Relaxed);
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {
        self.emit_parameters_changed_event
            .store(true, Ordering::Relaxed);
    }

    fn state_changed(&self) {
        self.emit_parameters_changed_event
            .store(true, Ordering::Relaxed);
    }

    fn on_virtual_key_from_host(
        &self,
        key_code: VirtualKeyCode,
        is_down: bool,
        modifiers: Modifiers,
    ) -> bool {
        // Called from the host's UI thread (e.g. REAPER dispatching
        // `IPlugView::onKeyDown` / `onKeyUp`). Claim the key only when
        // a textbox is currently focused; otherwise the host's
        // accelerator (e.g. space -> transport) should run normally.
        if !self.key_inject.text_focused.load(Ordering::Acquire) {
            return false;
        }

        // Modifier-held combinations (Cmd+A, Cmd+Left, Shift+Arrow,
        // Option+Backspace, etc.) are claimed by the host or handled by
        // AppKit's `keyDown:` + `doCommandBySelector:` path where
        // vizia's textbox reads modifier state for line/word movement.
        // Dispatching through our injection queue here would double-fire
        // and lose modifier context. Return `false` so the host keeps
        // the key and AppKit's normal path runs.
        if !modifiers.is_empty() {
            return false;
        }

        // Classify the virtual key. The host hands us virtual keys that
        // split into two groups vizia consumes differently:
        //
        // - Keys that represent a printable character (Space, numpad
        //   digits/operators, `=`) go in as `TextEvent::InsertText`.
        // - Named control keys (Backspace, Enter, arrows, F-keys,
        //   etc.) go in as `WindowEvent::KeyDown(code, Some(key))` so
        //   the focused view's own key handler (textbox's `KeyDown`
        //   match arm for Backspace / Enter / arrows) runs.
        //
        // Keys we don't enumerate here (media / volume keys, Select,
        // Print, modifier-only presses, Super) fall through to
        // `return false` so the host's own binding runs.
        let inject = match key_code {
            VirtualKeyCode::Space => Some(KeyInject::Char(' ')),
            VirtualKeyCode::Numpad0 => Some(KeyInject::Char('0')),
            VirtualKeyCode::Numpad1 => Some(KeyInject::Char('1')),
            VirtualKeyCode::Numpad2 => Some(KeyInject::Char('2')),
            VirtualKeyCode::Numpad3 => Some(KeyInject::Char('3')),
            VirtualKeyCode::Numpad4 => Some(KeyInject::Char('4')),
            VirtualKeyCode::Numpad5 => Some(KeyInject::Char('5')),
            VirtualKeyCode::Numpad6 => Some(KeyInject::Char('6')),
            VirtualKeyCode::Numpad7 => Some(KeyInject::Char('7')),
            VirtualKeyCode::Numpad8 => Some(KeyInject::Char('8')),
            VirtualKeyCode::Numpad9 => Some(KeyInject::Char('9')),
            VirtualKeyCode::NumpadMultiply => Some(KeyInject::Char('*')),
            VirtualKeyCode::NumpadAdd => Some(KeyInject::Char('+')),
            VirtualKeyCode::NumpadSeparator => Some(KeyInject::Char(',')),
            VirtualKeyCode::NumpadSubtract => Some(KeyInject::Char('-')),
            VirtualKeyCode::NumpadDecimal => Some(KeyInject::Char('.')),
            VirtualKeyCode::NumpadDivide => Some(KeyInject::Char('/')),
            VirtualKeyCode::Equals => Some(KeyInject::Char('=')),

            VirtualKeyCode::Backspace => Some(KeyInject::ControlKey(
                Code::Backspace,
                Key::Named(NamedKey::Backspace),
            )),
            VirtualKeyCode::Tab => {
                Some(KeyInject::ControlKey(Code::Tab, Key::Named(NamedKey::Tab)))
            }
            VirtualKeyCode::Return => Some(KeyInject::ControlKey(
                Code::Enter,
                Key::Named(NamedKey::Enter),
            )),
            VirtualKeyCode::NumpadEnter => Some(KeyInject::ControlKey(
                Code::NumpadEnter,
                Key::Named(NamedKey::Enter),
            )),
            VirtualKeyCode::Pause => Some(KeyInject::ControlKey(
                Code::Pause,
                Key::Named(NamedKey::Pause),
            )),
            VirtualKeyCode::Escape => Some(KeyInject::ControlKey(
                Code::Escape,
                Key::Named(NamedKey::Escape),
            )),
            VirtualKeyCode::End => {
                Some(KeyInject::ControlKey(Code::End, Key::Named(NamedKey::End)))
            }
            VirtualKeyCode::Home => Some(KeyInject::ControlKey(
                Code::Home,
                Key::Named(NamedKey::Home),
            )),
            VirtualKeyCode::ArrowLeft => Some(KeyInject::ControlKey(
                Code::ArrowLeft,
                Key::Named(NamedKey::ArrowLeft),
            )),
            VirtualKeyCode::ArrowUp => Some(KeyInject::ControlKey(
                Code::ArrowUp,
                Key::Named(NamedKey::ArrowUp),
            )),
            VirtualKeyCode::ArrowRight => Some(KeyInject::ControlKey(
                Code::ArrowRight,
                Key::Named(NamedKey::ArrowRight),
            )),
            VirtualKeyCode::ArrowDown => Some(KeyInject::ControlKey(
                Code::ArrowDown,
                Key::Named(NamedKey::ArrowDown),
            )),
            VirtualKeyCode::PageUp => Some(KeyInject::ControlKey(
                Code::PageUp,
                Key::Named(NamedKey::PageUp),
            )),
            VirtualKeyCode::PageDown => Some(KeyInject::ControlKey(
                Code::PageDown,
                Key::Named(NamedKey::PageDown),
            )),
            VirtualKeyCode::Insert => Some(KeyInject::ControlKey(
                Code::Insert,
                Key::Named(NamedKey::Insert),
            )),
            VirtualKeyCode::Delete => Some(KeyInject::ControlKey(
                Code::Delete,
                Key::Named(NamedKey::Delete),
            )),
            VirtualKeyCode::F1 => Some(KeyInject::ControlKey(Code::F1, Key::Named(NamedKey::F1))),
            VirtualKeyCode::F2 => Some(KeyInject::ControlKey(Code::F2, Key::Named(NamedKey::F2))),
            VirtualKeyCode::F3 => Some(KeyInject::ControlKey(Code::F3, Key::Named(NamedKey::F3))),
            VirtualKeyCode::F4 => Some(KeyInject::ControlKey(Code::F4, Key::Named(NamedKey::F4))),
            VirtualKeyCode::F5 => Some(KeyInject::ControlKey(Code::F5, Key::Named(NamedKey::F5))),
            VirtualKeyCode::F6 => Some(KeyInject::ControlKey(Code::F6, Key::Named(NamedKey::F6))),
            VirtualKeyCode::F7 => Some(KeyInject::ControlKey(Code::F7, Key::Named(NamedKey::F7))),
            VirtualKeyCode::F8 => Some(KeyInject::ControlKey(Code::F8, Key::Named(NamedKey::F8))),
            VirtualKeyCode::F9 => Some(KeyInject::ControlKey(Code::F9, Key::Named(NamedKey::F9))),
            VirtualKeyCode::F10 => {
                Some(KeyInject::ControlKey(Code::F10, Key::Named(NamedKey::F10)))
            }
            VirtualKeyCode::F11 => {
                Some(KeyInject::ControlKey(Code::F11, Key::Named(NamedKey::F11)))
            }
            VirtualKeyCode::F12 => {
                Some(KeyInject::ControlKey(Code::F12, Key::Named(NamedKey::F12)))
            }

            _ => None,
        };

        let Some(entry) = inject else {
            return false;
        };

        // Vizia's text-input model is press-driven: TextEvent::InsertText
        // and the textbox's KeyDown handlers run on the press only. Push
        // the queued event on key-down; on key-up, just claim the event
        // so the host doesn't pick the release up as a separate
        // accelerator (BillyDM's reasoning on nice-plug#9).
        if is_down {
            self.key_inject
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push_back(entry);
        }
        true
    }
}

impl Drop for ViziaEditorHandle {
    fn drop(&mut self) {
        self.vizia_state.open.store(false, Ordering::Release);
    }
}

struct HostCallbackAdapter(Box<dyn nice_plug_core::editor::HostCallbacks>);
impl baseview::host::HostCallbacks for HostCallbackAdapter {
    fn request_resize(&mut self, size: baseview::WindowSize) -> Result<(), baseview::HandlerError> {
        self.0
            .request_resize(size.physical.into(), size.scale_factor)
            .map_err(baseview::HandlerError::from_boxed)
    }
    fn destroyed(&mut self) {
        self.0.destroyed();
    }
}
struct HostMainThreadAdapter(Box<dyn nice_plug_core::editor::HostMainThreadCaller>);
impl baseview::host::HostMainThreadCaller for HostMainThreadAdapter {
    fn call_main_thread(&mut self) {
        self.0.call_main_thread();
    }
}

struct GeometryModel(Arc<ViziaState>);
impl Model for GeometryModel {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|scale: &vizia::WindowScaleChanged, _| self.0.system_scale_factor.store(scale.0));
        event.map(|scale: &vizia::UserScaleChanged, _| {
            self.0
                .scale_factor
                .store(scale.0 / self.0.base_scale_factor())
        });
    }
}
