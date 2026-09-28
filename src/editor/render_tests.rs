//! Exercise the real reactive view tree with Skia's raster surface. No display server.
use super::*;
use vizia_plug::vizia::{
    backend::{BackendContext, WindowDescription},
    events::EventManager,
    vg as sk,
};
use vizia_plug::widgets::{param_registry::ParamRegistry, RawParamEvent};

struct Root;
impl View for Root {}

#[test]
fn panel_renders_and_dialogs_follow_signals() {
    Runtime::init_on_ui_thread();
    let mut cx = Context::new();
    cx.ignore_default_theme = true;
    let mut backend = BackendContext::new(cx);
    let desc = WindowDescription::new().with_inner_size(PANEL_W as u32, WINDOW_H as u32);
    backend.add_main_window(Entity::root(), &desc, 1.0);
    backend.add_window(Root);
    backend.0.windows.insert(
        Entity::root(),
        WindowState {
            window_description: desc,
            ..Default::default()
        },
    );
    backend.context().add_built_in_styles();
    ParamRegistry::new().build(backend.context());
    let params = Arc::new(GainStageParams::default());
    build_panel(
        backend.context(),
        params,
        Arc::new(crate::meters::Meters::default()),
        1.0,
    );
    let mut events = EventManager::new();
    let mut surface = sk::surfaces::raster_n32_premul((PANEL_W as i32, WINDOW_H as i32)).unwrap();
    let mut dirty = sk::surfaces::raster_n32_premul((PANEL_W as i32, WINDOW_H as i32)).unwrap();
    let mut render = |backend: &mut BackendContext, name: &str| {
        for _ in 0..4 {
            events.flush_events(backend.context(), |_| {});
            backend.process_style_updates();
            backend.process_animations();
            backend.process_visual_updates();
            backend.draw(Entity::root(), &mut surface, &mut dirty);
        }
        if let Ok(directory) = std::env::var("GAINSTAGEFX_GUI_SNAPSHOTS") {
            let data = surface
                .image_snapshot()
                .encode(None, sk::EncodedImageFormat::PNG, None)
                .unwrap();
            std::fs::write(
                std::path::Path::new(&directory).join(format!("{name}.png")),
                data.as_bytes(),
            )
            .unwrap();
        }
    };
    render(&mut backend, "panel");
    // Exercise automation notifications as well as each transient overlay.
    backend.context().emit(RawParamEvent::ParametersChanged);
    for (event, name) in [
        (session::SessionEvent::Toggle, "presets"),
        (session::SessionEvent::Close, "closed"),
        (session::SessionEvent::ToggleSize, "sizes"),
        (session::SessionEvent::OpenSave, "save"),
        (session::SessionEvent::Cancel, "cancelled"),
    ] {
        backend.context().emit(event);
        render(&mut backend, name);
        if name == "save" {
            assert_eq!(backend.focused_element(), Some("textbox"));
            for c in "A typed preset".chars() {
                backend.emit_origin(WindowEvent::CharInput(c));
            }
            render(&mut backend, "typed");
        }
    }
    assert_eq!(
        backend.context().data::<session::Session>().draft,
        "A typed preset"
    );
    assert_eq!(
        backend.context().data::<session::Session>().dialog,
        session::Dialog::None
    );
    Runtime::deinit_on_ui_thread();
}
