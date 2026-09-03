//! The GainStageFx front panel.
//!
//! The panel is the signal path, drawn top to bottom, and that is the only
//! organising idea in it. Six numbered bands: what arrives, what does the
//! work, how hard it is worked, what is taken out afterwards, what it comes
//! out of, what leaves. An arrow at the foot of each band points into the
//! next.
//!
//! This is deliberate and it is the one thing the previous version could not
//! be given afterwards. Its controls sat where they had been added, so the
//! panel recorded the order the plugin was built in rather than the order the
//! signal travels, and nothing on it said what happened before what. A layout
//! is a claim about how a thing works; that one made no claim at all.

mod panel;
mod session;
mod sprites;
mod style;
mod widgets;

use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;

use crate::params::{Cabinet, Circuit, Diode, GainStageParams, Oversampling, ToneStack};
use panel::Faceplate;
use style::*;
use widgets::{Knob, Meter, Selector};

#[derive(Lens)]
pub struct Panel {
    pub params: Arc<GainStageParams>,
    pub meters: Arc<crate::meters::Meters>,
}

impl Model for Panel {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (PANEL_W as u32, WINDOW_H as u32))
}

/// Height of a label box, which is centred on its anchor point.
const LABEL_H: f32 = 16.0;

/// Picking one knob's parameter out of the set.
type ToKnob = fn(&Arc<GainStageParams>) -> &nih_plug::prelude::FloatParam;

pub fn create(
    params: Arc<GainStageParams>,
    meters: Arc<crate::meters::Meters>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, _| {
        assets::register_noto_sans_regular(cx);
        assets::register_noto_sans_bold(cx);

        Panel {
            params: params.clone(),
            meters: meters.clone(),
        }
        .build(cx);

        session::Session::build_into(cx, params.clone());

        Faceplate::new(cx);
        gutter(cx);
        strip(cx);
        input(cx);
        circuit(cx);
        drive(cx);
        tone(cx);
        cabinet(cx);
        output(cx);

        // Last, so it draws over the panel and takes the clicks first.
        session::menu(cx);
    })
}

// ---------------------------------------------------------------------------
// Pieces every section is built from
// ---------------------------------------------------------------------------

/// Small print, centred on a point.
fn label(cx: &mut Context, text: &str, x: f32, y: f32, size: f32, width: f32, colour: u32) {
    Label::new(cx, text)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - width / 2.0))
        .top(Pixels(y - LABEL_H / 2.0))
        .width(Pixels(width))
        .height(Pixels(LABEL_H))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_size(size)
        .color(Color::rgb(
            ((colour >> 16) & 0xff) as u8,
            ((colour >> 8) & 0xff) as u8,
            (colour & 0xff) as u8,
        ))
        .hoverable(false);
}

/// A knob with its name under it and its value under that.
///
/// Both lines, always. A knob whose value can only be discovered by dragging
/// it is a knob you cannot set deliberately, and every one of these has a
/// number worth knowing.
fn knob<P, F>(cx: &mut Context, x: f32, y: f32, radius: f32, name: &str, to_param: F, read: P)
where
    F: Fn(&Arc<GainStageParams>) -> &nih_plug::prelude::FloatParam + Copy + 'static,
    P: Fn(&Arc<GainStageParams>) -> String + Clone + 'static,
{
    Knob::new(cx, Panel::params, to_param, radius)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - radius))
        .top(Pixels(y - radius));

    label(cx, name, x, y + radius + 12.0, 10.5, 110.0, 0x9aa6b0);

    Label::new(cx, Panel::params.map(move |p| read(p)))
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x - 55.0))
        .top(Pixels(y + radius + 24.0 - LABEL_H / 2.0))
        .width(Pixels(110.0))
        .height(Pixels(LABEL_H))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_size(10.0)
        .color(Color::rgb(0xff, 0xb2, 0x6a))
        .hoverable(false);
}

/// A row of choices, sized to the space it is given.
fn selector<P, F>(
    cx: &mut Context,
    x: f32,
    y: f32,
    width: f32,
    to_param: F,
    labels: Vec<&'static str>,
    enabled: bool,
) where
    F: Fn(&Arc<GainStageParams>) -> &P + Copy + 'static,
    P: nih_plug::prelude::Param + 'static,
{
    Selector::new(cx, Panel::params, to_param, labels, enabled)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(x))
        .top(Pixels(y))
        .width(Pixels(width))
        .height(Pixels(22.0));
}

/// Where a section's controls start, clear of the numbered gutter.
fn body_x() -> f32 {
    GUTTER_W + 16.0
}

fn body_w() -> f32 {
    PANEL_W - body_x() - 24.0
}

