//! 2d pipeline for plurimus: software rasterization of sprite and glyph
//! entities into terminal camera buffers, including subcell modes.
//!
//! There is no GPU here - transforms are projected and cells written
//! directly, which is what lets the pipeline run in a headless test as
//! readily as a terminal. An entity draws as text ([`Glyph`],
//! [`GlyphBlock`]) or as subcell pixels ([`Pixel`], [`PixelBlock`]), and the
//! camera's [`SubcellMode`] decides whether those pixels resolve at halfblock
//! or braille resolution.
//!
//! A 2d view is a terminal camera carrying bevy's [`Camera2d`], projected
//! through its [`OrthographicProjection`](bevy_camera::OrthographicProjection):
//! the camera's translation is the view's centre, world space is Y-up, and
//! `scale` is world units per pixel. A cell is [`CellSize`] pixels, so an app
//! drawing 2d sets `CellSize { x: 1.0, y: 2.0 }` for a world unit to be a
//! column wide and half a row tall - a square drawn square on a terminal's
//! tall cells. [`RenderLayers`] is bevy's mask, an absent one being layer 0.
//!
//! [`Camera2d`]: bevy_camera::Camera2d
//! [`CellSize`]: plurimus_core::CellSize

mod extract;
mod projection;
mod rasterize;
mod sprite;

pub use bevy_camera::visibility::RenderLayers;
pub use projection::SubcellMode;
pub use sprite::{Glyph, GlyphBlock, Pixel, PixelBlock};

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_transform::plugins::TransformPlugin;
use plurimus_core::{
    RasterizeSystems, TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems,
};

/// Rasterizes 2d entities into terminal camera buffers, for every terminal
/// camera carrying [`Camera2d`](bevy_camera::Camera2d).
///
/// Requires [`plurimus_core::CorePlugin`] to be added first.
pub struct Plugin2d;

impl Plugin for Plugin2d {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TransformPlugin>() {
            app.add_plugins(TransformPlugin);
        }
        app.sub_app_mut(TerminalRenderApp)
            .init_resource::<extract::ExtractedViews2d>()
            .init_resource::<extract::ExtractedSprites2d>();
        app.add_extract_systems((extract::extract_views_2d, extract::extract_sprites_2d));
        app.add_terminal_systems(
            TerminalRenderSystems::Rasterize,
            rasterize::rasterize_2d.in_set(RasterizeSystems::World),
        );
    }
}
