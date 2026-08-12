//! Proves the glTF recipe an app assembles for itself, now that the
//! render stack carries neither the material system nor glTF loading.
//! Needs a wgpu adapter; run with `-- --ignored`.

#![cfg(all(feature = "3d", feature = "widgets"))]

use bevy_app::{App, PluginsState};
use bevy_asset::{AssetPlugin, AssetServer, Assets};
use bevy_ecs::prelude::{Entity, With};
use bevy_gltf::extensions::GltfExtensionHandlers;
use bevy_gltf::{GltfAssetLabel, GltfPlugin};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_pbr::PbrPlugin;
use bevy_world_serialization::{WorldAssetRoot, WorldSerializationPlugin};
use plurimus::core::{CorePlugin, TerminalSize};
use plurimus::render3d::{Plugin3d, Render3dPlugins};

const ASSET_ROOT: &str = "examples/lander/assets";
const MODEL: &str = "moonlander.glb";
const MAX_TICKS: usize = 600;

fn gltf_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        CorePlugin,
        AssetPlugin {
            file_path: ASSET_ROOT.into(),
            ..AssetPlugin::default()
        },
        Render3dPlugins,
    ));
    // The recipe: the registry PbrPlugin's build reads, then the material
    // system, then glTF loading after the render stack.
    app.init_resource::<GltfExtensionHandlers>();
    app.add_plugins(PbrPlugin::default());
    app.add_plugins((WorldSerializationPlugin, GltfPlugin::default(), Plugin3d));
    app.insert_resource(TerminalSize::new(40, 12));
    while app.plugins_state() == PluginsState::Adding {
        bevy_tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
    app
}

#[test]
#[ignore = "requires a wgpu adapter"]
fn an_app_wired_gltf_scene_loads_and_spawns_meshes() {
    let mut app = gltf_app();
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load(GltfAssetLabel::Scene(0).from_asset(MODEL));
    app.world_mut().spawn(WorldAssetRoot(handle));

    let mut meshes = 0;
    for _ in 0..MAX_TICKS {
        app.update();
        meshes = app
            .world_mut()
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(app.world())
            .count();
        if meshes > 0 {
            break;
        }
    }

    assert!(
        meshes > 0,
        "the glTF scene never spawned meshes; the app-side wiring is broken"
    );
    assert!(
        !app.world().resource::<Assets<Mesh>>().is_empty(),
        "mesh assets never loaded"
    );
}