/// The numbering down the left, which is what makes the order legible before
/// anything else on the panel is read.
fn gutter(cx: &mut Context) {
    for (index, (number, name, height)) in SECTIONS.iter().enumerate() {
        let mid = section_top(index) + height / 2.0;
        Label::new(cx, *number)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(16.0))
            .top(Pixels(mid - 18.0))
            .width(Pixels(26.0))
            .height(Pixels(36.0))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_size(30.0)
            .color(Color::rgba(0xff, 0xff, 0xff, 0x24))
            .hoverable(false);
        label(cx, name, 68.0, mid, 10.5, 60.0, 0x8b959d);
    }
}

// ---------------------------------------------------------------------------
// The strip above the panel
// ---------------------------------------------------------------------------

fn strip(cx: &mut Context) {
    label(cx, "GAINSTAGEFX", 78.0, HEADER_H / 2.0, 11.5, 140.0, 0xe8eef4);

    label(cx, "preset", 182.0, HEADER_H / 2.0, 10.0, 48.0, 0x7e8a96);
    session::PresetButton::new(cx)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(session::BUTTON_X))
        .top(Pixels(HEADER_H / 2.0 - 11.0))
        .width(Pixels(session::BUTTON_W))
        .height(Pixels(22.0));

    // Oversampling belongs up here rather than in a band: it changes what the
    // plugin costs, not what it sounds like, and putting it in the signal path
    // would say otherwise.
    let width = 132.0;
    let left = PANEL_W - 16.0 - width;
    label(cx, "quality", left - 30.0, HEADER_H / 2.0, 10.0, 52.0, 0x7e8a96);
    selector(
        cx,
        left,
        HEADER_H / 2.0 - 11.0,
        width,
        |p| &p.oversampling,
        Oversampling::ALL.iter().map(|o| o.name()).collect(),
        true,
    );
}

// ---------------------------------------------------------------------------
// 1 Input
// ---------------------------------------------------------------------------

fn input(cx: &mut Context) {
    let top = section_top(0);
    let mid = top + SECTIONS[0].2 / 2.0;

    knob(
        cx,
        body_x() + 40.0,
        mid - 6.0,
        26.0,
        "TRIM",
        |p| &p.input_trim,
        |p| format!("{:+.1} dB", p.input_trim.value()),
    );

    // The meter reads against the level the circuits were calibrated at, so
    // its zero is the only place on the panel where every other control means
    // what its label says.
    let meter_x = body_x() + 130.0;
    let meter_w = body_w() - 146.0;
    Meter::new(cx, Panel::meters)
        .position_type(PositionType::SelfDirected)
        .left(Pixels(meter_x))
        .top(Pixels(mid - 20.0))
        .width(Pixels(meter_w))
        .height(Pixels(16.0));

    label(
        cx,
        "signal arriving at the circuit, against the level it was voiced at",
        meter_x + meter_w / 2.0,
        mid + 14.0,
        10.0,
        meter_w,
        0x7e8a96,
    );
}

// ---------------------------------------------------------------------------
// 2 Circuit
// ---------------------------------------------------------------------------

fn circuit(cx: &mut Context) {
    let top = section_top(1);

    label(cx, "topology", body_x() + 34.0, top + 20.0, 10.0, 80.0, 0x7e8a96);
    selector(
        cx,
        body_x() + 84.0,
        top + 10.0,
        body_w() - 84.0,
        |p| &p.circuit,
        Circuit::ALL.iter().map(|c| c.name()).collect(),
        true,
    );

    label(cx, "clipping", body_x() + 34.0, top + 54.0, 10.0, 80.0, 0x7e8a96);
    // Greyed rather than hidden when the circuit has no diodes in it: a
    // control that vanishes makes the panel change shape, and a control that
    // stays but stops claiming to do anything is easier to trust.
    Binding::new(
        cx,
        Panel::params.map(|p| p.circuit.value().has_diodes()),
        |cx, live| {
            let live = live.get(cx);
            selector(
                cx,
                body_x() + 84.0,
                top_of_clipping(),
                220.0,
                |p| &p.diode,
                Diode::ALL.iter().map(|d| d.name()).collect(),
                live,
            );
        },
    );

    // The description gets the full width of its own line rather than the
    // space left over beside a control, because it is a sentence and a
    // sentence squeezed into a gap wraps into something nobody reads.
    Label::new(cx, Panel::params.map(|p| describe(p.circuit.value())))
        .position_type(PositionType::SelfDirected)
        .left(Pixels(body_x()))
        .top(Pixels(top + 76.0))
        .width(Pixels(body_w()))
        .height(Pixels(32.0))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0))
        .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
        .font_size(10.0)
        .color(Color::rgb(0x86, 0x92, 0x9c))
        .hoverable(false);
}

fn top_of_clipping() -> f32 {
    section_top(1) + 44.0
}

