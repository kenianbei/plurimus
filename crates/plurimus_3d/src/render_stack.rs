//! The headless bevy render stack the 3d pipeline runs on.
//!
//! `bevy_render` normally comes with a window, a swapchain and a full plugin
//! set; none of that applies when the output is a texture read back to the
//! CPU. This assembles the smallest stack that still renders - and stops
//! short of materials, so an app adds `PbrPlugin` and its own asset loading
//! rather than having them forced on it.

use bevy_app::{App, Plugin, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_camera::CameraPlugin;
use bevy_core_pipeline::CorePipelinePlugin;
use bevy_diagnostic::FrameCountPlugin;
use bevy_image::ImagePlugin;
use bevy_light::LightPlugin;
use bevy_mesh::MeshPlugin;
use bevy_render::RenderPlugin;
use bevy_time::TimePlugin;
use bevy_transform::TransformPlugin;
use bevy_window::{ExitCondition, WindowPlugin};

/// Headless bevy render stack: everything a 3d scene needs to render to
/// an image with no window or winit, in bevy's canonical plugin order.
///
/// Every plugin is added only if missing, so an app can pre-add any of
/// them with custom configuration - e.g. `AssetPlugin` with a custom
/// `file_path` - before this plugin.
///
/// The material system is deliberately not included: add `PbrPlugin` (or
/// your own material plugins) after this one, which is also where glTF
/// loading goes. `bevy_pbr`'s glTF support panics during `PbrPlugin::build`
/// unless `bevy_gltf`'s `GltfExtensionHandlers` resource already exists,
/// so an app loading glTF initializes that first:
///
/// ```ignore
/// app.add_plugins(Render3dPlugins);
/// app.init_resource::<GltfExtensionHandlers>();
/// app.add_plugins(PbrPlugin::default());
/// app.add_plugins((WorldSerializationPlugin, GltfPlugin::default()));
/// ```
///
/// Pipelined rendering is deliberately omitted - at terminal resolutions
/// there is no GPU work worth overlapping, and skipping it saves a frame
/// of latency. Apps with unusually heavy scenes can still add
/// `PipelinedRenderingPlugin` themselves.
///
/// Requires a working wgpu adapter (hardware or software) at startup.
pub struct Render3dPlugins;

impl Plugin for Render3dPlugins {
    fn build(&self, app: &mut App) {
        add_if_missing(app, TaskPoolPlugin::default());
        add_if_missing(app, FrameCountPlugin);
        // bevy_render's globals extract Time every frame, so the stack
        // carries it the way MinimalPlugins does rather than relying on
        // whatever else the app happened to add.
        add_if_missing(app, TimePlugin);
        add_if_missing(app, TransformPlugin);
        add_if_missing(
            app,
            WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..WindowPlugin::default()
            },
        );
        add_if_missing(app, AssetPlugin::default());
        add_if_missing(app, RenderPlugin::default());
        add_if_missing(app, ImagePlugin::default());
        add_if_missing(app, MeshPlugin);
        add_if_missing(app, CameraPlugin);
        add_if_missing(app, LightPlugin);
        add_if_missing(app, CorePipelinePlugin);
    }
}

fn add_if_missing<P: Plugin>(app: &mut App, plugin: P) {
    if !app.is_plugin_added::<P>() {
        app.add_plugins(plugin);
    }
}
