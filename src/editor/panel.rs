//! The box everything is bolted to, and the bands the sections sit in.

use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;

use super::style::*;

/// The panel itself: a dark steel face with the six sections banded across it.
///
/// The bands are drawn rather than merely implied by spacing, and they
/// alternate very slightly in shade. That is the whole structural idea: a
/// person opening this for the first time can see there are six things in a
/// row before reading a single word, and the numbers in the gutter say which
/// order they happen in.
///
/// The numbering itself is laid on as labels rather than drawn here -- text on
/// a canvas needs a font registered on the paint, and a paint without one
/// draws nothing and says nothing about it.
pub struct Faceplate;

impl Faceplate {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Pixels(PANEL_W))
            .height(Pixels(WINDOW_H))
            .hoverable(false)
    }
}

impl View for Faceplate {
    fn element(&self) -> Option<&'static str> {
        Some("gainstage-faceplate")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();

        let mut face = vg::Path::new();
        face.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(
            &face,
            &vg::Paint::linear_gradient(
                b.x,
                b.y,
                b.x,
                b.y + b.h,
                rgb(PANEL_TOP),
                rgb(PANEL_BOTTOM),
            ),
        );

        // Brushed grain: fine horizontal lines, which is the direction a panel
        // is actually linished in.
        for i in 0..(b.h as usize / 3) {
            let y = b.y + i as f32 * 3.0 * scale;
            let mut line = vg::Path::new();
            line.move_to(b.x, y);
            line.line_to(b.x + b.w, y);
            canvas.stroke_path(
                &line,
                &vg::Paint::color(rgba(0xffffff, 0.012)).with_line_width(scale),
            );
        }

        // The session strip, set apart because it is about the plugin rather
        // than about the sound.
        let mut strip = vg::Path::new();
        strip.rect(b.x, b.y, b.w, HEADER_H * scale);
        canvas.fill_path(&strip, &vg::Paint::color(rgba(0x000000, 0.28)));

        for (index, (_, _, height)) in SECTIONS.iter().enumerate() {
            let top = b.y + section_top(index) * scale;
            let h = height * scale;

            // Alternating shade, very slight: enough to see the banding
            // without turning the panel into stripes.
            if index % 2 == 1 {
                let mut band = vg::Path::new();
                band.rect(b.x, top, b.w, h);
                canvas.fill_path(&band, &vg::Paint::color(rgba(0xffffff, 0.018)));
            }

            // A hairline between sections, and the gutter the numbering sits
            // in marked off from the controls.
            let mut rule = vg::Path::new();
            rule.move_to(b.x, top);
            rule.line_to(b.x + b.w, top);
            canvas.stroke_path(
                &rule,
                &vg::Paint::color(rgba(0x000000, 0.35)).with_line_width(scale),
            );
            let mut lit = vg::Path::new();
            lit.move_to(b.x, top + scale);
            lit.line_to(b.x + b.w, top + scale);
            canvas.stroke_path(
                &lit,
                &vg::Paint::color(rgba(0xffffff, 0.05)).with_line_width(scale),
            );

            // The arrow down the gutter, which says the signal goes this way.
            if index + 1 < SECTIONS.len() {
                let x = b.x + 27.0 * scale;
                let y = top + h - 10.0 * scale;
                let mut arrow = vg::Path::new();
                arrow.move_to(x, y - 6.0 * scale);
                arrow.line_to(x, y + 2.0 * scale);
                arrow.move_to(x - 3.0 * scale, y - 1.0 * scale);
                arrow.line_to(x, y + 2.5 * scale);
                arrow.line_to(x + 3.0 * scale, y - 1.0 * scale);
                canvas.stroke_path(
                    &arrow,
                    &vg::Paint::color(rgba(0xffffff, 0.10)).with_line_width(1.5 * scale),
                );
            }
        }

        let mut divider = vg::Path::new();
        divider.move_to(b.x + GUTTER_W * scale, b.y + HEADER_H * scale);
        divider.line_to(b.x + GUTTER_W * scale, b.y + b.h);
        canvas.stroke_path(
            &divider,
            &vg::Paint::color(rgba(0x000000, 0.30)).with_line_width(scale),
        );
    }
}
