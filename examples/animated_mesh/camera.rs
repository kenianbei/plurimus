//! Orbiting the fox: arrows turn the camera, a left drag turns it with the
//! pointer, the wheel zooms, and space toggles a slow automatic orbit.

use bevy_app::{App, Update};
use bevy_ecs::prelude::{
    Changed, Component, IntoScheduleConfigs, Local, MessageReader, Query, Res,
};
use bevy_math::{Vec2, Vec3};
use bevy_time::Time;
use bevy_transform::components::Transform;
use bevy_window::CursorMoved;
use plurimus::core::CellSize;
use plurimus::term::bevy_input::ButtonInput;
use plurimus::term::bevy_input::keyboard::KeyCode;
use plurimus::term::bevy_input::mouse::{AccumulatedMouseScroll, MouseButton, MouseScrollUnit};

/// The point upstream's camera looks at, about the fox's chest.
pub const ORBIT_TARGET: Vec3 = Vec3::new(0.0, 20.0, 0.0);
const KEY_RADIANS_PER_SECOND: f32 = 1.5;
const AUTO_RADIANS_PER_SECOND: f32 = 0.3;
const DRAG_RADIANS_PER_PIXEL: f32 = 0.05;
const ZOOM_PER_NOTCH: f32 = 1.1;
const MIN_DISTANCE: f32 = 60.0;
const MAX_DISTANCE: f32 = 600.0;
/// Just above the ground plane, whose underside draws nothing.
const MIN_PITCH: f32 = 0.05;
/// Short of the pole, where looking at the target loses its up vector.
const MAX_PITCH: f32 = 1.45;

/// The camera's place on a sphere about [`ORBIT_TARGET`].
#[derive(Component, Debug, Clone, Copy)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub is_auto: bool,
}

impl Orbit {
    /// The orbit placing the camera at `ORBIT_TARGET + offset`, turning by
    /// itself.
    pub fn from_offset(offset: Vec3) -> Self {
        let distance = offset.length();
        Self {
            yaw: offset.x.atan2(offset.z),
            pitch: (offset.y / distance).asin(),
            distance,
            is_auto: true,
        }
    }

    fn offset(&self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        Vec3::new(pitch_cos * yaw_sin, pitch_sin, pitch_cos * yaw_cos) * self.distance
    }
}

pub fn add_camera_controls(app: &mut App) {
    app.add_systems(
        Update,
        (
            (
                orbit_with_keys,
                orbit_with_drag,
                zoom_with_wheel,
                auto_orbit,
            ),
            apply_orbit,
        )
            .chain(),
    );
}

fn orbit_with_keys(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut orbits: Query<&mut Orbit>,
) {
    let axis =
        |positive, negative| f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative));
    let turn = Vec2::new(
        axis(KeyCode::ArrowRight, KeyCode::ArrowLeft),
        axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
    );
    if turn == Vec2::ZERO {
        return;
    }
    let step = turn * KEY_RADIANS_PER_SECOND * time.delta_secs();
    for mut orbit in &mut orbits {
        orbit.yaw += step.x;
        orbit.pitch += step.y;
    }
}

fn orbit_with_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    mut moves: MessageReader<CursorMoved>,
    mut last_position: Local<Option<Vec2>>,
    mut orbits: Query<&mut Orbit>,
) {
    let mut dragged = Vec2::ZERO;
    for moved in moves.read() {
        if let Some(last) = *last_position
            && buttons.pressed(MouseButton::Left)
        {
            dragged += moved.position - last;
        }
        *last_position = Some(moved.position);
    }
    if dragged == Vec2::ZERO {
        return;
    }
    for mut orbit in &mut orbits {
        orbit.yaw -= dragged.x * DRAG_RADIANS_PER_PIXEL;
        orbit.pitch += dragged.y * DRAG_RADIANS_PER_PIXEL;
    }
}

fn zoom_with_wheel(
    cell: Res<CellSize>,
    scroll: Res<AccumulatedMouseScroll>,
    mut orbits: Query<&mut Orbit>,
) {
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / cell.y,
    };
    if notches == 0.0 {
        return;
    }
    for mut orbit in &mut orbits {
        orbit.distance /= ZOOM_PER_NOTCH.powf(notches);
    }
}

