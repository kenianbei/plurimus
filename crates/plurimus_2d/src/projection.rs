//! World-to-cell projection for 2d camera views.
//!
//! Terminal cells are taller than they are wide, so a square in world space
//! is not a square on screen; [`Projection2d`] carries the cell aspect that
//! corrects for it alongside the zoom. Projections here are unclipped and
//! signed - a sprite whose anchor sits off-screen can still have visible
//! parts - so callers and the subcell grids do the clipping.

use bevy_ecs::prelude::Component;
use bevy_math::Vec2;
use bevy_transform::components::Transform;
use ratatui_core::layout::Rect;

/// Marks a terminal camera as a 2d view and sets its world-to-cell mapping.
///
/// The camera entity's `GlobalTransform` is the view center, mapped to the
/// middle of the viewport. World space is Y-up.
#[derive(Component, Debug, Clone, Copy)]
#[require(Transform)]
#[non_exhaustive]
pub struct Projection2d {
    /// World units per cell column; larger values zoom out.
    pub scale: f32,
    /// Cell height in units of cell width; terminal cells are tall, so a
    /// square world shape looks square on screen at the default.
    pub cell_aspect: f32,
}

impl Projection2d {
    /// Sets world units per cell column.
    #[must_use]
    pub const fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

impl Default for Projection2d {
    fn default() -> Self {
        Self {
            scale: 1.0,
            cell_aspect: 2.0,
        }
    }
}

/// Subcell rendering mode for a camera's [`Pixel`](crate::Pixel)
/// entities. A per-camera component beside [`Projection2d`]; absent
/// means halfblocks.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum SubcellMode {
    /// 1×2 points per cell, colored foreground and background.
    #[default]
    Halfblock,
    /// 2×4 dots per cell, foreground-only with averaged color.
    Braille,
}

/// One camera's resolved projection: world positions to screen cells.
#[derive(Debug, Clone, Copy)]
pub(crate) struct View2d {
    /// World position mapped to the middle of the viewport.
    pub center: Vec2,
    /// Zoom and cell-aspect settings for the mapping.
    pub projection: Projection2d,
    /// Screen-space cell region the view renders into.
    pub viewport: Rect,
}

impl View2d {
    /// Projects a world position to signed absolute halfblock subcell
    /// coordinates `(x, sub_y)` at twice the vertical resolution.
    /// Unclipped: subcell grids ignore out-of-area writes.
    #[must_use]
    pub(crate) fn project_subcell_raw(&self, world: Vec2) -> (i32, i32) {
        (
            i32::from(self.viewport.x) + self.column_of(world.x).floor() as i32,
            i32::from(self.viewport.y) * 2 + (self.row_of(world.y) * 2.0).floor() as i32,
        )
    }

    /// Projects a world position to signed absolute braille subcell
    /// coordinates `(sub_x, sub_y)` at 2×4 dot resolution. Unclipped:
    /// subcell grids ignore out-of-area writes.
    #[must_use]
    pub(crate) fn project_braille_raw(&self, world: Vec2) -> (i32, i32) {
        (
            i32::from(self.viewport.x) * 2 + (self.column_of(world.x) * 2.0).floor() as i32,
            i32::from(self.viewport.y) * 4 + (self.row_of(world.y) * 4.0).floor() as i32,
        )
    }

    /// Projects a world position to signed viewport-relative cell
    /// coordinates without clipping - for multi-cell shapes whose anchor
    /// may sit off-viewport while parts remain visible; callers
    /// bounds-check per cell.
    #[must_use]
    pub(crate) fn project_cell_raw(&self, world: Vec2) -> (i32, i32) {
        (
            self.column_of(world.x).floor() as i32,
            self.row_of(world.y).floor() as i32,
        )
    }

    fn column_of(&self, world_x: f32) -> f32 {
        f32::from(self.viewport.width) / 2.0 + (world_x - self.center.x) / self.projection.scale
    }

