use bevy_app::{App, Startup, Update};
use bevy_asset::{Assets, Handle};
use bevy_camera::visibility::Visibility;
use bevy_color::Color;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{
    Commands, Component, Entity, IntoScheduleConfigs, Query, Res, ResMut, Resource, With,
    any_with_component, resource_changed,
};
use bevy_math::{FloatExt, Vec3, primitives::Sphere};
use bevy_mesh::{Mesh, Mesh3d};
use bevy_pbr::{MeshMaterial3d, StandardMaterial};
use bevy_time::{Time, Timer, TimerMode};
use bevy_transform::components::Transform;

use crate::game::{Lander, Phase};

const EXPLOSION_SECONDS: f32 = 0.6;
const START_SCALE: f32 = 0.5;
const END_SCALE: f32 = 4.0;
const FIREBALL_RADIUS: f32 = 1.0;

#[derive(Resource)]
struct ExplosionAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Explosion(Timer);

pub fn add_effects(app: &mut App) {
    app.add_systems(Startup, create_explosion_assets);
    app.add_systems(
        Update,
        (
            react_to_phase.run_if(resource_changed::<Phase>),
            animate_explosions.run_if(any_with_component::<Explosion>),
        ),
    );
}

fn create_explosion_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Sphere::new(FIREBALL_RADIUS));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.45, 0.1),
        emissive: Color::srgb(8.0, 2.5, 0.4).into(),
        ..StandardMaterial::default()
    });
    commands.insert_resource(ExplosionAssets { mesh, material });
}

fn react_to_phase(
    phase: Res<Phase>,
    fireball: Res<ExplosionAssets>,
    landers: Query<(Entity, &Transform), With<Lander>>,
    mut commands: Commands,
) {
    if phase.is_added() {
        return;
    }
    let Ok((lander, transform)) = landers.single() else {
        return;
    };
    match *phase {
        Phase::Crashed => {
            commands.entity(lander).insert(Visibility::Hidden);
            commands.spawn((
                Explosion(Timer::from_seconds(EXPLOSION_SECONDS, TimerMode::Once)),
                Mesh3d(fireball.mesh.clone()),
                MeshMaterial3d(fireball.material.clone()),
                Transform::from_translation(transform.translation)
                    .with_scale(Vec3::splat(START_SCALE)),
            ));
        }
        Phase::Flying => {
            commands.entity(lander).insert(Visibility::Inherited);
        }
        Phase::Landed => {}
    }
}

fn animate_explosions(
    time: Res<Time>,
    mut explosions: Query<(Entity, &mut Explosion, &mut Transform)>,
    mut commands: Commands,
) {
    for (entity, mut explosion, mut transform) in explosions.iter_mut() {
        if explosion.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = Vec3::splat(START_SCALE.lerp(END_SCALE, explosion.0.fraction()));
    }
}
