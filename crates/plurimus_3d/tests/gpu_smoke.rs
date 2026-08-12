//! GPU integration smoke. Needs a working wgpu adapter; run manually:
//! `cargo test -p plurimus_3d --test gpu_smoke -- --ignored`
//!
//! Doubles as the worked example of an app assembling its own material
//! system on top of the render stack.

use bevy_app::{App, PluginsState};
use bevy_asset::Assets;
use bevy_camera::Camera3d;
use bevy_core_pipeline::tonemapping::Tonemapping;
use bevy_gltf::extensions::GltfExtensionHandlers;
use bevy_light::DirectionalLight;
use bevy_math::Vec3;
use bevy_math::primitives::Cuboid;
use bevy_mesh::{Mesh, Mesh3d};
use bevy_pbr::{MeshMaterial3d, PbrPlugin, StandardMaterial};
use bevy_render::view::Msaa;
use bevy_transform::components::Transform;
use plurimus_3d::{DepthFrame, DepthReadback, Plugin3d, Render3dPlugins, Strategy3d};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::composed_frame;

fn wait_for_renderer(app: &mut App) {
    while app.plugins_state() == PluginsState::Adding {
        bevy_tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
}

fn spawn_cube_scene(app: &mut App) {
    let cube = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(4.0, 4.0, 4.0));
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    app.world_mut()
        .spawn((Mesh3d(cube), MeshMaterial3d(material)));
    app.world_mut().spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    app.world_mut().spawn((
        Camera3d::default(),
        Tonemapping::None,
        TerminalCamera::default(),
        Strategy3d::default(),
        Transform::from_xyz(0.0, 4.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[test]
#[ignore = "requires a wgpu adapter"]
fn depth_readback_fills_reverse_z_depths() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, Render3dPlugins));
    // bevy_pbr registers a glTF material handler during build whenever
    // its bevy_gltf feature is on - which any crate in the graph can turn
    // on - so the registry is initialized first either way.
    app.init_resource::<GltfExtensionHandlers>();
    app.add_plugins((PbrPlugin::default(), Plugin3d));
    app.insert_resource(TerminalSize::new(40, 12));
    wait_for_renderer(&mut app);
    spawn_cube_scene(&mut app);
    let camera = {
        let world = app.world_mut();
        let mut cameras = world
            .query_filtered::<bevy_ecs::prelude::Entity, bevy_ecs::prelude::With<Strategy3d>>();
        let camera = cameras.single(world).unwrap();
        world.entity_mut(camera).insert((Msaa::Off, DepthReadback));
        camera
    };

    for _ in 0..10 {
        app.update();
    }

    let frame = app.world().entity(camera).get::<DepthFrame>().unwrap();
    assert!(!frame.depths.is_empty(), "no depths read back");
    assert!(
        frame.depths.iter().all(|d| (0.0..=1.0).contains(d)),
        "depths out of the reverse-z range"
    );
    let center = frame.depths[(frame.size.y / 2 * frame.size.x + frame.size.x / 2) as usize];
    let corner = frame.depths[0];
    assert!(
        center > corner,
        "cube ({center}) should be nearer than empty background ({corner})"
    );
}

#[test]
#[ignore = "requires a wgpu adapter"]
fn renders_a_lit_cube_to_cells() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, Render3dPlugins));
    // bevy_pbr registers a glTF material handler during build whenever
    // its bevy_gltf feature is on - which any crate in the graph can turn
    // on - so the registry is initialized first either way.
    app.init_resource::<GltfExtensionHandlers>();
    app.add_plugins((PbrPlugin::default(), Plugin3d));
    app.insert_resource(TerminalSize::new(40, 12));
    wait_for_renderer(&mut app);
    spawn_cube_scene(&mut app);

    for _ in 0..10 {
        app.update();
    }

    let rendered = composed_frame(&app);
    assert!(
        rendered
            .chars()
            .any(|symbol| symbol != ' ' && symbol != '\n'),
        "no cells rendered from the GPU frame"
    );
}
