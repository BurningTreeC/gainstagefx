//! The panel's controls.

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::{util::ModifiersExt, RawParamEvent};

use super::sprites::{self, Sprite};
use super::style::*;

const DRAG_RANGE: f32 = 260.0;
const FINE: f32 = 0.15;

/// A knob: the photograph, still, with its pointer drawn on.
pub struct Knob {
    param: ParamWidgetBase,
    radius: f32,
    face: Sprite,
    dragging: bool,
    last_y: f32,
}

impl Knob {
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
        radius: f32,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        Self {
            param: ParamWidgetBase::new(cx, params, params_to_param),
            radius,
            face: Sprite::new(),
            dragging: false,
            last_y: 0.0,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, data| {
                let value = data.make_lens(|param| param.modulated_normalized_value());
                Binding::new(cx, value, |cx, _| cx.needs_redraw());
            }),
        )
        .width(Pixels(radius * 2.0))
        .height(Pixels(radius * 2.0))
    }

    fn nudge(&self, cx: &mut EventContext, delta: f32) {
        let current = self.param.unmodulated_normalized_value();
        self.param
            .set_normalized_value(cx, (current + delta).clamp(0.0, 1.0));
    }

    /// Ends a drag: releases the mouse and closes the gesture with the host.
    ///
    /// Called from more than one place because the one that must not be relied
    /// on is the mouse button coming back up. See `event`.
    fn finish(&mut self, cx: &mut EventContext) {
        if !self.dragging {
            return;
        }
        self.dragging = false;
        cx.release();
        cx.set_active(false);
        self.param.end_set_parameter(cx);
    }
}

impl View for Knob {
    fn element(&self) -> Option<&'static str> {
        Some("frontend-knob")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        let r = self.radius * scale;
        let (mx, my) = (b.x + b.w / 2.0, b.y + b.h / 2.0);

        // The shadow it casts on the panel.
        let mut shadow = vg::Path::new();
        shadow.ellipse(mx, my + r * 0.16, r * 1.16, r * 1.10);
        canvas.fill_path(
            &shadow,
            &vg::Paint::radial_gradient(
                mx,
                my + r * 0.16,
                r * 0.74,
                r * 1.16,
                rgba(0x000000, 0.55),
                rgba(0x000000, 0.0),
            ),
        );

        self.face.draw(canvas, sprites::KNOB, mx, my, r * 2.0, 1.0);

        // The pointer, and only the pointer, turns.
        let angle = knob_angle(self.param.modulated_normalized_value());
        let (x0, y0) = polar(mx, my, r * 0.26, angle);
        let (x1, y1) = polar(mx, my, r * 0.70, angle);
        let mut pointer = vg::Path::new();
        pointer.move_to(x0, y0);
        pointer.line_to(x1, y1);
        // Cut into the metal: a dark groove with a lit lower lip.
        canvas.stroke_path(
            &pointer,
            &vg::Paint::color(rgba(0x07090a, 0.92))
                .with_line_width(r * 0.085)
                .with_line_cap(vg::LineCap::Round),
        );
        let (lx0, ly0) = polar(mx, my + r * 0.022, r * 0.26, angle);
        let (lx1, ly1) = polar(mx, my + r * 0.022, r * 0.70, angle);
        let mut lip = vg::Path::new();
        lip.move_to(lx0, ly0);
        lip.line_to(lx1, ly1);
        canvas.stroke_path(
            &lip,
            &vg::Paint::color(rgba(0xffffff, 0.20))
                .with_line_width(r * 0.028)
                .with_line_cap(vg::LineCap::Round),
        );
    }

    /// Mouse handling, and the one thing in it that is not obvious.
    ///
    /// A drag captures the mouse so that the knob keeps receiving movement
    /// when the pointer leaves it, and releases on the button coming back up.
    /// That release must not be the *only* way out.
    ///
    /// vizia routes every mouse event to the captured entity, and nothing in
    /// vizia ever clears a capture on its own -- `MouseCaptureOutEvent` is
    /// declared in its event enum and emitted nowhere, and `release` only
    /// clears the field when the widget itself asks. So a drag whose button-up
    /// never arrives leaves this knob holding the mouse for the rest of the
    /// session: every other control stops responding, the window looks frozen,
    /// and the audio thread carries on as though nothing were wrong. The
    /// gesture opened with the host is never closed either, so it also thinks
    /// an edit is still in progress.
    ///
    /// A button-up can genuinely go missing. On Windows the pointer is held
    /// with `SetCapture`, and a `WM_CAPTURECHANGED` -- another window taking
    /// capture, the host putting up a dialog, the plugin window being
    /// deactivated mid-drag -- sends the button-up somewhere else entirely.
    ///
    /// So the drag is also ended by anything that says the mouse is no longer
    /// down, and the check that does not depend on an event arriving at all is
    /// in `MouseMove`: if the button is up while this knob thinks it is
    /// dragging, the drag is over whether or not anyone said so. That one
    /// heals the window the moment the pointer moves over it again.
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|param_event, _| {
            if let RawParamEvent::ParametersChanged = param_event {
                cx.needs_redraw();
            }
        });
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().command() {
                    self.param.begin_set_parameter(cx);
                    self.param
                        .set_normalized_value(cx, self.param.default_normalized_value());
                    self.param.end_set_parameter(cx);
                } else if !self.dragging {
                    // Guarded: a second press without an intervening release
                    // would open a gesture inside a gesture, which is not
                    // something a host has to make sense of.
                    self.dragging = true;
                    self.last_y = cx.mouse().cursory;
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.param.begin_set_parameter(cx);
                }
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseDown(MouseButton::Right) => {
                self.param.begin_set_parameter(cx);
                self.param
                    .set_normalized_value(cx, self.param.default_normalized_value());
                self.param.end_set_parameter(cx);
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.dragging {
                    self.finish(cx);
                    meta.consume();
                }
            }
            // Anything that means this window is no longer the one being used.
            // These are the events that do arrive when a drag is interrupted;
            // the check in `MouseMove` covers the times none of them does.
            WindowEvent::FocusOut | WindowEvent::WindowClose | WindowEvent::MouseCaptureOutEvent => {
                self.finish(cx);
            }
            WindowEvent::MouseMove(_, y) => {
                if self.dragging {
                    // The button came up somewhere this window never heard
                    // about. Without this the knob holds the mouse for good.
                    if cx.mouse().left.state == MouseButtonState::Released {
                        self.finish(cx);
                        return;
                    }
                    let speed = if cx.modifiers().shift() { FINE } else { 1.0 };
                    let delta = (self.last_y - *y) / (DRAG_RANGE * cx.scale_factor()) * speed;
                    self.last_y = *y;
                    self.nudge(cx, delta);
                    cx.needs_redraw();
                }
            }
            WindowEvent::MouseScroll(_, y) => {
                let step = if cx.modifiers().shift() { 0.005 } else { 0.02 };
                self.param.begin_set_parameter(cx);
                self.nudge(cx, y * step);
                self.param.end_set_parameter(cx);
                cx.needs_redraw();
                meta.consume();
            }
            _ => {}
        });
    }
}

