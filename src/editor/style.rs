//! Panel colours and geometry.
//!
//! The whole layout is here as numbers rather than spread through the panel
//! code, because the one thing this panel has to get right is that it reads in
//! the order the signal travels, and that is a property of where things are.

use nih_plug_vizia::vizia::vg;

pub const PANEL_W: f32 = 640.0;
pub const HEADER_H: f32 = 32.0;

/// The six sections, in signal order, with the height each needs.
///
/// Every one is full width and they stack downwards, so the panel is read the
/// way the signal goes through it and there is no second column to wonder
/// about. The number and name sit in a gutter down the left, which is what
/// makes the order legible at a glance rather than only after reading the
/// labels.
/// Sized to the controls in them and nothing else. An earlier set of these
/// was laid out around three and four line explanations sitting beside every
/// section, which made the window 720 by 734 -- a lot of screen for a plugin
/// with eleven controls on it, and permanently so, since panel text cannot be
/// dismissed once it has been read. The explanations are in the README, where
/// they can be read once.
pub const SECTIONS: [(&str, &str, f32); 6] = [
    ("1", "INPUT", 74.0),
    ("2", "CIRCUIT", 148.0),
    ("3", "DRIVE", 76.0),
    ("4", "TONE", 104.0),
    ("5", "CABINET", 44.0),
    ("6", "OUTPUT", 74.0),
];

/// Width of the numbered gutter down the left.
pub const GUTTER_W: f32 = 78.0;

pub fn section_top(index: usize) -> f32 {
    HEADER_H + SECTIONS.iter().take(index).map(|s| s.2).sum::<f32>()
}

pub const PANEL_H: f32 = {
    let mut total = 0.0;
    let mut i = 0;
    while i < SECTIONS.len() {
        total += SECTIONS[i].2;
        i += 1;
    }
    total
};
pub const WINDOW_H: f32 = PANEL_H + HEADER_H;

/// A knob sweeps this many degrees, zero at the lower left.
pub const SWEEP: f32 = 280.0;

pub const PANEL_TOP: u32 = 0x2f3336;
pub const PANEL_BOTTOM: u32 = 0x16191b;
/// The warm light a working control is lit by.
pub const GLOW: u32 = 0xff8a3c;

pub fn rgb(hex: u32) -> vg::Color {
    vg::Color::rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

pub fn rgba(hex: u32, alpha: f32) -> vg::Color {
    let mut c = rgb(hex);
    c.set_alphaf(alpha);
    c
}

pub fn knob_angle(normalized: f32) -> f32 {
    (normalized - 0.5) * SWEEP
}

pub fn polar(cx: f32, cy: f32, radius: f32, degrees: f32) -> (f32, f32) {
    let a = degrees.to_radians();
    (cx + radius * a.sin(), cy - radius * a.cos())
}
