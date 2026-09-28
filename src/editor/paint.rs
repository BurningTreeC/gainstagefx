//! Small Skia drawing helpers for the panel's shared paths and lighting.
use vizia_plug::vizia::vg as sk;
pub type Color = sk::Color4f;
pub use sk::paint::Cap as LineCap;

pub struct Path(sk::PathBuilder);
impl Path {
    pub fn new() -> Self {
        Self(sk::PathBuilder::new())
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.0.add_rect(sk::Rect::from_xywh(x, y, w, h), None, None);
    }
    pub fn rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.0.add_rrect(
            sk::RRect::new_rect_xy(sk::Rect::from_xywh(x, y, w, h), r, r),
            None,
            None,
        );
    }
    pub fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32) {
        self.0.add_oval(
            sk::Rect::from_xywh(x - rx, y - ry, 2.0 * rx, 2.0 * ry),
            None,
            None,
        );
    }
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x, y));
    }
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x, y));
    }
}

pub struct Paint(sk::Paint);
impl Paint {
    pub fn color(color: Color) -> Self {
        let mut paint = sk::Paint::new(color, None);
        paint.set_anti_alias(true);
        Self(paint)
    }
    pub fn with_line_width(mut self, width: f32) -> Self {
        self.0.set_stroke_width(width);
        self
    }
    pub fn with_line_cap(mut self, cap: LineCap) -> Self {
        self.0.set_stroke_cap(cap);
        self
    }
    pub fn linear_gradient(x0: f32, y0: f32, x1: f32, y1: f32, c0: Color, c1: Color) -> Self {
        let mut paint = Self::color(Color::new(1.0, 1.0, 1.0, 1.0));
        let colors = [c0, c1];
        paint.0.set_shader(sk::gradient::shaders::linear_gradient(
            ((x0, y0), (x1, y1)),
            &sk::gradient::Gradient::new(
                sk::gradient::Colors::new_evenly_spaced(&colors, sk::TileMode::Clamp, None),
                sk::gradient::Interpolation::default(),
            ),
            None,
        ));
        paint
    }
    pub fn radial_gradient(x: f32, y: f32, inner: f32, outer: f32, c0: Color, c1: Color) -> Self {
        let mut paint = Self::color(Color::new(1.0, 1.0, 1.0, 1.0));
        let colors = [c0, c0, c1];
        let stops = [0.0, (inner / outer).clamp(0.0, 1.0), 1.0];
        paint.0.set_shader(sk::gradient::shaders::radial_gradient(
            ((x, y), outer),
            &sk::gradient::Gradient::new(
                sk::gradient::Colors::new(&colors, Some(&stops), sk::TileMode::Clamp, None),
                sk::gradient::Interpolation::default(),
            ),
            None,
        ));
        paint
    }
}

pub trait PanelCanvas {
    fn fill_path(&self, path: &Path, paint: &Paint);
    fn stroke_path(&self, path: &Path, paint: &Paint);
}
impl PanelCanvas for sk::Canvas {
    fn fill_path(&self, path: &Path, paint: &Paint) {
        self.draw_path(&path.0.snapshot(), &paint.0);
    }
    fn stroke_path(&self, path: &Path, paint: &Paint) {
        let mut stroke = paint.0.clone();
        stroke.set_style(sk::paint::Style::Stroke);
        self.draw_path(&path.0.snapshot(), &stroke);
    }
}
