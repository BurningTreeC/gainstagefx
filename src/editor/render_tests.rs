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
    let desc =
        WindowDescription::new().with_inner_size(PANEL_W as u32, window_height(FIRST_OPEN) as u32);
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
    assert_eq!(params.open_sections.load(Ordering::Relaxed), FIRST_OPEN);
    build_panel(
        backend.context(),
        params.clone(),
        Arc::new(crate::meters::Meters::default()),
        1.0,
    );
    let mut events = EventManager::new();
    let first = (PANEL_W as u32, window_height(FIRST_OPEN) as u32);
    let new_surface = |(width, height): (u32, u32)| {
        sk::surfaces::raster_n32_premul((width as i32, height as i32)).unwrap()
    };
    let mut surface = new_surface(first);
    let mut dirty = new_surface(first);
    let resized = std::rc::Rc::new(std::cell::Cell::new(None));
    let mut applied = first;
    let mut render = |backend: &mut BackendContext, name: &str| {
        for _ in 0..4 {
            events.flush_events(backend.context(), {
                let resized = resized.clone();
                move |event| {
                    if let WindowEvent::SetSize(size) = event {
                        resized.set(Some((size.width, size.height)));
                    }
                }
            });
            backend.process_style_updates();
            backend.process_animations();
            backend.process_visual_updates();
            backend.draw(Entity::root(), &mut surface, &mut dirty);
            // What the baseview backend does when the host has resized the
            // window, a frame later: fresh surfaces at the new size, the root
            // resized, and everything marked for layout and drawing
            // (`ApplicationRunner::handle_resized`).
            if let Some(size) = resized.get().filter(|size| *size != applied) {
                applied = size;
                surface = new_surface(size);
                dirty = new_surface(size);
                backend.set_window_size(Entity::root(), size.0 as f32, size.1 as f32);
                backend.needs_refresh(Entity::root());
            }
        }
        // Every pixel of the window is panel: the faceplate covers it all, so
        // one left transparent is one that was never drawn.
        let image = surface.image_snapshot();
        let info = image.image_info();
        let mut pixels = vec![0u8; (info.width() * info.height() * 4) as usize];
        image.read_pixels(
            info,
            &mut pixels,
            (info.width() * 4) as usize,
            (0, 0),
            sk::image::CachingHint::Allow,
        );
        let undrawn = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] != 255)
            .count();
        assert_eq!(
            undrawn,
            0,
            "{name}: {} of {}x{} pixels never drawn after the window resized",
            undrawn,
            info.width(),
            info.height()
        );
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
    // Opening a section, closing another: the stored set follows, and the
    // window is asked for the height of what is now open.
    for (index, name, open) in [
        (2, "drive-open", 0b000101),
        (0, "input-closed", 0b000100),
        (5, "output-open", 0b100100),
    ] {
        backend.context().emit(PanelEvent::ToggleSection(index));
        render(&mut backend, name);
        assert_eq!(params.open_sections.load(Ordering::Relaxed), open, "{name}");
        assert_eq!(
            resized.take(),
            Some((PANEL_W as u32, window_height(open) as u32)),
            "{name}"
        );
    }
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