fn auto_orbit(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut orbits: Query<&mut Orbit>) {
    let is_toggled = keys.just_pressed(KeyCode::Space);
    for mut orbit in &mut orbits {
        if is_toggled {
            orbit.is_auto = !orbit.is_auto;
        }
        if orbit.is_auto {
            orbit.yaw += AUTO_RADIANS_PER_SECOND * time.delta_secs();
        }
    }
}

fn apply_orbit(mut cameras: Query<(&mut Orbit, &mut Transform), Changed<Orbit>>) {
    for (mut orbit, mut transform) in &mut cameras {
        orbit.pitch = orbit.pitch.clamp(MIN_PITCH, MAX_PITCH);
        orbit.distance = orbit.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
        *transform = Transform::from_translation(ORBIT_TARGET + orbit.offset())
            .looking_at(ORBIT_TARGET, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy_ecs::prelude::Entity;
    use bevy_time::TimeUpdateStrategy;
    use plurimus::core::{CorePlugin, TerminalSize};
    use plurimus::term::TermPlugin;
    use plurimus::term::bevy_input::keyboard::Key;
    use plurimus_test::{character, move_to, press_at, scroll_at, write_key};

    use super::*;

    const STEP: Duration = Duration::from_millis(50);
    const START: Orbit = Orbit {
        yaw: 0.5,
        pitch: 0.5,
        distance: 200.0,
        is_auto: false,
    };

    fn orbit_app(orbit: Orbit) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((CorePlugin, TermPlugin));
        app.insert_resource(TerminalSize::new(80, 24));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
        add_camera_controls(&mut app);
        let camera = app.world_mut().spawn((orbit, Transform::default())).id();
        app.update();
        (app, camera)
    }

    fn orbit(app: &App, camera: Entity) -> Orbit {
        *app.world().entity(camera).get::<Orbit>().unwrap()
    }

    #[test]
    fn from_offset_places_the_camera_at_the_offset() {
        let offset = Vec3::new(100.0, 80.0, 150.0);
        let (app, camera) = orbit_app(Orbit {
            is_auto: false,
            ..Orbit::from_offset(offset)
        });

        let transform = app.world().entity(camera).get::<Transform>().unwrap();

        assert!(transform.translation.distance(ORBIT_TARGET + offset) < 1e-3);
    }

    #[test]
    fn a_held_arrow_turns_the_camera() {
        let (mut app, camera) = orbit_app(START);

        write_key(&mut app, Key::ArrowLeft);
        app.update();
        app.update();

        assert!(orbit(&app, camera).yaw < START.yaw);
    }

    #[test]
    fn a_left_drag_turns_the_camera_and_a_bare_move_does_not() {
        let (mut app, camera) = orbit_app(START);
        move_to(&mut app, 10, 10);
        move_to(&mut app, 20, 10);
        assert_eq!(orbit(&app, camera).yaw, START.yaw);

        press_at(&mut app, 20, 10);
        move_to(&mut app, 30, 10);
        app.update();

        assert!(orbit(&app, camera).yaw < START.yaw);
    }

    #[test]
    fn the_wheel_zooms_in_and_out() {
        let (mut app, camera) = orbit_app(START);

        scroll_at(&mut app, 10, 10, -1);
        let closer = orbit(&app, camera).distance;
        scroll_at(&mut app, 10, 10, 2);

        assert!(closer < START.distance);
        assert!(orbit(&app, camera).distance > START.distance);
    }

    #[test]
    fn space_toggles_an_orbit_that_turns_by_itself() {
        let (mut app, camera) = orbit_app(START);

        write_key(&mut app, character(' '));
        app.update();
        app.update();
        let turned = orbit(&app, camera);

        assert!(turned.is_auto);
        assert!(turned.yaw > START.yaw);
    }

    #[test]
    fn pitch_and_distance_hold_their_bounds() {
        let (app, camera) = orbit_app(Orbit {
            pitch: 3.0,
            distance: 1.0,
            ..START
        });

        let clamped = orbit(&app, camera);

        assert_eq!(clamped.pitch, MAX_PITCH);
        assert_eq!(clamped.distance, MIN_DISTANCE);
    }
}