///
/// Zero in the middle is where a preset was voiced. The lit band either side
/// of it is the range over which these circuits behave as the preset intends;
/// below it a pedal only clips under the pick attack and what you hear is
/// mostly the clean signal, which is the whole reason this meter exists.
pub struct Meter {
    meters: std::sync::Arc<crate::meters::Meters>,
}

impl Meter {
    /// The scale, in dB either side of nominal.
    const SPAN: f32 = 24.0;
    /// How far off nominal a circuit still does what the preset intends.
    const WORKING: f32 = 6.0;

    pub fn new<L>(cx: &mut Context, meters: L) -> Handle<'_, Self>
    where
        L: Lens<Target = std::sync::Arc<crate::meters::Meters>>,
    {
        let meters = meters.get(cx);
        let mut handle = Self { meters }.build(cx, |_| {}).hoverable(false);
        // A meter follows the audio, not the parameters, so it has to drive
        // its own repaint rather than waiting to be asked.
        let timer = handle.context().add_timer(
            std::time::Duration::from_millis(33),
            None,
            move |cx, action| {
                if let TimerAction::Tick(_) = action {
                    cx.needs_redraw();
                }
            },
        );
        handle.context().start_timer(timer);
        handle
    }

    fn position(&self, db: f32) -> f32 {
        ((db + Self::SPAN) / (2.0 * Self::SPAN)).clamp(0.0, 1.0)
    }
}

impl View for Meter {
    fn element(&self) -> Option<&'static str> {
        Some("frontend-meter")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();

        let mut track = vg::Path::new();
        track.rounded_rect(b.x, b.y, b.w, b.h, 3.0 * scale);
        canvas.fill_path(&track, &vg::Paint::color(rgb(0x0e1113)));
        canvas.stroke_path(
            &track,
            &vg::Paint::color(rgba(0xffffff, 0.10)).with_line_width(scale),
        );

        // The band the circuits were voiced in.
        let low = self.position(-Self::WORKING);
        let high = self.position(Self::WORKING);
        let mut band = vg::Path::new();
        band.rect(b.x + b.w * low, b.y + 2.0 * scale, b.w * (high - low), b.h - 4.0 * scale);
        canvas.fill_path(&band, &vg::Paint::color(rgba(GLOW, 0.16)));

        // Nominal itself.
        let mid = b.x + b.w * self.position(0.0);
        let mut centre = vg::Path::new();
        centre.rect(mid - 0.5 * scale, b.y + 2.0 * scale, scale, b.h - 4.0 * scale);
        canvas.fill_path(&centre, &vg::Paint::color(rgba(0xffffff, 0.35)));

        let db = self.meters.input_db();
        if db <= -Self::SPAN {
            return;
        }
        // A bar from the bottom of the scale to where the signal is, lit warm
        // once it is inside the band and cool while it is under it.
        let at = b.x + b.w * self.position(db);
        let mut bar = vg::Path::new();
        bar.rounded_rect(
            b.x + 2.0 * scale,
            b.y + 4.0 * scale,
            (at - b.x - 2.0 * scale).max(0.0),
            b.h - 8.0 * scale,
            2.0 * scale,
        );
        let colour = if db < -Self::WORKING {
            rgba(0x6f7d88, 0.85)
        } else {
            rgba(GLOW, 0.85)
        };
        canvas.fill_path(&bar, &vg::Paint::color(colour));
    }

}

