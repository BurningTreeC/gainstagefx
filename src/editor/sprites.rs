//! The rendered parts the panel is built from.
//!
//! The knob is rendered from `assets/knob.glb` by `assets/knob.sh`, under the
//! same lighting rig every other rendered control on these panels uses: a key
//! up and to the left of the camera. It used to be a photograph, which brought
//! its own studio with it and agreed with nothing else on the panel about
//! where the light was.
//!
//! It is drawn without ever being turned. Rotating a sprite rotates the light
//! baked into it, so the highlight would travel round with the control instead
//! of staying at the corner the panel is lit from -- and on a knurled
//! aluminium knob that is glaring. The model therefore carries no indicator;
//! the pointer is drawn on top at whatever angle the value asks for. The body
//! never moves, only the line does, which is what happens when you turn a real
//! one.

use std::cell::OnceCell;
use vizia_plug::vizia::vg as sk;
pub const KNOB: &[u8] = include_bytes!("../../assets/knob.png");
#[derive(Default)]
pub struct Sprite {
    image: OnceCell<Option<sk::Image>>,
}
impl Sprite {
    pub const fn new() -> Self {
        Self {
            image: OnceCell::new(),
        }
    }
    pub fn draw(
        &self,
        canvas: &sk::Canvas,
        bytes: &[u8],
        cx: f32,
        cy: f32,
        height: f32,
        tint: f32,
    ) {
        let Some(image) = self
            .image
            .get_or_init(|| sk::Image::from_encoded(sk::Data::new_copy(bytes)))
        else {
            return;
        };
        let width = height * image.width() as f32 / image.height() as f32;
        let mut paint = sk::Paint::default();
        paint.set_alpha_f(tint);
        canvas.draw_image_rect_with_sampling_options(
            image,
            None,
            sk::Rect::from_xywh(cx - width / 2.0, cy - height / 2.0, width, height),
            sk::SamplingOptions::new(sk::FilterMode::Linear, sk::MipmapMode::Linear),
            &paint,
        );
    }
}
