//! World-to-cell projection for 2d camera views.
//!
//! The projection is bevy's: a [`Camera2d`](bevy_camera::Camera2d)'s
//! orthographic [`Projection`](bevy_camera::Projection), fitted to its view by
//! core, maps a world position to terminal pixels through
//! [`Camera::world_to_viewport`], and [`CellSize`](plurimus_core::CellSize)
//! turns pixels into cells - which is where the tall cell's aspect comes from.
//! Projections here are signed and unclipped - a sprite whose anchor sits
//! off-screen can still have visible parts - so the buffer and the subcell
//! grids do the clipping.

use bevy_ecs::prelude::Component;
use bevy_math::{Vec2, Vec3};

use crate::extract::ExtractedView2d;

const CELL: Vec2 = Vec2::ONE;
const HALFBLOCK: Vec2 = Vec2::new(1.0, 2.0);
const BRAILLE: Vec2 = Vec2::new(2.0, 4.0);

/// Subcell rendering mode for a camera's [`Pixel`](crate::Pixel)
/// entities. A per-camera component beside
/// [`Camera2d`](bevy_camera::Camera2d); absent means halfblocks.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum SubcellMode {
    /// 1×2 points per cell, colored foreground and background.
    #[default]
    Halfblock,
    /// 2×4 dots per cell, foreground-only with averaged color.
    Braille,
}

impl ExtractedView2d {
    /// Absolute terminal cell under `world`.
    pub(crate) fn project_cell(&self, world: Vec3) -> Option<(i32, i32)> {
        self.project(world, CELL)
    }

    /// Absolute halfblock subcell under `world`, at twice the rows.
    pub(crate) fn project_halfblock(&self, world: Vec3) -> Option<(i32, i32)> {
        self.project(world, HALFBLOCK)
    }

    /// Absolute braille dot under `world`, at 2×4 per cell.
    pub(crate) fn project_braille(&self, world: Vec3) -> Option<(i32, i32)> {
        self.project(world, BRAILLE)
    }

    // `None` where bevy refuses: past the near or far plane, or a camera
    // core has not sized yet.
    fn project(&self, world: Vec3, per_cell: Vec2) -> Option<(i32, i32)> {
        let pixel = self.camera.world_to_viewport(&self.transform, world).ok()?;
        let point = (pixel / self.cell * per_cell).floor();
        Some((point.x as i32, point.y as i32))
    }
}