/// A row of choices, one lit.
///
/// One widget for every selection on the panel rather than one per control.
/// The first attempt grew a bespoke segment widget for each -- circuit,
/// device, cabinet, topology -- five near-copies that drifted apart, so that
/// two of them silently stopped highlighting anything at all. A selection is a
/// selection: it needs the list, which one is on, and somewhere to send a
/// click.
pub struct Selector {
    param: ParamWidgetBase,
    /// What to write under each segment.
    labels: Vec<&'static str>,
    /// Whether the row is live. A greyed row still draws, so the panel does
    /// not change shape when a control stops applying -- it just stops
    /// claiming to mean anything.
    enabled: bool,
}

impl Selector {
    pub fn new<'a, L, Params, P, FMap>(
        cx: &'a mut Context,
        params: L,
        params_to_param: FMap,
        labels: Vec<&'static str>,
        enabled: bool,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let count = labels.len().max(1);
        let captions = labels.clone();
        Self {
            param: ParamWidgetBase::new(cx, params, params_to_param),
            labels,
            enabled,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, data| {
                let value = data.make_lens(|param| param.modulated_normalized_value());
                // The captions are real labels rather than text drawn onto the
                // canvas. A canvas paint with no font on it draws nothing at
                // all and reports no error, so a row of empty boxes is what
                // that mistake looks like.
                HStack::new(cx, |cx| {
                    for (index, caption) in captions.iter().enumerate() {
                        Label::new(cx, *caption)
                            .width(Stretch(1.0))
                            .height(Stretch(1.0))
                            .child_left(Stretch(1.0))
                            .child_right(Stretch(1.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0))
                            .font_size(11.0)
                            .color(value.map(move |v| {
                                let selected = (v * (count - 1) as f32).round() as usize;
                                match (enabled, selected.min(count - 1) == index) {
                                    (false, _) => Color::rgba(0xff, 0xff, 0xff, 0x33),
                                    (true, true) => Color::rgb(0xff, 0xb2, 0x6a),
                                    (true, false) => Color::rgb(0xa8, 0xb2, 0xba),
                                }
                            }))
                            .hoverable(false);
                    }
                })
                .width(Percentage(100.0))
                .height(Percentage(100.0))
                .hoverable(false);
                Binding::new(cx, value, |cx, _| cx.needs_redraw());
            }),
        )
    }

    /// Which segment is lit. A normalised value maps onto the step list the
    /// same way the parameter itself does, so this cannot drift from what the
    /// host thinks is selected.
    fn selected(&self) -> usize {
        let count = self.labels.len().max(1);
        let v = self.param.unmodulated_normalized_value();
        ((v * (count - 1) as f32).round() as usize).min(count - 1)
    }

    fn pick(&self, cx: &mut EventContext, index: usize) {
        let count = self.labels.len().max(1);
        let normalised = if count > 1 {
            index.min(count - 1) as f32 / (count - 1) as f32
        } else {
            0.0
        };
        self.param.begin_set_parameter(cx);
        self.param.set_normalized_value(cx, normalised);
        self.param.end_set_parameter(cx);
    }
}

impl View for Selector {
    fn element(&self) -> Option<&'static str> {
        Some("selector")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        if !self.enabled {
            return;
        }
        let b = cx.bounds();
        let count = self.labels.len().max(1) as f32;
        event.map(|window: &WindowEvent, meta| match window {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let x = cx.mouse().cursorx - b.x;
                let index = ((x / (b.w / count)).floor().max(0.0) as usize)
                    .min(self.labels.len().saturating_sub(1));
                self.pick(cx, index);
                meta.consume();
            }
            // The wheel steps through the list, which is what it does on every
            // other row and on the preset menu.
            WindowEvent::MouseScroll(_, y) => {
                let current = self.selected() as i64;
                let next = (current - y.signum() as i64)
                    .clamp(0, self.labels.len().saturating_sub(1) as i64);
                self.pick(cx, next as usize);
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let b = cx.bounds();
        let scale = cx.scale_factor();
        let count = self.labels.len().max(1);
        let seg = b.w / count as f32;
        let selected = self.selected();

        for index in 0..count {
            let x = b.x + seg * index as f32;
            let lit = index == selected;

            let mut cell = vg::Path::new();
            cell.rounded_rect(
                x + 1.5 * scale,
                b.y,
                seg - 3.0 * scale,
                b.h,
                3.0 * scale,
            );
            let ground = if lit {
                rgba(GLOW, if self.enabled { 0.22 } else { 0.07 })
            } else {
                rgba(0x000000, 0.20)
            };
            canvas.fill_path(&cell, &vg::Paint::color(ground));
            canvas.stroke_path(
                &cell,
                &vg::Paint::color(rgba(0xffffff, if lit { 0.20 } else { 0.07 }))
                    .with_line_width(scale),
            );

        }
    }
}
