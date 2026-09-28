use vizia_plug::vizia::prelude::Context;
pub const ROBOTO: &str = "Roboto";
pub fn register_roboto(cx: &mut Context) {
    cx.load_font_mem(include_bytes!("../../assets/fonts/Roboto-Regular.ttf"));
}
pub fn register_roboto_bold(cx: &mut Context) {
    cx.load_font_mem(include_bytes!("../../assets/fonts/Roboto-Bold.ttf"));
}
