//! The preset strip and the menu behind it.
//!
//! The menu scrolls, and it has to. A shipped catalogue is a known size and
//! could be laid out to fit; user presets are not, and a list that runs off
//! the bottom of the window puts everything past the fold permanently out of
//! reach. So the list is a scrolling view from the start rather than a grid
//! that fits today and stops fitting the first time somebody saves twenty
//! sounds of their own.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, widgets::RawParamEvent};
use std::sync::Arc;

use super::style::*;
use crate::params::GainStageParams;
use crate::presets::{self, GROUPS, PRESETS};

/// How tall the menu is allowed to get before it scrolls. Short enough to sit
/// inside the window at the smallest size the panel opens at.
const MENU_H: f32 = 330.0;
const MENU_W: f32 = 260.0;
/// Where the preset button sits in the strip, which is also where the menu
/// hangs from.
pub const BUTTON_X: f32 = 212.0;
pub const BUTTON_W: f32 = 208.0;
const ROW_H: f32 = 22.0;
const HEADING_H: f32 = 24.0;

#[derive(Lens)]
pub struct Session {
    pub open: bool,
    pub current: String,
    params: Arc<GainStageParams>,
}

pub enum SessionEvent {
    Toggle,
    Close,
    Load(usize),
    /// Step through the whole catalogue, which is what the arrows either side
    /// of the name do and what the wheel over the name does.
    Step(i32),
}

impl Session {
    pub fn build_into(cx: &mut Context, params: Arc<GainStageParams>) {
        let current = params
            .preset_name
            .lock()
            .map(|n| n.clone())
            .unwrap_or_else(|_| String::from("Init"));
        Self {
            open: false,
            current,
            params,
        }
        .build(cx);
    }

    /// Push a preset out as ordinary parameter gestures, so the change is
    /// automatable and undoable like any other edit rather than a set of
    /// assignments the host never hears about.
    fn apply(&mut self, cx: &mut EventContext, index: usize) {
        let Some(preset) = PRESETS.get(index) else {
            return;
        };
        let dials = preset.dials();
        for (id, ptr, _) in self.params.param_map() {
            let Some((_, plain)) = dials.iter().find(|(name, _)| *name == id) else {
                continue;
            };
            // Safety: the pointer comes straight from this parameter set and
            // is used before anything can drop it.
            let normalised = unsafe { ptr.preview_normalized(*plain) };
            cx.emit(RawParamEvent::BeginSetParameter(ptr));
            cx.emit(RawParamEvent::SetParameterNormalized(ptr, normalised));
            cx.emit(RawParamEvent::EndSetParameter(ptr));
        }
        self.current = preset.name.to_string();
        if let Ok(mut name) = self.params.preset_name.lock() {
            *name = preset.name.to_string();
        }
    }
}

impl Model for Session {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|e: &SessionEvent, meta| {
            match e {
                SessionEvent::Toggle => self.open = !self.open,
                SessionEvent::Close => self.open = false,
                SessionEvent::Load(index) => {
                    self.open = false;
                    self.apply(cx, *index);
                }
                SessionEvent::Step(delta) => {
                    let at = presets::index_of(&self.current).unwrap_or(0) as i32;
                    let last = PRESETS.len() as i32 - 1;
                    self.apply(cx, (at + delta).clamp(0, last) as usize);
                }
            }
            meta.consume();
        });
    }
}

/// The name in the strip, with an arrow either side of it.
pub struct PresetButton;

impl PresetButton {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |cx| {
            Label::new(cx, Session::current)
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_left(Pixels(10.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_size(11.0)
                .color(Color::rgb(0xff, 0xb2, 0x6a))
                .hoverable(false);
        })
    }
}

