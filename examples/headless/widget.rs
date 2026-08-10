//! A widget written against [`TerminalWidget`] directly.
//!
//! Most widgets are ratatui types, which plurimus accepts through a blanket
//! impl. This one implements the trait by hand to show the seam a backend
//! or widget library actually builds on: a `Rect` and a `Buffer`, and no
//! terminal anywhere in the signature.

use plurimus::core::TerminalWidget;
use plurimus::core::ratatui_core::buffer::Buffer;
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::ratatui_core::style::{Color, Style};

/// Eighths of a cell, so a bar resolves finer than a row.
const BARS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// A row of bars whose heights advance with `phase`.
pub struct Sparkline {
    phase: u16,
}

impl Sparkline {
    pub const fn new(phase: u16) -> Self {
        Self { phase }
    }

    /// A repeatable height per column, so a frame is reproducible in a test.
    const fn height(&self, column: u16) -> u16 {
        (column.wrapping_mul(3).wrapping_add(self.phase)) % 16
    }
}

impl TerminalWidget for Sparkline {
    fn render(&self, area: Rect, buffer: &mut Buffer) {
        for column in 0..area.width {
            let height = self.height(column);
            let full = height / 2;
            let remainder = height % 2;
            for row in 0..area.height {
                let from_bottom = area.height - 1 - row;
                let Some(cell) = buffer.cell_mut((area.x + column, area.y + row)) else {
                    continue;
                };
                let symbol = if from_bottom < full {
                    BARS[7]
                } else if from_bottom == full && remainder == 1 {
                    BARS[3]
                } else {
                    continue;
                };
                cell.set_symbol(symbol);
                let green = 180u8.saturating_sub((height * 8) as u8);
                cell.set_style(Style::new().fg(Color::Rgb(60, green, 220)));
            }
        }
    }
}
