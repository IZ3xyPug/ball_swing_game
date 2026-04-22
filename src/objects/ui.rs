// ── objects/ui.rs ─────────────────────────────────────────────────────────────
use quartz::*;
use std::sync::Arc;

pub fn ui_text_spec(text: &str, font: &Font, font_size: f32, color: Color, width: f32) -> Text {
    Text::new(
        vec![Span::new(
            text.to_string(),
            font_size,
            Some(font_size * 1.25),
            Arc::new(font.clone()),
            color,
            0.0,
        )],
        Some(width),
        Align::Center,
        None,
    )
}
