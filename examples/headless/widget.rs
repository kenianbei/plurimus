//! A widget written against [`TerminalWidget`] directly.
//!
//! Most widgets are ratatui types, which plurimus accepts through a blanket
//! impl. This one implements the trait by hand to show the seam a backend
//! or widget library builds on: a `Rect` and a `Buffer`, and nothing about
//! where either ends up.
//!
//! It draws through [`HalfblockGrid`], core's own subcell primitive, which
//! is what buys twice the vertical resolution and a colour per half-cell -
//! neither of which a table of block glyphs can manage.

use plurimus::core::TerminalWidget;
use plurimus::core::raster::HalfblockGrid;
use plurimus::core::ratatui_core::buffer::Buffer;
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::ratatui_core::style::Color;

/// A bar per column, each as tall as `phase` decides.
pub struct Waveform {
    phase: u16,
}

impl Waveform {
    pub const fn new(phase: u16) -> Self {
        Self { phase }
    }

    /// Repeatable per column, so a frame is reproducible in a test.
    const fn height(&self, column: u16, subcells: u16) -> u16 {
        let raw = column.wrapping_mul(5).wrapping_add(self.phase) % 32;
        // Scaled into the grid rather than assumed, since the widget draws
        // at whatever height its area gives it.
        raw * subcells / 32
    }
}

impl TerminalWidget for Waveform {
    fn render(&self, area: Rect, buffer: &mut Buffer) {
        let mut grid = HalfblockGrid::new(area);
        // Subcell coordinates are absolute, not area-relative: a widget on
        // a camera further down the screen starts further down the grid.
        let subcells = grid.subcell_area();
        for column in 0..subcells.width {
            let height = self.height(column, subcells.height);
            for step in 0..height {
                // Rows count from the top, so a bar grows up from the last.
                let sub_y = subcells.bottom() - 1 - step;
                let lit = 80 + u8::try_from(step * 5).unwrap_or(u8::MAX).min(160);
                grid.set(subcells.x + column, sub_y, Color::Rgb(40, lit, 200));
            }
        }
        grid.resolve_into(buffer);
    }
}
