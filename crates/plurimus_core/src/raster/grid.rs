//! The halfblock grid: two independently colored points per terminal cell.
//!
//! Drawing `▀` with a foreground and a background color paints the top and
//! bottom of one cell separately, giving twice the vertical resolution
//! without giving up per-point color. Pipelines set points by subcell
//! coordinate and [`HalfblockGrid::resolve_into`] collapses the grid onto a
//! cell buffer. A cell with only one half set draws over whatever was there
//! already, so partial coverage composites instead of blanking the cell, and
//! one grid is reset and refilled each frame so a steady size stops
//! allocating.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Color;
use ratatui_core::symbols::half_block;

/// A color grid at double vertical resolution over a cell area, resolved
/// into half-block glyphs. Coordinates are screen cells (`x`) and
/// half-rows (`sub_y` = `cell_y * 2` for the upper half).
#[derive(Default)]
pub struct HalfblockGrid {
    area: Rect,
    halves: Vec<Option<Color>>,
}

impl HalfblockGrid {
    /// An empty grid covering `area` at twice its vertical resolution.
    #[must_use]
    pub fn new(area: Rect) -> Self {
        let mut grid = Self::default();
        grid.reset(area);
        grid
    }

    /// Clears the grid and refits it to `area`, retaining allocated
    /// capacity so per-frame reuse stays allocation-free at a stable size.
    pub fn reset(&mut self, area: Rect) {
        self.area = area;
        self.halves.clear();
        self.halves
            .resize(area.width as usize * area.height as usize * 2, None);
    }

    /// The grid's area in absolute subcell coordinates: cell columns,
    /// half-rows at twice the vertical resolution.
    #[must_use]
    pub fn subcell_area(&self) -> Rect {
        Rect::new(
            self.area.x,
            self.area.y * 2,
            self.area.width,
            self.area.height * 2,
        )
    }

    /// Sets a subcell pixel; out-of-area coordinates are ignored.
    pub fn set(&mut self, x: u16, sub_y: u16, color: Color) {
        if let Some(index) = self.index(x, sub_y) {
            self.halves[index] = Some(color);
        }
    }

    fn index(&self, x: u16, sub_y: u16) -> Option<usize> {
        let column = usize::from(x).checked_sub(usize::from(self.area.left()))?;
        let row = usize::from(sub_y).checked_sub(usize::from(self.area.top()) * 2)?;
        let width = usize::from(self.area.width);
        (column < width && row < usize::from(self.area.height) * 2).then(|| row * width + column)
    }

    /// Resolves into `buffer`: both halves set draws `▀` with the upper
    /// color as foreground and the lower as background; a single half
    /// draws its half-block over the cell's existing background.
    pub fn resolve_into(&self, buffer: &mut Buffer) {
        let width = usize::from(self.area.width);
        if width == 0 {
            return;
        }
        for (row, halves) in self.halves.chunks_exact(width * 2).enumerate() {
            for column in 0..width {
                let x = self.area.left() + column as u16;
                let y = self.area.top() + row as u16;
                write_halves(buffer, x, y, halves[column], halves[width + column]);
            }
        }
    }
}

fn write_halves(buffer: &mut Buffer, x: u16, y: u16, upper: Option<Color>, lower: Option<Color>) {
    let Some(cell) = buffer.cell_mut((x, y)) else {
        return;
    };
    match (upper, lower) {
        (Some(up), Some(low)) => {
            cell.set_char(half_block::UPPER);
            cell.set_fg(up);
            cell.set_bg(low);
        }
        (Some(up), None) => {
            cell.set_char(half_block::UPPER);
            cell.set_fg(up);
        }
        (None, Some(low)) => {
            cell.set_char(half_block::LOWER);
            cell.set_fg(low);
        }
        (None, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::{Color, Style};

    use super::HalfblockGrid;

    #[test]
    fn both_halves_resolve_to_fg_bg_pair() {
        let area = Rect::new(2, 1, 3, 2);
        let mut grid = HalfblockGrid::new(area);
        grid.set(3, 2, Color::Red);
        grid.set(3, 3, Color::Blue);
        let mut buffer = Buffer::empty(area);

        grid.resolve_into(&mut buffer);

        let cell = buffer.cell((3, 1)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Red);
        assert_eq!(cell.bg, Color::Blue);
    }

    #[test]
    fn single_half_preserves_cell_background() {
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(0, 0, "  ", Style::new().bg(Color::Green));
        let mut grid = HalfblockGrid::new(area);
        grid.set(0, 0, Color::Red);
        grid.set(1, 1, Color::Blue);

        grid.resolve_into(&mut buffer);

        let upper_only = buffer.cell((0, 0)).unwrap();
        assert_eq!(upper_only.symbol(), "▀");
        assert_eq!(upper_only.fg, Color::Red);
        assert_eq!(upper_only.bg, Color::Green);
        let lower_only = buffer.cell((1, 0)).unwrap();
        assert_eq!(lower_only.symbol(), "▄");
        assert_eq!(lower_only.fg, Color::Blue);
        assert_eq!(lower_only.bg, Color::Green);
    }

    #[test]
    fn reset_clears_stale_content_and_refits_the_area() {
        let mut grid = HalfblockGrid::new(Rect::new(0, 0, 1, 1));
        grid.set(0, 0, Color::Red);

        grid.reset(Rect::new(0, 0, 2, 1));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        grid.resolve_into(&mut buffer);
        assert!(buffer.content.iter().all(|cell| cell.symbol() == " "));
        grid.set(1, 0, Color::Blue);
        grid.resolve_into(&mut buffer);
        let cell = buffer.cell((1, 0)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Blue);
    }

    #[test]
    fn out_of_area_sets_are_ignored() {
        let area = Rect::new(1, 1, 2, 1);
        let mut grid = HalfblockGrid::new(area);
        grid.set(0, 2, Color::Red);
        grid.set(3, 2, Color::Red);
        grid.set(1, 0, Color::Red);
        grid.set(1, 4, Color::Red);
        let mut buffer = Buffer::empty(area);

        grid.resolve_into(&mut buffer);

        assert!(buffer.content.iter().all(|cell| cell.symbol() == " "));
    }
}
