use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSColor, NSRectFill, NSView};
use objc2_foundation::{NSRect, NSSize};

use crate::bridge::{
    nice_au2_destroy_editor, nice_au2_flush_editor_notifications, nice_au2_get_editor_size,
    nice_au2_spawn_editor,
};

type CVDisplayLinkRef = *mut c_void;
type CVReturn = i32;
type CVOptionFlags = u64;

type CVDisplayLinkOutputCallback = unsafe extern "C" fn(
    CVDisplayLinkRef,
    *const c_void,
    *const c_void,
    CVOptionFlags,
    *mut CVOptionFlags,
    *mut c_void,
) -> CVReturn;

unsafe extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(link: *mut CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkSetOutputCallback(
        link: CVDisplayLinkRef,
        callback: Option<CVDisplayLinkOutputCallback>,
        context: *mut c_void,
    ) -> CVReturn;
    fn CVDisplayLinkStart(link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkStop(link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkRelease(link: CVDisplayLinkRef);
}

#[derive(Default)]
pub(super) struct EditorIvars {
    rust_instance: Cell<*mut c_void>,
    editor_handle: Cell<*mut c_void>,
    display_link: Cell<CVDisplayLinkRef>,
    display_link_context: Cell<*mut DisplayLinkContext>,
}

struct DisplayLinkContext {
    view: *mut EditorView,
    refresh_queued: AtomicBool,
}

define_class!(
    #[unsafe(super(NSView))]
    #[name = "NiceAu2EditorView"]
    #[thread_kind = MainThreadOnly]
    #[ivars = EditorIvars]
    pub(super) struct EditorView;

    impl EditorView {
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> Bool {
            Bool::YES
        }

        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> Bool {
            Bool::YES
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty_rect: NSRect) {
            NSColor::blackColor().setFill();
            NSRectFill(dirty_rect);
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move_to_window(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            if self.window().is_some() {
                self.spawn_editor_if_needed();
            }
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, requested: NSSize) {
            let instance = self.ivars().rust_instance.get();
            let mut width = 0;
            let mut height = 0;
            let size = if !instance.is_null()
                && nice_au2_get_editor_size(instance.cast(), &mut width, &mut height)
                && width > 0
                && height > 0
            {
                NSSize::new(width as f64, height as f64)
            } else {
                requested
            };
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.layout_embedded_subviews();
        }

        #[unsafe(method(niceAu2RefreshEditorView))]
        fn refresh_editor_view(&self) {
            let instance = self.ivars().rust_instance.get();
            if !instance.is_null() {
                nice_au2_flush_editor_notifications(instance.cast());
            }
            invalidate_view_hierarchy(self);
            let _: () = unsafe { msg_send![self, layoutSubtreeIfNeeded] };
            let _: () = unsafe { msg_send![self, displayIfNeeded] };
            let context = self.ivars().display_link_context.get();
            if !context.is_null() {
                unsafe { &*context }
                    .refresh_queued
                    .store(false, Ordering::Release);
            }
        }

        #[unsafe(method(niceAu2CloseEditorForDestroyedAudioUnit))]
        fn close_for_destroyed_audio_unit(&self) {
            unregister(self.ivars().rust_instance.get(), self);
            self.close_editor();
            self.ivars().rust_instance.set(ptr::null_mut());
        }
    }
);
impl EditorView {
    pub(super) fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        rust_instance: *mut c_void,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(EditorIvars {
            rust_instance: Cell::new(rust_instance),
            ..EditorIvars::default()
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setWantsLayer(true);
        this
    }

    fn spawn_editor_if_needed(&self) {
        let ivars = self.ivars();
        let instance = ivars.rust_instance.get();
        if instance.is_null() || !ivars.editor_handle.get().is_null() {
            return;
        }

        if let Some(previous) = register(instance, self) {
            let _: () = unsafe { msg_send![previous, niceAu2CloseEditorForDestroyedAudioUnit] };
        }
        let handle = nice_au2_spawn_editor(instance.cast(), (self as *const Self).cast_mut().cast());
        ivars.editor_handle.set(handle);
        self.layout_embedded_subviews();
        let _: () = unsafe { msg_send![self, niceAu2RefreshEditorView] };
        self.start_display_link();
    }

    fn start_display_link(&self) {
        if !self.ivars().display_link.get().is_null() {
            return;
        }
        let mut link = ptr::null_mut();
        if unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&mut link) } != 0 || link.is_null() {
            return;
        }
        let view = unsafe {
            Retained::into_raw(
                Retained::retain((self as *const Self).cast_mut())
                    .expect("an Objective-C method always has a live self"),
            )
        };
        let context = Box::into_raw(Box::new(DisplayLinkContext {
            view,
            refresh_queued: AtomicBool::new(false),
        }));
        if unsafe {
            CVDisplayLinkSetOutputCallback(link, Some(display_link_callback), context.cast())
        } != 0
        {
            unsafe {
                let context = Box::from_raw(context);
                drop(Retained::<Self>::from_raw(context.view).unwrap());
                CVDisplayLinkRelease(link);
            }
            return;
        }
        unsafe { CVDisplayLinkStart(link) };
        self.ivars().display_link_context.set(context);
        self.ivars().display_link.set(link);
    }

    fn stop_display_link(&self) {
        let link = self.ivars().display_link.replace(ptr::null_mut());
        if link.is_null() {
            return;
        }
        unsafe {
            CVDisplayLinkStop(link);
            CVDisplayLinkSetOutputCallback(link, None, ptr::null_mut());
            CVDisplayLinkRelease(link);
            let context = self.ivars().display_link_context.replace(ptr::null_mut());
            if !context.is_null() {
                let context = Box::from_raw(context);
                drop(Retained::<Self>::from_raw(context.view).unwrap());
            }
        }
    }

    fn close_editor(&self) {
        self.stop_display_link();
        let handle = self.ivars().editor_handle.replace(ptr::null_mut());
        let instance = self.ivars().rust_instance.get();
        if !handle.is_null() && !instance.is_null() {
            nice_au2_destroy_editor(instance.cast(), handle);
        }
        let subviews: Retained<objc2_foundation::NSArray<NSView>> =
            unsafe { msg_send![self, subviews] };
        for subview in subviews.iter() {
            subview.removeFromSuperview();
        }
    }

    fn layout_embedded_subviews(&self) {
        let bounds = self.bounds();
        let subviews: Retained<objc2_foundation::NSArray<NSView>> =
            unsafe { msg_send![self, subviews] };
        for subview in subviews.iter() {
            subview.setFrame(bounds);
            subview.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            subview.setNeedsDisplay(true);
        }
    }
}

impl Drop for EditorView {
    fn drop(&mut self) {
        unregister(self.ivars().rust_instance.get(), self);
        self.close_editor();
    }
}

unsafe extern "C" fn display_link_callback(
    _link: CVDisplayLinkRef,
    _now: *const c_void,
    _output: *const c_void,
    _input_flags: CVOptionFlags,
    _output_flags: *mut CVOptionFlags,
    context: *mut c_void,
) -> CVReturn {
    let context = unsafe { &*context.cast::<DisplayLinkContext>() };
    if !context.refresh_queued.swap(true, Ordering::AcqRel) {
        let _: () = unsafe {
            msg_send![context.view,
                performSelectorOnMainThread: sel!(niceAu2RefreshEditorView),
                withObject: ptr::null::<objc2::runtime::AnyObject>(),
                waitUntilDone: Bool::NO
            ]
        };
    }
    0
}

fn invalidate_view_hierarchy(view: &NSView) {
    view.setNeedsDisplay(true);
    let subviews: Retained<objc2_foundation::NSArray<NSView>> =
        unsafe { msg_send![view, subviews] };
    for subview in subviews.iter() {
        invalidate_view_hierarchy(&subview);
    }
}

fn active_views() -> &'static Mutex<HashMap<usize, usize>> {
    static VIEWS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
    VIEWS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register(instance: *mut c_void, view: &EditorView) -> Option<*mut EditorView> {
    active_views()
        .lock()
        .ok()?
        .insert(instance as usize, view as *const _ as usize)
        .map(|p| p as *mut _)
}

fn unregister(instance: *mut c_void, view: &EditorView) {
    if let Ok(mut views) = active_views().lock()
        && views.get(&(instance as usize)).copied() == Some(view as *const _ as usize)
    {
        views.remove(&(instance as usize));
    }
}

pub(super) fn close_for_rust_instance(instance: *mut c_void) {
    let view = active_views()
        .lock()
        .ok()
        .and_then(|views| views.get(&(instance as usize)).copied());
    if let Some(view) = view {
        let view = view as *mut EditorView;
        let _: () = unsafe {
            msg_send![view,
                performSelectorOnMainThread: sel!(niceAu2CloseEditorForDestroyedAudioUnit),
                withObject: ptr::null::<objc2::runtime::AnyObject>(),
                waitUntilDone: Bool::YES
            ]
        };
    }
}
