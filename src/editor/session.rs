//! The preset strip, the menu behind it, and the two dialogs.
//!
//! The menu scrolls, and it has to. A shipped catalogue is a known size and
//! could be laid out to fit; saved presets are not, and a list that runs off
//! the bottom of the window puts everything past the fold permanently out of
//! reach. So the list is a scrolling view from the start rather than a grid
//! that fits today and stops fitting the first time somebody saves twenty
//! sounds of their own.
//!
//! Saved presets sit in their own section at the foot of the list rather than
//! mixed into the shipped groups. Which of them you can delete and which you
//! cannot is worth being able to see without clicking.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::{assets, widgets::RawParamEvent};
use std::collections::BTreeMap;
use std::sync::Arc;

use super::style::*;
use crate::params::GainStageParams;
use crate::presets::{self, Stored, GROUPS, SAVED};

/// How tall the menu is allowed to get before it scrolls. Short enough to sit
/// inside the window at the size the panel opens at.
const MENU_H: f32 = 330.0;
const MENU_W: f32 = 260.0;
const ROW_H: f32 = 22.0;
const HEADING_H: f32 = 24.0;

/// Where the preset button sits in the strip, which is also where the menu
/// hangs from.
pub const BUTTON_X: f32 = 212.0;
pub const BUTTON_W: f32 = 160.0;

// `Data` is how vizia decides whether a bound value has actually changed.
// These are local types, so the impls belong here rather than putting a user
// interface dependency into the preset store.
impl Data for Stored {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// Which question, if any, is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialog {
    None,
    /// Naming a preset before it is written.
    Save,
    /// Confirming that a save will replace one of yours. Shipped presets are
    /// deliberately not asked about: saving under one of their names writes a
    /// new file beside it and replaces nothing.
    Overwrite,
    Delete,
}

impl Data for Dialog {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Lens)]
pub struct Session {
    pub open: bool,
    pub dialog: Dialog,
    /// The name shown in the strip.
    pub current: String,
    /// Whether the panel has been moved since that preset was loaded.
    pub dirty: bool,
    /// Whether the current preset is one of yours, and so can be deleted.
    pub deletable: bool,
    /// What is being typed into the save dialog.
    pub draft: String,
    pub error: String,
    pub entries: Vec<Stored>,
    params: Arc<GainStageParams>,
    /// The values the current preset was loaded with, which is what `dirty` is
    /// measured against.
    reference: BTreeMap<String, f32>,
}

pub enum SessionEvent {
    Toggle,
    Close,
    Load(usize),
    /// Step through the whole list, which is what the wheel over the name
    /// does: the quickest way to hear what is in it.
    Step(i32),
    OpenSave,
    OpenDelete,
    Draft(String),
    Confirm,
    Cancel,
}

impl Session {
    pub fn build_into(cx: &mut Context, params: Arc<GainStageParams>) {
        let current = params
            .preset_name
            .lock()
            .map(|n| n.clone())
            .unwrap_or_else(|_| String::from("Init"));
        let entries = presets::load_all(&*params);
        let reference = entries
            .iter()
            .find(|p| p.name == current)
            .map(|p| p.values.clone())
            .unwrap_or_default();
        let deletable = entries
            .iter()
            .any(|p| !p.built_in && p.name == current);
        Self {
            open: false,
            dialog: Dialog::None,
            current,
            dirty: false,
            deletable,
            draft: String::new(),
            error: String::new(),
            entries,
            params,
            reference,
        }
        .build(cx);
    }

    /// Rebuilt rather than patched, so a preset saved a moment ago is in the
    /// list and one deleted by hand outside the plugin is not.
    fn refresh(&mut self) {
        self.entries = presets::load_all(&*self.params);
        self.deletable = self
            .entries
            .iter()
            .any(|p| !p.built_in && p.name == self.current);
    }

    /// Push a preset out as ordinary parameter gestures, so the change is
    /// automatable and undoable like any other edit rather than a set of
    /// assignments the host never hears about.
    fn apply(&mut self, cx: &mut EventContext, index: usize) {
        let Some(preset) = self.entries.get(index).cloned() else {
            return;
        };
        for (id, ptr, _) in self.params.param_map() {
            let Some(&normalised) = preset.values.get(&id) else {
                continue;
            };
            cx.emit(RawParamEvent::BeginSetParameter(ptr));
            cx.emit(RawParamEvent::SetParameterNormalized(ptr, normalised));
            cx.emit(RawParamEvent::EndSetParameter(ptr));
        }
        self.current = preset.name.clone();
        self.reference = preset.values.clone();
        self.dirty = false;
        self.deletable = !preset.built_in;
        if let Ok(mut name) = self.params.preset_name.lock() {
            *name = preset.name;
        }
    }

