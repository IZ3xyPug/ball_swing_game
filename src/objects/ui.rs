// ── objects/ui.rs ─────────────────────────────────────────────────────────────
use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

/// One `Arc<Font>` per font file, shared by every span.
///
/// The renderer caches rasterised glyphs per `Arc<Font>` (by pointer), so a
/// fresh `Arc::new(font.clone())` per span is a cold cache: every glyph of
/// the text is rasterised at 160 px and uploaded again. The shop rebuilt its
/// labels that way on every scroll (and its rainbow titles every frame),
/// which is what made the carousel hitch.
pub fn shared_font(font: &Font) -> Arc<Font> {
    static FONTS: OnceLock<Mutex<Vec<Arc<Font>>>> = OnceLock::new();
    let mut fonts = FONTS.get_or_init(Default::default).lock().unwrap();
    let h = font.file_hash();
    if let Some(f) = fonts.iter().find(|f| f.file_hash() == h) {
        return f.clone();
    }
    let f = Arc::new(font.clone());
    fonts.push(f.clone());
    f
}

pub fn ui_text_spec(text: &str, font: &Font, font_size: f32, color: Color, width: f32) -> Text {
    Text::new(
        vec![Span::new(
            text.to_string(),
            font_size,
            Some(font_size * 1.25),
            shared_font(font),
            color,
            0.0,
        )],
        Some(width),
        Align::Center,
        None,
    )
}

pub fn ui_text_left_spec(text: &str, font: &Font, font_size: f32, color: Color, width: f32) -> Text {
    Text::new(
        vec![Span::new(
            text.to_string(),
            font_size,
            Some(font_size * 1.25),
            shared_font(font),
            color,
            0.0,
        )],
        Some(width),
        Align::Left,
        None,
    )
}
