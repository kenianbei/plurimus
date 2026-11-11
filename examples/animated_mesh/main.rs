//! Plays an animation on a skinned glTF model of a fox. Ported from bevy's
//! `examples/animation/animated_mesh.rs` (0.20) onto the terminal 3d
//! pipeline. `t` cycles the pixel-to-cell strategy (halfblocks, shading
//! and ascii luminance, braille), `e` cycles the sobel edge overlay (off,
//! luminance, depth, both), `q` or ctrl-c quits. The camera orbits the
//! fox by itself; space stops it, arrows or a left drag turn it, and the
//! wheel zooms. The first frames take a few seconds while GPU pipelines
//! compile.

mod camera;
mod render_keys;

use std::f32::consts::PI;
use std::time::Duration;

use bevy_animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy_animation::{AnimationPlayer, AnimationPlugin};
use bevy_app::{App, AppExit, ScheduleRunnerPlugin, Startup};
use bevy_asset::{AssetMetaCheck, AssetPlugin, AssetServer, Assets, Handle};
use bevy_camera::Camera3d;
use bevy_color::Color;
use bevy_core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy_ecs::prelude::{Children, Commands, Component, On, Query, Res, ResMut};
use bevy_gltf::extensions::GltfExtensionHandlers;
use bevy_gltf::{GltfAssetLabel, GltfPlugin};
use bevy_light::{CascadeShadowConfigBuilder, DirectionalLight, GlobalAmbientLight};
use bevy_math::{EulerRot, Quat, Vec3};
use bevy_mesh::{Mesh, Mesh3d, Meshable};
use bevy_pbr::{MeshMaterial3d, PbrPlugin, StandardMaterial};
use bevy_shape::Plane3d;
use bevy_transform::components::Transform;
use bevy_world_serialization::{WorldAssetRoot, WorldInstanceReady, WorldSerializationPlugin};
use camera::{ORBIT_TARGET, Orbit};
use plurimus::core::{CellSize, CorePlugin, TerminalCamera};
use plurimus::crossterm::CrosstermPlugin;
use plurimus::render3d::{DepthReadback, Plugin3d, Render3dPlugins, Strategy3d};

const ASSET_ROOT: &str = "examples/animated_mesh/assets";
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const CAMERA_POSITION: Vec3 = Vec3::new(100.0, 100.0, 150.0);

// An example asset that contains a mesh and animation.
const GLTF_PATH: &str = "Fox.glb";

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(FRAME_INTERVAL),
        CrosstermPlugin::default(),
    ));
    add_scene(&mut app);
    app.run()
}

/// Everything but the backend, which the GPU test leaves out.
fn add_scene(app: &mut App) {
    // A terminal cell is about twice as tall as it is wide.
    app.insert_resource(CellSize { x: 1.0, y: 2.0 });
    app.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 2000.,
        ..GlobalAmbientLight::default()
    });
    app.add_plugins((
        AssetPlugin {
            file_path: ASSET_ROOT.into(),
            meta_check: AssetMetaCheck::Never,
            ..AssetPlugin::default()
        },
        Render3dPlugins,
    ));
    // The order lander documents: the glTF handler registry before
    // PbrPlugin, glTF loading after the render stack.
    app.init_resource::<GltfExtensionHandlers>();
    app.add_plugins(PbrPlugin::default());
    app.add_plugins((
        WorldSerializationPlugin,
        GltfPlugin::default(),
        AnimationPlugin,
        Plugin3d,
    ));
    render_keys::add_render_keys(app);
    camera::add_camera_controls(app);
    app.add_systems(Startup, setup_mesh_and_animation);
    app.add_systems(Startup, setup_camera_and_environment);
}

// A component that stores a reference to an animation we want to play. This is
// created when we start loading the mesh (see `setup_mesh_and_animation`) and
// read when the mesh has spawned (see `play_animation_when_ready`).
#[derive(Component)]
struct AnimationToPlay {
    graph_handle: Handle<AnimationGraph>,
    index: AnimationNodeIndex,
}

fn setup_mesh_and_animation(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    // Create an animation graph containing a single animation. We want the "run"
    // animation from our example asset, which has an index of two.
    let (graph, index) = AnimationGraph::from_clip(
        asset_server.load(GltfAssetLabel::Animation(2).from_asset(GLTF_PATH)),
    );

    // Store the animation graph as an asset.
    let graph_handle = graphs.add(graph);

    // Create a component that stores a reference to our animation.
    let animation_to_play = AnimationToPlay {
        graph_handle,
        index,
    };

    // Start loading the asset as a scene and store a reference to it in a
    // WorldAssetRoot component. This component will automatically spawn a scene
    // containing our mesh once it has loaded.
    let mesh_scene =
        WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(GLTF_PATH)));

    // Spawn an entity with our components, and connect it to an observer that
    // will trigger when the scene is loaded and spawned.
    commands
        .spawn((animation_to_play, mesh_scene))
        .observe(play_animation_when_ready);
}

fn play_animation_when_ready(
    scene_ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    animations_to_play: Query<&AnimationToPlay>,
    mut players: Query<&mut AnimationPlayer>,
) {
    // The entity we spawned in `setup_mesh_and_animation` is the trigger's target.
    // Start by finding the AnimationToPlay component we added to that entity.
    if let Ok(animation_to_play) = animations_to_play.get(scene_ready.entity) {
        // The WorldAssetRoot component will have spawned the scene as a hierarchy
        // of entities parented to our entity. Since the asset contained a skinned
        // mesh and animations, it will also have spawned an animation player
        // component. Search our entity's descendants to find the animation player.
        for child in children.iter_descendants(scene_ready.entity) {
            if let Ok(mut player) = players.get_mut(child) {
                // Tell the animation player to start the animation and keep
                // repeating it.
                player.play(animation_to_play.index).repeat();

                // Add the animation graph. This only needs to be done once to
                // connect the animation player to the mesh.
                commands
                    .entity(child)
                    .insert(AnimationGraphHandle(animation_to_play.graph_handle.clone()));
            }
        }
    }
}

// Spawn a camera and a simple environment with a ground plane and light.
fn setup_camera_and_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(CAMERA_POSITION).looking_at(ORBIT_TARGET, Vec3::Y),
        // What the terminal adds: the view fills the terminal, the readback
        // starts as halfblocks, depth is read back for the depth edges, and
        // the camera orbits from upstream's position.
        Orbit::from_offset(CAMERA_POSITION - ORBIT_TARGET),
        TerminalCamera::Fill,
        Strategy3d::default(),
        DepthReadback,
        Tonemapping::None,
        DebandDither::Disabled,
    ));

    // Plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(500_000.0, 500_000.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
    ));

    // Light
    commands.spawn((
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
        DirectionalLight {
            shadow_maps_enabled: true,
            ..DirectionalLight::default()
        },
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 200.0,
            maximum_distance: 400.0,
            ..CascadeShadowConfigBuilder::default()
        }
        .build(),
    ));
}

#[cfg(test)]
mod tests;
