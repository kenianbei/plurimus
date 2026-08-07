//! The braille grid: 2×4 dots per cell, at the cost of a single color.
//!
//! A braille pattern addresses eight points in one cell, four times what a
//! halfblock offers, but the cell carries one foreground color and no
//! background, so every dot in it shares that color. The color is the average
//! of the dots written to the cell, which means a later write blends with the
//! earlier ones rather than covering them - a pipeline that needs one point
//! to occlude another wants [`HalfblockGrid`](super::HalfblockGrid) instead.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Color;
use ratatui_core::symbols::braille::BRAILLE;

use super::average::LinearColorSum;

/// A dot grid at 2×4 subcell resolution over a cell area, resolved into
/// braille glyphs. Coordinates are subcell columns (`sub_x` =
/// `cell_x * 2`) and subcell rows (`sub_y` = `cell_y * 4`). Braille is
/// foreground-only: a cell renders one color, the average of its set
/// dots, and its background is never written.
#[derive(Default)]
pub struct BrailleGrid {
    area: Rect,
    dots: Vec<u8>,
    colors: Vec<ColorSum>,
}

impl BrailleGrid {
    /// An empty grid covering `area` at 2×4 dots per cell.
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
        let cells = area.width as usize * area.height as usize;
        self.dots.clear();
        self.dots.resize(cells, 0);
        self.colors.clear();
        self.colors.resize(cells, ColorSum::default());
    }

    /// The grid's area in absolute subcell coordinates: doubled columns,
    /// quadrupled rows.
    #[must_use]
    pub const fn subcell_area(&self) -> Rect {
        Rect::new(
            self.area.x * 2,
            self.area.y * 4,
            self.area.width * 2,
            self.area.height * 4,
        )
    }

    /// Sets a subcell dot; out-of-area coordinates are ignored.
    pub fn set(&mut self, sub_x: u16, sub_y: u16, color: Color) {
        let Some((index, bit)) = self.index(sub_x, sub_y) else {
            return;
        };
        self.dots[index] |= 1 << bit;
        self.colors[index].add(color);
    }

    fn index(&self, sub_x: u16, sub_y: u16) -> Option<(usize, u8)> {
        let column = usize::from(sub_x).checked_sub(usize::from(self.area.left()) * 2)?;
        let row = usize::from(sub_y).checked_sub(usize::from(self.area.top()) * 4)?;
        let width = usize::from(self.area.width);
        if column >= width * 2 || row >= usize::from(self.area.height) * 4 {
            return None;
        }
        let bit = ((row % 4) * 2 + column % 2) as u8;
        Some(((row / 4) * width + column / 2, bit))
    }

    /// Resolves into `buffer`: cells with any dot set draw their braille
    /// pattern with the resolved foreground color; empty cells and every
    /// cell background are untouched.
    pub fn resolve_into(&self, buffer: &mut Buffer) {
        let width = usize::from(self.area.width);
        if width == 0 {
            return;
        }
        for (index, (&bits, sum)) in self.dots.iter().zip(&self.colors).enumerate() {
            if bits == 0 {
                continue;
            }
            let Some(color) = sum.resolve() else {
                continue;
            };
            let x = self.area.left() + (index % width) as u16;
            let y = self.area.top() + (index / width) as u16;
            write_braille_cell(buffer, (x, y), bits, color);
        }
    }
}

fn write_braille_cell(buffer: &mut Buffer, position: (u16, u16), bits: u8, color: Color) {
    let Some(cell) = buffer.cell_mut(position) else {
        return;
    };
    cell.set_char(BRAILLE[usize::from(bits)]);
    cell.set_fg(color);
}

/// Per-cell color accumulator: `Rgb` dots average in linear space; when
/// a cell only received non-`Rgb` colors, the last one written wins.
#[derive(Clone, Copy, Default)]
struct ColorSum {
    sum: LinearColorSum,
    fallback: Option<Color>,
}

impl ColorSum {
    fn add(&mut self, color: Color) {
        if let Color::Rgb(red, green, blue) = color {
            self.sum.add([red, green, blue]);
        } else {
            self.fallback = Some(color);
        }
    }

    fn resolve(&self) -> Option<Color> {
        match self.sum.resolve_srgb() {
            Some([red, green, blue]) => Some(Color::Rgb(red, green, blue)),
            None => self.fallback,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::{Color, Style};

    use super::BrailleGrid;

    #[test]
    fn dot_positions_map_to_the_braille_bit_layout() {
        let area = Rect::new(0, 0, 3, 1);
        let mut grid = BrailleGrid::new(area);
        grid.set(0, 0, Color::White);
        grid.set(3, 3, Color::White);
        for sub_y in 0..4 {
            grid.set(4, sub_y, Color::White);
            grid.set(5, sub_y, Color::White);
        }
        let mut buffer = Buffer::empty(area);

        grid.resolve_into(&mut buffer);

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "⠁");
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "⢀");
        assert_eq!(buffer.cell((2, 0)).unwrap().symbol(), "⣿");
    }

    #[test]
    fn cell_color_averages_the_set_dots() {
        let area = Rect::new(0, 0, 1, 1);
        let mut grid = BrailleGrid::new(area);
        grid.set(0, 0, Color::Rgb(255, 0, 0));
        grid.set(1, 0, Color::Rgb(0, 0, 255));
        let mut buffer = Buffer::empty(area);

        grid.resolve_into(&mut buffer);

        assert_eq!(buffer.cell((0, 0)).unwrap().fg, Color::Rgb(188, 0, 188));
    }

    #[test]
    fn non_rgb_colors_fall_back_to_last_write() {
        let area = Rect::new(0, 0, 1, 1);
        let mut grid = BrailleGrid::new(area);
        grid.set(0, 0, Color::Green);
        grid.set(1, 0, Color::Magenta);
        let mut buffer = Buffer::empty(area);

        grid.resolve_into(&mut buffer);

        assert_eq!(buffer.cell((0, 0)).unwrap().fg, Color::Magenta);
    }

    #[test]
    fn empty_cells_and_backgrounds_stay_untouched() {
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(0, 0, "  ", Style::new().bg(Color::Green));
        let mut grid = BrailleGrid::new(area);
        grid.set(0, 0, Color::Red);

        grid.resolve_into(&mut buffer);

        let drawn = buffer.cell((0, 0)).unwrap();
        assert_eq!(drawn.symbol(), "⠁");
        assert_eq!(drawn.bg, Color::Green);
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn out_of_area_sets_are_ignored_and_offsets_are_absolute() {
        let area = Rect::new(1, 1, 1, 1);
        let mut grid = BrailleGrid::new(area);
        grid.set(0, 4, Color::Red);
        grid.set(4, 4, Color::Red);
        grid.set(2, 3, Color::Red);
        grid.set(2, 8, Color::Red);
        grid.set(2, 4, Color::Blue);
        let mut buffer = Buffer::empty(area);

        grid.resolve_into(&mut buffer);

        let cell = buffer.cell((1, 1)).unwrap();
        assert_eq!(cell.symbol(), "⠁");
        assert_eq!(cell.fg, Color::Blue);
    }

    #[test]
    fn reset_clears_stale_dots_and_refits_the_area() {
        let mut grid = BrailleGrid::new(Rect::new(0, 0, 1, 1));
        grid.set(0, 0, Color::Red);

        grid.reset(Rect::new(0, 0, 2, 1));

        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        grid.resolve_into(&mut buffer);
        assert!(buffer.content.iter().all(|cell| cell.symbol() == " "));
        grid.set(2, 0, Color::Blue);
        grid.resolve_into(&mut buffer);
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "⠁");
    }
}
