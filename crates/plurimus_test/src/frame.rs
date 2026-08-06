//! Composed-frame snapshot formatters.
//!
//! Reads the [`FrameBuffer`](plurimus_core::FrameBuffer) straight out of the
//! render sub-app, so a test sees exactly what the presenter would have
//! written without a terminal or a backend. Plain text answers what was
//! drawn; the styled form adds a per-cell legend for when color or modifiers
//! are the thing under test.

use bevy_app::App;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::{FrameBuffer, TerminalRenderApp};

/// The app's composed frame as one line of cell symbols per row.
#[must_use]
pub fn composed_frame(app: &App) -> String {
    frame_to_string(&composed_buffer(app).0)
}

/// The app's composed frame as symbols plus a style map: a letter grid
/// referencing a legend of distinct non-default styles (`.` marks the
/// default style).
#[must_use]
pub fn composed_styled_frame(app: &App) -> String {
    frame_to_styled_string(&composed_buffer(app).0)
}

fn composed_buffer(app: &App) -> &FrameBuffer {
    app.sub_app(TerminalRenderApp)
        .world()
        .resource::<FrameBuffer>()
}

fn frame_to_string(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut lines = Vec::with_capacity(area.height as usize);
    for y in area.top()..area.bottom() {
        let mut line = String::with_capacity(area.width as usize);
        for x in area.left()..area.right() {
            if let Some(cell) = buffer.cell((x, y)) {
                line.push_str(cell.symbol());
            }
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn frame_to_styled_string(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut legend: Vec<Style> = Vec::new();
    let mut symbol_rows = Vec::with_capacity(area.height as usize);
    let mut style_rows = Vec::with_capacity(area.height as usize);
    for y in area.top()..area.bottom() {
        let mut symbols = String::new();
        let mut styles = String::new();
        for x in area.left()..area.right() {
            if let Some(cell) = buffer.cell((x, y)) {
                symbols.push_str(cell.symbol());
                styles.push(style_letter(cell.style(), &mut legend));
            }
        }
        symbol_rows.push(symbols);
        style_rows.push(styles);
    }
    let mut out = symbol_rows.join("\n");
    out.push_str("\n--\n");
    out.push_str(&style_rows.join("\n"));
    for (index, style) in legend.iter().enumerate() {
        out.push_str(&format!("\n{}: {}", letter(index), describe(*style)));
    }
    out
}

// Formats only fields that exist regardless of ratatui-core features, so
// snapshots stay identical across feature unification.
fn describe(style: Style) -> String {
    format!(
        "fg:{:?} bg:{:?} mods:{:?}",
        style.fg, style.bg, style.add_modifier
    )
}

fn style_letter(style: Style, legend: &mut Vec<Style>) -> char {
    if style == Style::default() {
        return '.';
    }
    let index = legend
        .iter()
        .position(|known| *known == style)
        .unwrap_or_else(|| {
            legend.push(style);
            legend.len() - 1
        });
    letter(index)
}

fn letter(index: usize) -> char {
    char::from(b'a' + u8::try_from(index % 26).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use plurimus_core::ratatui_core::buffer::Buffer;
    use plurimus_core::ratatui_core::layout::Rect;
    use plurimus_core::ratatui_core::style::Style;

    use super::frame_to_string;

    #[test]
    fn renders_rows_as_lines() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 2));
        buffer.set_string(0, 0, "ab", Style::new());
        buffer.set_string(1, 1, "c", Style::new());

        assert_eq!(frame_to_string(&buffer), "ab \n c ");
    }
}
