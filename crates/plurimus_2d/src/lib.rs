//! 2d pipeline for plurimus: software rasterization of sprite and glyph
//! entities into terminal camera buffers, including subcell modes.
//!
//! There is no GPU here - transforms are projected and cells written
//! directly, which is what lets the pipeline run in a headless test as
//! readily as a terminal. An entity draws as text ([`Glyph`],
//! [`GlyphBlock`]) or as subcell pixels ([`Pixel`], [`PixelBlock`]), and the
//! camera's [`SubcellMode`] decides whether those pixels resolve at halfblock
//! or braille resolution.

mod extract;
mod layers;
mod projection;
mod rasterize;
mod sprite;

pub use layers::RenderLayers;
pub use projection::{Projection2d, SubcellMode};
pub use sprite::{Glyph, GlyphBlock, Pixel, PixelBlock};

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_transform::plugins::TransformPlugin;
use plurimus_core::{
    RasterizeSystems, TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems,
};

/// Rasterizes 2d entities into terminal camera buffers.
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
