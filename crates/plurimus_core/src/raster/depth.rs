//! The depth buffer that lets separate cameras occlude each other.
//!
//! A camera buffer alone cannot say whether the cell it drew is in front of
//! what another camera drew there, so 3d keeps a [`DepthGrid`] beside the
//! color grid at the same halfblock resolution and depth-tests every subcell
//! before writing it. Depths follow bevy's reverse-Z convention, 1.0 near and
//! 0.0 far, so a grid resets to all-far and a closer-or-equal sample wins.

use ratatui_core::layout::Rect;

/// Per-subcell reverse-Z depth over a cell area at halfblock resolution
/// (two depths per cell, `sub_y` = `cell_y * 2`). Follows bevy's depth
/// convention: 1.0 at the near plane, 0.0 at the far plane, so higher
/// values are closer.
#[derive(Debug, Default)]
pub struct DepthGrid {
    area: Rect,
    depths: Vec<f32>,
}

impl DepthGrid {
    /// An empty (all-far) grid covering `area`.
    #[must_use]
    pub fn new(area: Rect) -> Self {
        let mut grid = Self::default();
        grid.reset(area);
        grid
    }

    /// Clears every depth to far and refits the grid to `area`,
    /// retaining allocated capacity.
    pub fn reset(&mut self, area: Rect) {
        self.area = area;
        let subcells = area.width as usize * area.height as usize * 2;
        self.depths.clear();
        self.depths.resize(subcells, 0.0);
    }

    /// The recorded depth at a subcell; `None` out of area.
    #[must_use]
    pub fn get(&self, x: u16, sub_y: u16) -> Option<f32> {
        Some(self.depths[self.index(x, sub_y)?])
    }

    /// Depth-tests `depth` against the subcell: a closer-or-equal value
    /// is recorded and wins (`Some(true)` - draw); a farther value
    /// leaves the record and loses (`Some(false)` - occluded). `None`
    /// out of area.
    pub fn compare_and_update(&mut self, x: u16, sub_y: u16, depth: f32) -> Option<bool> {
        let index = self.index(x, sub_y)?;
        if depth >= self.depths[index] {
            self.depths[index] = depth;
            return Some(true);
        }
        Some(false)
    }

    fn index(&self, x: u16, sub_y: u16) -> Option<usize> {
        let column = usize::from(x).checked_sub(usize::from(self.area.left()))?;
        let row = usize::from(sub_y).checked_sub(usize::from(self.area.top()) * 2)?;
        let width = usize::from(self.area.width);
        if column >= width || row >= usize::from(self.area.height) * 2 {
            return None;
        }
        Some(row * width + column)
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::layout::Rect;

    use super::DepthGrid;

    #[test]
    fn closer_or_equal_depths_win_and_record() {
        let mut grid = DepthGrid::new(Rect::new(0, 0, 2, 1));

        assert_eq!(grid.compare_and_update(0, 0, 0.4), Some(true));
        assert_eq!(grid.compare_and_update(0, 0, 0.2), Some(false));
        assert_eq!(grid.compare_and_update(0, 0, 0.4), Some(true));
        assert_eq!(grid.get(0, 0), Some(0.4));
    }

    #[test]
    fn coordinates_are_absolute_and_out_of_area_is_none() {
        let mut grid = DepthGrid::new(Rect::new(2, 1, 1, 1));

        assert_eq!(grid.compare_and_update(2, 2, 0.5), Some(true));
        assert_eq!(grid.get(2, 3), Some(0.0));
        assert_eq!(grid.get(1, 2), None);
        assert_eq!(grid.compare_and_update(2, 4, 0.5), None);
        assert_eq!(grid.get(3, 2), None);
    }

    #[test]
    fn reset_clears_to_far_and_refits() {
        let mut grid = DepthGrid::new(Rect::new(0, 0, 1, 1));
        grid.compare_and_update(0, 0, 0.9);

        grid.reset(Rect::new(0, 0, 2, 1));

        assert_eq!(grid.get(0, 0), Some(0.0));
        assert_eq!(grid.get(1, 1), Some(0.0));
    }
}