    fn store(&mut self) {
        let name = self.draft.trim().to_string();
        let preset = presets::capture(&*self.params, &name);
        match presets::save(&preset) {
            Ok(_) => {
                self.current = name.clone();
                self.reference = preset.values.clone();
                self.dirty = false;
                self.dialog = Dialog::None;
                self.error.clear();
                if let Ok(mut stored) = self.params.preset_name.lock() {
                    *stored = name;
                }
                self.refresh();
            }
            Err(err) => {
                // Kept on screen with the reason, rather than closed as though
                // it had worked.
                self.dialog = Dialog::Save;
                self.error = err.to_string();
            }
        }
    }
}

impl Model for Session {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // Any parameter movement can make the panel differ from the preset it
        // was loaded from, and moving a control back should make it match
        // again -- so this is compared rather than tracked with a flag.
        event.map(|_: &RawParamEvent, _| {
            self.dirty = !presets::matches(&*self.params, &self.reference);
        });

        event.map(|e: &SessionEvent, meta| {
            match e {
                SessionEvent::Toggle => {
                    if self.open {
                        self.open = false;
                    } else {
                        self.refresh();
                        self.open = true;
                    }
                }
                SessionEvent::Close => self.open = false,
                SessionEvent::Load(index) => {
                    self.open = false;
                    self.apply(cx, *index);
                }
                SessionEvent::Step(delta) => {
                    let at = self
                        .entries
                        .iter()
                        .position(|p| p.name == self.current)
                        .unwrap_or(0) as i32;
                    let last = self.entries.len().saturating_sub(1) as i32;
                    self.apply(cx, (at + delta).clamp(0, last) as usize);
                }
                SessionEvent::OpenSave => {
                    self.open = false;
                    self.refresh();
                    // Offered under the name it already has, which is what
                    // somebody tweaking a sound and saving it expects.
                    self.draft = self.current.clone();
                    self.error.clear();
                    self.dialog = Dialog::Save;
                }
                SessionEvent::OpenDelete => {
                    self.open = false;
                    self.error.clear();
                    if self.deletable {
                        self.dialog = Dialog::Delete;
                    }
                }
                SessionEvent::Draft(text) => self.draft = text.clone(),
                SessionEvent::Cancel => {
                    self.dialog = Dialog::None;
                    self.error.clear();
                }
                SessionEvent::Confirm => match self.dialog {
                    Dialog::Save => {
                        if self.draft.trim().is_empty() {
                            self.error = String::from("a preset needs a name");
                        } else if presets::name_taken(&self.draft, &self.entries) {
                            self.dialog = Dialog::Overwrite;
                        } else {
                            self.store();
                        }
                    }
                    Dialog::Overwrite => self.store(),
                    Dialog::Delete => {
                        match presets::delete(&self.current) {
                            Ok(()) => {
                                self.dialog = Dialog::None;
                                self.error.clear();
                                self.refresh();
                            }
                            Err(err) => self.error = err.to_string(),
                        }
                    }
                    Dialog::None => {}
                },
            }
            meta.consume();
        });
    }
}

// ---------------------------------------------------------------------------
// The strip
// ---------------------------------------------------------------------------

/// The name in the strip, with a caret and a dot when it has been edited.
pub struct PresetButton;

