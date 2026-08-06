//! 3d pipeline for plurimus: GPU camera readback converted to terminal
//! cells.
//!
//! [`Render3dPlugins`] assembles a headless bevy render stack, so a scene
//! is built from bevy's own crates rather than through this one. Depend on
//! the ones the scene needs at the same version plurimus uses (bevy 0.19) -
//! cargo resolves both to a single crate:
//!
//! ```toml
//! bevy_asset = "0.19"          # Assets, Handle
//! bevy_camera = "0.19"         # Camera3d
//! bevy_core_pipeline = "0.19"  # Tonemapping
//! bevy_light = "0.19"          # DirectionalLight
//! bevy_math = "0.19"           # primitives
//! bevy_mesh = "0.19"           # Mesh, Mesh3d
//! bevy_pbr = "0.19"            # PbrPlugin, StandardMaterial, custom Materials
//! bevy_transform = "0.19"      # Transform
//! ```
//!
//! [`Render3dPlugins`] stops at the render stack: the app adds its own
//! material system (`PbrPlugin`) and, if it loads models, `bevy_gltf` -
//! see [`Render3dPlugins`] for the order those require.

mod convert;
mod depth;
mod depth_copy;
mod occlude;
mod overlay;
mod pipeline;
mod render_stack;
mod sample;
mod strategy;
mod target;

pub use depth::{DepthFrame, DepthOcclusion, DepthReadback};
pub use overlay::{EdgeCharacters, EdgeOverlay, EdgeSource};
pub use pipeline::FrameDepth;
pub use render_stack::Render3dPlugins;
pub use strategy::{
    DepthRamp, LuminanceRamp, RAMP_ASCII, RAMP_BLOCKS, RAMP_BRAILLE, RAMP_SHADING, Strategy3d,
};
pub use target::{Aspect3d, ReadbackFrame, Supersample};

use bevy_app::{App, Plugin, PostUpdate};
use bevy_core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_render::RenderApp;
use bevy_render::extract_component::ExtractComponentPlugin;
use plurimus_core::{
    RasterizeSystems, TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems,
};

/// Converts GPU camera readback into terminal camera buffers.
///
/// Requires [`plurimus_core::CorePlugin`] and the headless render stack
/// ([`Render3dPlugins`]) to be added first.
pub struct Plugin3d;

impl Plugin for Plugin3d {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (target::sync_render_targets, depth::sync_depth_targets).chain(),
        );
        app.add_observer(target::store_readback);
        app.add_observer(depth::store_depth_readback);
        app.add_plugins(ExtractComponentPlugin::<depth_copy::DepthCopyTarget>::default());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Core3d,
                depth_copy::copy_depth_to_readback.in_set(Core3dSystems::EarlyPostProcess),
            );
        }
        let sub_app = app.sub_app_mut(TerminalRenderApp);
        sub_app.init_resource::<pipeline::ExtractedFrames3d>();
        sub_app.init_resource::<FrameDepth>();
        app.add_extract_systems(pipeline::extract_3d);
        app.add_terminal_systems(
            TerminalRenderSystems::Rasterize,
            (
                pipeline::reset_frame_depth.before(RasterizeSystems::World),
                pipeline::rasterize_3d.in_set(RasterizeSystems::World),
            ),
        );
    }
}
