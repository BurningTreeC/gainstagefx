//! The photographs the panel is built from.
//!
//! The knob is drawn without ever being turned. Rotating a photograph rotates
//! the light baked into it, so the highlight travels round with the control
//! instead of staying where the panel light is -- and on a knurled aluminium
//! knob that is glaring. Its printed indicator was patched out of the
//! photograph instead, and the pointer is drawn on top at whatever angle the
//! value asks for. The body never moves; only the line does, which is what
//! happens when you turn a real one.

use nih_plug_vizia::vizia::prelude::Canvas;
use nih_plug_vizia::vizia::vg;
use std::cell::Cell;

pub const KNOB: &[u8] = include_bytes!("../../assets/knob.png");

/// A lazily uploaded image. The canvas is only reachable from `draw`, so the
/// upload happens on the first frame and the id is kept from then on.
#[derive(Default)]
pub struct Sprite {
    id: Cell<Option<vg::ImageId>>,
}

impl Sprite {
    pub const fn new() -> Self {
        Self { id: Cell::new(None) }
    }

    fn id(&self, canvas: &mut Canvas, bytes: &[u8]) -> Option<vg::ImageId> {
        if let Some(id) = self.id.get() {
            return Some(id);
        }
        match canvas.load_image_mem(bytes, vg::ImageFlags::GENERATE_MIPMAPS) {
            Ok(id) => {
                self.id.set(Some(id));
                Some(id)
            }
            Err(_) => None,
        }
    }

    /// Draws the photograph centred on a point, at a given height, keeping its
    /// proportions. `tint` fades it towards the panel.
    pub fn draw(&self, canvas: &mut Canvas, bytes: &[u8], cx: f32, cy: f32, height: f32, tint: f32) {
        let Some(id) = self.id(canvas, bytes) else {
            return;
        };
        let Ok((iw, ih)) = canvas.image_size(id) else {
            return;
        };
        let scale = height / ih as f32;
        let (w, h) = (iw as f32 * scale, ih as f32 * scale);
        let (x, y) = (cx - w / 2.0, cy - h / 2.0);
        let mut path = vg::Path::new();
        path.rect(x, y, w, h);
        canvas.fill_path(&path, &vg::Paint::image(id, x, y, w, h, 0.0, tint));
    }

}
