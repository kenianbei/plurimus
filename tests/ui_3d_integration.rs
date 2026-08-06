//! Proves the ui pipeline's virtual window entity coexists with the
//! headless render stack. Needs a wgpu adapter; run manually:
//! `cargo test --test ui_3d_integration --features "ui,3d" -- --ignored`

#![cfg(all(feature = "ui", feature = "3d"))]

use std::time::Duration;

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
use bevy_time::TimeUpdateStrategy;
use bevy_transform::components::Transform;
use plurimus::core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus::render3d::{Plugin3d, Render3dPlugins, Strategy3d};
use plurimus::ui::UiPlugin;
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
fn ui_and_3d_render_together() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin, Render3dPlugins));
    // The app assembles its own material system; glTF support in bevy_pbr
    // reads this registry during PbrPlugin's build, so it comes first.
    app.init_resource::<GltfExtensionHandlers>();
    app.add_plugins((PbrPlugin::default(), Plugin3d));
    app.insert_resource(TerminalSize { cols: 40, rows: 12 });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        16,
    )));
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
        "no cells rendered with ui + 3d together"
    );
}