impl View for PresetButton {
    fn element(&self) -> Option<&'static str> {
        Some("preset-button")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| match window {
            WindowEvent::MouseDown(MouseButton::Left) => {
                cx.emit(SessionEvent::Toggle);
                meta.consume();
            }
            // The wheel steps through the catalogue without opening anything,
            // which is the quickest way to hear what is in it.
            WindowEvent::MouseScroll(_, y) => {
                cx.emit(SessionEvent::Step(-y.signum() as i32));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        let mut box_ = vg::Path::new();
        box_.rounded_rect(b.x, b.y, b.w, b.h, 3.0 * scale);
        canvas.fill_path(&box_, &vg::Paint::color(rgba(0x000000, 0.30)));
        canvas.stroke_path(
            &box_,
            &vg::Paint::color(rgba(0xffffff, 0.12)).with_line_width(scale),
        );

        // A caret on the right, so it reads as something that opens.
        let (x, y) = (b.x + b.w - 14.0 * scale, b.y + b.h / 2.0);
        let mut caret = vg::Path::new();
        caret.move_to(x - 4.0 * scale, y - 2.0 * scale);
        caret.line_to(x, y + 2.5 * scale);
        caret.line_to(x + 4.0 * scale, y - 2.0 * scale);
        canvas.stroke_path(
            &caret,
            &vg::Paint::color(rgba(0xc9d2d8, 0.7)).with_line_width(1.4 * scale),
        );
    }
}

/// The list itself: a scrolling view, grouped, with the group headings in it.
pub fn menu(cx: &mut Context) {
    Binding::new(cx, Session::open, |cx, open| {
        if !open.get(cx) {
            return;
        }
        // A sheet behind the menu that swallows a click anywhere else, so the
        // menu closes the way every other menu does.
        Backdrop::new(cx);

        VStack::new(cx, |cx| {
            // Vertical only: the list is a column of names, and a sideways
            // scroll on a column is a way to lose the list.
            ScrollView::new(cx, 0.0, 0.0, false, true, |cx| {
                VStack::new(cx, |cx| {
                    for group in GROUPS {
                        Label::new(cx, group)
                            .width(Stretch(1.0))
                            .height(Pixels(HEADING_H))
                            .child_left(Pixels(10.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0))
                            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                            .font_size(9.5)
                            .color(Color::rgb(0x7e, 0x8a, 0x96))
                            .hoverable(false);

                        for (index, preset) in presets::in_group(group) {
                            Row::new(cx, index, preset.name);
                        }
                    }
                })
                .width(Stretch(1.0))
                .height(Auto);
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        .position_type(PositionType::SelfDirected)
        // Hung under the button it belongs to, not merely near it.
        .left(Pixels(BUTTON_X))
        .top(Pixels(HEADER_H - 2.0))
        .width(Pixels(MENU_W))
        .height(Pixels(MENU_H))
        .background_color(Color::rgb(0x1c, 0x20, 0x23))
        .border_color(Color::rgba(0xff, 0xff, 0xff, 0x22))
        .border_width(Pixels(1.0));
    });
}

/// One name in the list.
pub struct Row {
    index: usize,
}

impl Row {
    pub fn new<'a>(cx: &'a mut Context, index: usize, name: &'static str) -> Handle<'a, Self> {
        Self { index }
            .build(cx, |cx| {
                Label::new(cx, name)
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_left(Pixels(20.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(11.0)
                    .color(Session::current.map(move |current| {
                        if current == name {
                            Color::rgb(0xff, 0xb2, 0x6a)
                        } else {
                            Color::rgb(0xc9, 0xd2, 0xd8)
                        }
                    }))
                    .hoverable(false);
            })
            .width(Stretch(1.0))
            .height(Pixels(ROW_H))
    }
}

impl View for Row {
    fn element(&self) -> Option<&'static str> {
        Some("preset-row")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        let index = self.index;
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window {
                cx.emit(SessionEvent::Load(index));
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        // The current preset already reads warm; this is only the faint band
        // that says a row is a thing you can click.
        let b = cx.bounds();
        let mut row = vg::Path::new();
        row.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(&row, &vg::Paint::color(rgba(0xffffff, 0.02)));
    }
}

/// Catches a click anywhere outside the menu.
pub struct Backdrop;

impl Backdrop {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |_| {})
            .position_type(PositionType::SelfDirected)
            .left(Pixels(0.0))
            .top(Pixels(0.0))
            .width(Pixels(PANEL_W))
            .height(Pixels(WINDOW_H))
    }
}

impl View for Backdrop {
    fn element(&self) -> Option<&'static str> {
        Some("backdrop")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(_) = window {
                cx.emit(SessionEvent::Close);
                meta.consume();
            }
        });
    }
}

use nih_plug_vizia::vizia::vg;