/// One line saying what the selected circuit actually is. It changes with the
/// selection, so it is a single line rather than five pieces of permanent
/// small print nobody reads.
fn describe(circuit: Circuit) -> String {
    match circuit {
        Circuit::Clean => "One valve stage barely working. The sound of a signal \
                           having been through something, not of distortion.",
        Circuit::Crunch => "Two stages, the second driven by the first, so each \
                            amplifies the last one's distortion as well.",
        Circuit::HighGain => "Three stages run hard, all clipping on every note. \
                              Where the gain stops being a texture.",
        Circuit::Overdrive => "Diodes across the feedback resistor: they lower the \
                               gain, so it keeps following and cleans up.",
        Circuit::Distortion => "Diodes across the signal to ground: a ceiling. The \
                                wave is squared off, top to bottom of the band.",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// 3 Drive
// ---------------------------------------------------------------------------

fn drive(cx: &mut Context) {
    let top = section_top(2);
    let mid = top + SECTIONS[2].2 / 2.0;

    knob(
        cx,
        body_x() + 46.0,
        mid - 12.0,
        32.0,
        "DRIVE",
        |p| &p.drive,
        |p| format!("{:.0} %", p.drive.value() * 100.0),
    );

    let x = body_x() + 130.0 + (body_w() - 146.0) / 2.0;
    label(cx, "All the way up is the sound the circuit is named for.", x, mid - 20.0, 10.5, 440.0, 0x86929c);
    label(cx, "Down from there only cleans up: the level is held across", x, mid - 4.0, 10.5, 440.0, 0x86929c);
    label(cx, "the whole travel, so what changes is character, not loudness.", x, mid + 12.0, 10.5, 440.0, 0x86929c);
}

// ---------------------------------------------------------------------------
// 4 Tone
// ---------------------------------------------------------------------------

fn tone(cx: &mut Context) {
    let top = section_top(3);

    label(cx, "stack", body_x() + 34.0, top + 20.0, 10.0, 80.0, 0x7e8a96);
    selector(
        cx,
        body_x() + 84.0,
        top + 10.0,
        260.0,
        |p| &p.tone,
        ToneStack::ALL.iter().map(|t| t.name()).collect(),
        true,
    );

    let names: [(&str, ToKnob); 3] = [
        ("BASS", |p| &p.bass),
        ("MID", |p| &p.mid),
        ("TREBLE", |p| &p.treble),
    ];
    for (i, (name, to_param)) in names.into_iter().enumerate() {
        let x = body_x() + 56.0 + i as f32 * 104.0;
        Knob::new(cx, Panel::params, to_param, 22.0)
            .position_type(PositionType::SelfDirected)
            .left(Pixels(x - 22.0))
            .top(Pixels(top + 46.0));
        label(cx, name, x, top + 104.0, 10.5, 90.0, 0x9aa6b0);
    }

    // Kept to lines that fit the space rather than sentences that overflow
    // it: text wider than its box is simply clipped, with no warning.
    let x = body_x() + 400.0;
    label(cx, "A passive stack only ever cuts.", x, top + 56.0, 10.0, 260.0, 0x86929c);
    label(cx, "The scooping voicing has a resonant leg,", x, top + 72.0, 10.0, 260.0, 0x86929c);
    label(cx, "which puts a dip in the middle rather", x, top + 88.0, 10.0, 260.0, 0x86929c);
    label(cx, "than a shelf at each end.", x, top + 104.0, 10.0, 260.0, 0x86929c);
}

// ---------------------------------------------------------------------------
// 5 Cabinet
// ---------------------------------------------------------------------------

fn cabinet(cx: &mut Context) {
    let top = section_top(4);
    let mid = top + SECTIONS[4].2 / 2.0;

    selector(
        cx,
        body_x(),
        mid - 20.0,
        260.0,
        |p| &p.cabinet,
        Cabinet::ALL.iter().map(|c| c.name()).collect(),
        true,
    );

    let x = body_x() + 420.0;
    label(cx, "A speaker is most of what a distorted", x, mid - 22.0, 10.0, 300.0, 0x86929c);
    label(cx, "amplifier sounds like. Without one the", x, mid - 6.0, 10.0, 300.0, 0x86929c);
    label(cx, "top of the band is bare -- and a preamp", x, mid + 10.0, 10.0, 300.0, 0x86929c);
    label(cx, "sound wants it off.", x, mid + 26.0, 10.0, 300.0, 0x86929c);
}

// ---------------------------------------------------------------------------
// 6 Output
// ---------------------------------------------------------------------------

fn output(cx: &mut Context) {
    let top = section_top(5);
    let mid = top + SECTIONS[5].2 / 2.0;

    knob(
        cx,
        body_x() + 46.0,
        mid - 12.0,
        26.0,
        "MIX",
        |p| &p.mix,
        |p| format!("{:.0} %", p.mix.value() * 100.0),
    );
    knob(
        cx,
        body_x() + 166.0,
        mid - 12.0,
        26.0,
        "LEVEL",
        |p| &p.output_trim,
        |p| format!("{:+.1} dB", p.output_trim.value()),
    );

    let x = body_x() + 420.0;
    label(cx, "The dry signal is delayed to match, so", x, mid - 14.0, 10.0, 300.0, 0x86929c);
    label(cx, "mixing the two is a mix and not a comb.", x, mid + 2.0, 10.0, 300.0, 0x86929c);
}
