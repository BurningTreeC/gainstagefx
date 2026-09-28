use std::ffi::c_void;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_audio_toolbox::AudioUnit;
use objc2_foundation::{NSPoint, NSRect, NSSize};

use super::editor::EditorView;
use crate::bridge::nice_au2_get_editor_size;

const NICE_AU2_PROPERTY_RUST_INSTANCE: u32 = 0x4E41_7269;
const AUDIO_UNIT_SCOPE_GLOBAL: u32 = 0;

unsafe extern "C" {
    fn AudioUnitGetProperty(
        audio_unit: AudioUnit,
        property: u32,
        scope: u32,
        element: u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> i32;
}

pub(super) fn create_view(audio_unit: AudioUnit) -> *mut NSView {
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    let Some(view) = make_view(mtm, audio_unit) else {
        return std::ptr::null_mut();
    };
    Retained::autorelease_ptr(view)
}

fn make_view(mtm: MainThreadMarker, audio_unit: AudioUnit) -> Option<Retained<NSView>> {
    let mut instance: *mut c_void = std::ptr::null_mut();
    let mut size = std::mem::size_of_val(&instance) as u32;
    if unsafe {
        AudioUnitGetProperty(
            audio_unit,
            NICE_AU2_PROPERTY_RUST_INSTANCE,
            AUDIO_UNIT_SCOPE_GLOBAL,
            0,
            (&mut instance as *mut *mut c_void).cast(),
            &mut size,
        )
    } != 0
        || instance.is_null()
    {
        return None;
    }
    let (mut width, mut height) = (0, 0);
    if !nice_au2_get_editor_size(instance.cast(), &mut width, &mut height) {
        return None;
    }
    let frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(width as f64, height as f64),
    );
    Some(EditorView::new(mtm, frame, instance).into_super())
}