    fn row_of(&self, world_y: f32) -> f32 {
        f32::from(self.viewport.height) / 2.0
            - (world_y - self.center.y) / (self.projection.scale * self.projection.cell_aspect)
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::Vec2;
    use ratatui_core::layout::{Position, Rect};

    use super::{Projection2d, View2d};

    // Clipped cell-level projection exists only as the vehicle for the
    // projection-math tests; production paths project unclipped.
    impl View2d {
        fn project_cell(&self, world: Vec2) -> Option<Position> {
            let (column, row) = self.project_cell_raw(world);
            let column = u16::try_from(column).ok()?;
            let row = u16::try_from(row).ok()?;
            (column < self.viewport.width && row < self.viewport.height)
                .then(|| Position::new(self.viewport.x + column, self.viewport.y + row))
        }
    }

    fn view(center: Vec2, scale: f32, viewport: Rect) -> View2d {
        View2d {
            center,
            projection: Projection2d {
                scale,
                ..Projection2d::default()
            },
            viewport,
        }
    }

    #[test]
    fn camera_center_projects_to_viewport_middle() {
        let view = view(Vec2::new(10.0, 5.0), 1.0, Rect::new(2, 1, 8, 4));
        assert_eq!(
            view.project_cell(Vec2::new(10.0, 5.0)),
            Some(Position::new(6, 3))
        );
    }

    #[test]
    fn y_up_and_cell_aspect_shape_the_mapping() {
        let view = view(Vec2::ZERO, 1.0, Rect::new(0, 0, 8, 8));
        let center = view.project_cell(Vec2::ZERO).unwrap();
        let up_two = view.project_cell(Vec2::new(0.0, 2.0)).unwrap();
        let right_two = view.project_cell(Vec2::new(2.0, 0.0)).unwrap();
        assert_eq!(up_two.y, center.y - 1);
        assert_eq!(right_two.x, center.x + 2);
    }

    #[test]
    fn scale_zooms_out() {
        let view = view(Vec2::ZERO, 2.0, Rect::new(0, 0, 8, 8));
        let center = view.project_cell(Vec2::ZERO).unwrap();
        let right_two = view.project_cell(Vec2::new(2.0, 0.0)).unwrap();
        assert_eq!(right_two.x, center.x + 1);
    }

    #[test]
    fn off_viewport_positions_clip() {
        let view = view(Vec2::ZERO, 1.0, Rect::new(0, 0, 8, 4));
        assert_eq!(view.project_cell(Vec2::new(-5.0, 0.0)), None);
        assert_eq!(view.project_cell(Vec2::new(4.0, 0.0)), None);
        assert_eq!(view.project_cell(Vec2::new(0.0, 5.0)), None);
    }

    #[test]
    fn braille_projection_doubles_columns_and_quadruples_rows() {
        let view = view(Vec2::ZERO, 1.0, Rect::new(0, 0, 4, 4));
        let (center_x, center_y) = view.project_braille_raw(Vec2::ZERO);
        let (right_x, _) = view.project_braille_raw(Vec2::new(1.0, 0.0));
        let (_, up_y) = view.project_braille_raw(Vec2::new(0.0, 1.0));
        assert_eq!((center_x, center_y), (4, 8));
        assert_eq!(right_x, 6);
        assert_eq!(up_y, 6);
    }

    #[test]
    fn raw_projections_return_signed_off_viewport_coordinates() {
        let origin = view(Vec2::ZERO, 1.0, Rect::new(0, 0, 4, 4));
        assert_eq!(origin.project_cell_raw(Vec2::ZERO), (2, 2));
        assert_eq!(origin.project_cell_raw(Vec2::new(-5.0, 8.0)), (-3, -2));
        assert_eq!(origin.project_subcell_raw(Vec2::new(0.0, -12.0)), (2, 16));
    }

    #[test]
    fn subcell_projections_are_absolute() {
        let offset = view(Vec2::ZERO, 1.0, Rect::new(3, 2, 4, 4));
        assert_eq!(offset.project_subcell_raw(Vec2::ZERO), (5, 8));
        assert_eq!(offset.project_braille_raw(Vec2::ZERO), (10, 16));
    }

    #[test]
    fn subcell_resolves_upper_and_lower_halves() {
        let view = view(Vec2::ZERO, 1.0, Rect::new(0, 0, 4, 4));
        let (_, at_center) = view.project_subcell_raw(Vec2::ZERO);
        let (_, up_half) = view.project_subcell_raw(Vec2::new(0.0, 1.0));
        assert_eq!(at_center, 4);
        assert_eq!(up_half, 3);
    }
}
