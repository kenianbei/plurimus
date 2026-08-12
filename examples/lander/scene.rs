use bevy_asset::{AssetServer, Assets};
use bevy_camera::{Camera, Camera3d, ClearColorConfig, PerspectiveProjection, Projection};
use bevy_color::Color;
use bevy_core_pipeline::tonemapping::Tonemapping;
use bevy_ecs::prelude::{Commands, Res, ResMut};
use bevy_gltf::GltfAssetLabel;
use bevy_light::{CascadeShadowConfigBuilder, DirectionalLight, GlobalAmbientLight};
use bevy_math::primitives::Sphere;
use bevy_math::{Quat, Vec3};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_transform::components::Transform;
use bevy_world_serialization::WorldAssetRoot;
use plurimus::core::{TerminalCamera, Viewport};
use plurimus::render3d::{DepthReadback, Strategy3d};

use crate::game::{Lander, PAD_CENTER_X, START_POSITION};

const AMBIENT_BRIGHTNESS: f32 = 150.0;
const SHADOW_DISTANCE: f32 = 80.0;
const LANDER_MODEL: &str = "moonlander.glb";
const SURFACE_MODEL: &str = "moonsurface.glb";
const BASE_MODEL: &str = "moonbase.glb";
const MODEL_SCALE: f32 = 1.5;
const SURFACE_SCALE: Vec3 = Vec3::new(2.0, 1.0, 2.0);
const CAMERA_FOV_DEGREES: f32 = 25.0;
const CAMERA_POSITION: Vec3 = Vec3::new(0.0, 12.0, 60.0);
const CAMERA_TARGET: Vec3 = Vec3::new(0.0, 9.0, 0.0);
const STAR_COUNT: usize = 360;
const STAR_RADIUS: f32 = 0.1;
const STAR_DEPTH: f32 = -90.0;
const STAR_SPREAD_X: f32 = 64.0;
const STAR_MIN_Y: f32 = -8.0;
const STAR_SPAN_Y: f32 = 77.0;
const HASH_MULTIPLY_A: u32 = 0x9E37_79B9;
const HASH_MULTIPLY_B: u32 = 0xC2B2_AE35;
const HASH_OFFSET: u32 = 0x85EB_CA6B;
const HASH_SHIFT: u32 = 16;
const HASH_UNIT_BITS: u32 = 24;
const EARTH_MODEL: &str = "earth.glb";
const EARTH_POSITION: Vec3 = Vec3::new(-20.0, -3.0, -70.0);
const EARTH_RADIUS: f32 = 5.0;
const EARTH_TILT_RADIANS: f32 = -0.4;

pub fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    spawn_camera(&mut commands);
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..DirectionalLight::default()
        },
        CascadeShadowConfigBuilder {
            maximum_distance: SHADOW_DISTANCE,
            ..CascadeShadowConfigBuilder::default()
        }
        .build(),
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: AMBIENT_BRIGHTNESS,
        ..GlobalAmbientLight::default()
    });
    spawn_sky(&mut commands, &mut meshes, &mut materials, &asset_server);
    spawn_terrain(&mut commands, &asset_server);
    spawn_lander(&mut commands, &asset_server);
}

fn spawn_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..Camera::default()
        },
        Tonemapping::None,
        Projection::Perspective(PerspectiveProjection {
            fov: CAMERA_FOV_DEGREES.to_radians(),
            ..PerspectiveProjection::default()
        }),
        TerminalCamera::default().with_viewport(Viewport::Fill),
        Strategy3d::default(),
        // Feeds the depth-sourced edge overlay modes.
        DepthReadback,
        Transform::from_translation(CAMERA_POSITION).looking_at(CAMERA_TARGET, Vec3::Y),
    ));
}

fn spawn_sky(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) {
    let star_mesh = meshes.add(Sphere::new(STAR_RADIUS));
    let star_material = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: Color::srgb(3.0, 3.0, 2.7).into(),
        ..StandardMaterial::default()
    });
    for index in 0..STAR_COUNT {
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_material.clone()),
            Transform::from_translation(star_position(index as u32)),
        ));
    }
    let earth = asset_server.load(GltfAssetLabel::Scene(0).from_asset(EARTH_MODEL));
    commands.spawn((
        WorldAssetRoot(earth),
        Transform::from_translation(EARTH_POSITION)
            .with_scale(Vec3::splat(EARTH_RADIUS))
            .with_rotation(Quat::from_rotation_z(EARTH_TILT_RADIANS)),
    ));
}

fn star_position(index: u32) -> Vec3 {
    let scatter_x = hash_unit(index * 2);
    let scatter_y = hash_unit(index * 2 + 1);
    Vec3::new(
        STAR_SPREAD_X * (scatter_x * 2.0 - 1.0),
        STAR_MIN_Y + STAR_SPAN_Y * scatter_y,
        STAR_DEPTH,
    )
}

fn hash_unit(seed: u32) -> f32 {
    let mut state = seed.wrapping_mul(HASH_MULTIPLY_A).wrapping_add(HASH_OFFSET);
    state ^= state >> HASH_SHIFT;
    state = state.wrapping_mul(HASH_MULTIPLY_B);
    state ^= state >> HASH_SHIFT;
    (state >> (u32::BITS - HASH_UNIT_BITS)) as f32 / (1u32 << HASH_UNIT_BITS) as f32
}

fn spawn_lander(commands: &mut Commands, asset_server: &AssetServer) {
    let model = asset_server.load(GltfAssetLabel::Scene(0).from_asset(LANDER_MODEL));
    commands.spawn((
        Lander::default(),
        WorldAssetRoot(model),
        Transform::from_translation(START_POSITION.extend(0.0))
            .with_scale(Vec3::splat(MODEL_SCALE)),
    ));
}

fn spawn_terrain(commands: &mut Commands, asset_server: &AssetServer) {
    let surface = asset_server.load(GltfAssetLabel::Scene(0).from_asset(SURFACE_MODEL));
    commands.spawn((
        WorldAssetRoot(surface),
        Transform::from_scale(SURFACE_SCALE),
    ));
    let base = asset_server.load(GltfAssetLabel::Scene(0).from_asset(BASE_MODEL));
    commands.spawn((
        WorldAssetRoot(base),
        Transform::from_xyz(PAD_CENTER_X, 0.0, 0.0),
    ));
}