impl PresetButton {
    pub fn build_into(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |cx| {
            // A dot rather than an asterisk: it reads as a state, not as a
            // footnote pointing at something.
            Label::new(
                cx,
                Session::root.map(|s: &Session| {
                    if s.dirty {
                        format!("{}  \u{2022}", s.current)
                    } else {
                        s.current.clone()
                    }
                }),
            )
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
        frame(canvas, b, scale, false);

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

/// A small labelled button that emits one event.
pub struct Press {
    make: Box<dyn Fn() -> SessionEvent>,
    /// A greyed button still draws, so the strip does not change shape when
    /// there is nothing to delete.
    enabled: bool,
    strong: bool,
}

impl Press {
    pub fn build_into<'a>(
        cx: &'a mut Context,
        text: &'static str,
        enabled: bool,
        strong: bool,
        make: impl Fn() -> SessionEvent + 'static,
    ) -> Handle<'a, Self> {
        Self {
            make: Box::new(make),
            enabled,
            strong,
        }
        .build(cx, move |cx| {
            Label::new(cx, text)
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_size(10.5)
                .color(if !enabled {
                    Color::rgba(0xff, 0xff, 0xff, 0x33)
                } else if strong {
                    Color::rgb(0xff, 0xb2, 0x6a)
                } else {
                    Color::rgb(0xc9, 0xd2, 0xd8)
                })
                .hoverable(false);
        })
    }
}

impl View for Press {
    fn element(&self) -> Option<&'static str> {
        Some("press")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        if !self.enabled {
            return;
        }
        event.map(|window: &WindowEvent, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window {
                cx.emit((self.make)());
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        frame(canvas, cx.bounds(), cx.scale_factor(), self.strong && self.enabled);
    }
}

/// The box a button or field is drawn in.
fn frame(canvas: &mut Canvas, b: BoundingBox, scale: f32, lit: bool) {
    let mut path = vg::Path::new();
    path.rounded_rect(b.x, b.y, b.w, b.h, 3.0 * scale);
    canvas.fill_path(
        &path,
        &vg::Paint::color(if lit {
            rgba(GLOW, 0.18)
        } else {
            rgba(0x000000, 0.30)
        }),
    );
    canvas.stroke_path(
        &path,
        &vg::Paint::color(rgba(0xffffff, if lit { 0.20 } else { 0.12 })).with_line_width(scale),
    );
}

// ---------------------------------------------------------------------------
// The list
// ---------------------------------------------------------------------------

pub fn menu(cx: &mut Context) {
    Binding::new(cx, Session::open, |cx, open| {
        if !open.get(cx) {
            return;
        }
        Backdrop::new(cx, SessionEvent::Close);

        VStack::new(cx, |cx| {
            ScrollView::new(cx, 0.0, 0.0, false, true, |cx| {
                Binding::new(cx, Session::entries, |cx, entries| {
                    let entries = entries.get(cx);
                    VStack::new(cx, move |cx| {
                        for group in GROUPS.iter().copied().chain([SAVED]) {
                            let rows: Vec<(usize, String)> = entries
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| p.group == group)
                                .map(|(i, p)| (i, p.name.clone()))
                                .collect();
                            // A heading with nothing under it is a promise the
                            // list does not keep, so "Saved" only appears once
                            // something has been saved.
                            if rows.is_empty() {
                                continue;
                            }
                            heading(cx, group);
                            for (index, name) in rows {
                                Row::build_into(cx, index, name);
                            }
                        }
                    })
                    .width(Stretch(1.0))
                    .height(Auto);
                });
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0));
        })
        // Hung under the button it belongs to, not merely near it.
        .position_type(PositionType::SelfDirected)
        .left(Pixels(BUTTON_X))
        .top(Pixels(HEADER_H - 2.0))
        .width(Pixels(MENU_W))
        .height(Pixels(MENU_H))
        .background_color(Color::rgb(0x1c, 0x20, 0x23))
        .border_color(Color::rgba(0xff, 0xff, 0xff, 0x22))
        .border_width(Pixels(1.0));
    });
}

fn heading(cx: &mut Context, text: &'static str) {
    Label::new(cx, text)
        .width(Stretch(1.0))
        .height(Pixels(HEADING_H))
        .child_left(Pixels(10.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_size(9.5)
        .color(Color::rgb(0x7e, 0x8a, 0x96))
        .hoverable(false);
}

/// One name in the list.
pub struct Row {
    index: usize,
}

impl Row {
    pub fn build_into(cx: &mut Context, index: usize, name: String) -> Handle<'_, Self> {
        let shown = name.clone();
        Self { index }
            .build(cx, move |cx| {
                Label::new(cx, &shown)
                    .width(Stretch(1.0))
                    .height(Stretch(1.0))
                    .child_left(Pixels(20.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(11.0)
                    .color(Session::current.map(move |current| {
                        if *current == name {
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
        let b = cx.bounds();
        let mut row = vg::Path::new();
        row.rect(b.x, b.y, b.w, b.h);
        canvas.fill_path(&row, &vg::Paint::color(rgba(0xffffff, 0.02)));
    }
}

// ---------------------------------------------------------------------------
// The dialogs
// ---------------------------------------------------------------------------

const DIALOG_W: f32 = 320.0;
const DIALOG_H: f32 = 150.0;

pub fn dialogs(cx: &mut Context) {
    Binding::new(cx, Session::dialog, |cx, which| {
        let which = which.get(cx);
        if which == Dialog::None {
            return;
        }
        // A question has to sit on top of whatever asked it, and clicking away
        // from it means no.
        Backdrop::new(cx, SessionEvent::Cancel);

        let left = (PANEL_W - DIALOG_W) / 2.0;
        let top = (WINDOW_H - DIALOG_H) / 2.0;

        VStack::new(cx, move |cx| {
            let title = match which {
                Dialog::Save => "Save preset",
                Dialog::Overwrite => "Replace it?",
                Dialog::Delete => "Delete preset",
                Dialog::None => "",
            };
            Label::new(cx, title)
                .width(Stretch(1.0))
                .height(Pixels(28.0))
                .child_left(Pixels(16.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0))
                .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                .font_size(11.5)
                .color(Color::rgb(0xe8, 0xee, 0xf4))
                .hoverable(false);

            match which {
                Dialog::Save => {
                    Textbox::new(cx, Session::draft)
                        .width(Stretch(1.0))
                        .height(Pixels(26.0))
                        .left(Pixels(16.0))
                        .right(Pixels(16.0))
                        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                        .font_size(11.0)
                        .color(Color::rgb(0xe8, 0xee, 0xf4))
                        .background_color(Color::rgba(0x00, 0x00, 0x00, 0x55))
                        .border_color(Color::rgba(0xff, 0xff, 0xff, 0x22))
                        .border_width(Pixels(1.0))
                        // Both, because a name typed and then clicked away
                        // from has still been typed. Committing only on Enter
                        // is how a save quietly writes the previous name.
                        .on_edit(|cx, text| cx.emit(SessionEvent::Draft(text)))
                        .on_submit(|cx, text, _| {
                            cx.emit(SessionEvent::Draft(text));
                            cx.emit(SessionEvent::Confirm);
                        });
                }
                Dialog::Overwrite => {
                    note(
                        cx,
                        Session::draft
                            .map(|n| format!("You already have a preset called {n}.")),
                    );
                }
                Dialog::Delete => {
                    note(
                        cx,
                        Session::current.map(|n| format!("Delete {n}? This cannot be undone.")),
                    );
                }
                Dialog::None => {}
            }

            Binding::new(cx, Session::error, |cx, error| {
                let error = error.get(cx);
                if error.is_empty() {
                    return;
                }
                Label::new(cx, &error)
                    .width(Stretch(1.0))
                    .height(Pixels(18.0))
                    .left(Pixels(16.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Stretch(1.0))
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_size(10.0)
                    .color(Color::rgb(0xe8, 0x7a, 0x5a))
                    .hoverable(false);
            });

            HStack::new(cx, move |cx| {
                Press::build_into(cx, "Cancel", true, false, || SessionEvent::Cancel)
                    .width(Pixels(84.0))
                    .height(Pixels(24.0));
                let go = match which {
                    Dialog::Delete => "Delete",
                    Dialog::Overwrite => "Replace",
                    _ => "Save",
                };
                Press::build_into(cx, go, true, true, || SessionEvent::Confirm)
                    .width(Pixels(84.0))
                    .height(Pixels(24.0))
                    .left(Pixels(10.0));
            })
            .width(Stretch(1.0))
            .height(Pixels(24.0))
            .top(Stretch(1.0))
            .left(Stretch(1.0))
            .right(Pixels(16.0))
            .bottom(Pixels(14.0));
        })
        .position_type(PositionType::SelfDirected)
        .left(Pixels(left))
        .top(Pixels(top))
        .width(Pixels(DIALOG_W))
        .height(Pixels(DIALOG_H))
        .background_color(Color::rgb(0x22, 0x27, 0x2a))
        .border_color(Color::rgba(0xff, 0xff, 0xff, 0x2a))
        .border_width(Pixels(1.0));
    });
}

fn note(cx: &mut Context, text: impl Lens<Target = String>) {
    Label::new(cx, text)
        .width(Stretch(1.0))
        .height(Pixels(30.0))
        .left(Pixels(16.0))
        .right(Pixels(16.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_size(10.5)
        .color(Color::rgb(0xa8, 0xb2, 0xba))
        .hoverable(false);
}

/// Catches a click anywhere outside whatever is on top.
pub struct Backdrop {
    on_click: Box<dyn Fn() -> SessionEvent>,
}

impl Backdrop {
    pub fn new(cx: &mut Context, close: SessionEvent) -> Handle<'_, Self> {
        // Captured as a maker rather than a value, because the view outlives
        // the one event it was built with.
        let which = match close {
            SessionEvent::Cancel => 1,
            _ => 0,
        };
        Self {
            on_click: Box::new(move || {
                if which == 1 {
                    SessionEvent::Cancel
                } else {
                    SessionEvent::Close
                }
            }),
        }
        .build(cx, |_| {})
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
                cx.emit((self.on_click)());
                meta.consume();
            }
        });
    }
}
