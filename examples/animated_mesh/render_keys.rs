use bevy_app::{App, AppExit, Update};
use bevy_ecs::prelude::{Commands, Entity, MessageReader, MessageWriter, Query};
use plurimus::render3d::{EdgeOverlay, EdgeSource, LuminanceRamp, RAMP_SHADING, Strategy3d};
use plurimus::term::HeldModifiers;
use plurimus::term::bevy_input::ButtonState;
use plurimus::term::bevy_input::keyboard::{Key, KeyboardInput};

/// Upstream's daylit scene sits high in linear space, where the ramp's
/// default boost for dark scenes would saturate every cell.
pub const LUMINANCE_SCALE: f32 = 0.8;

pub fn add_render_keys(app: &mut App) {
    app.add_systems(Update, (quit_on_key, handle_render_keys));
}

fn quit_on_key(
    mut keys: MessageReader<KeyboardInput>,
    held: HeldModifiers,
    mut exit: MessageWriter<AppExit>,
) {
    for key in keys.read() {
        let ctrl_c = held.get().ctrl && key.logical_key == Key::Character("c".into());
        if key.state == ButtonState::Pressed
            && (key.logical_key == Key::Character("q".into()) || ctrl_c)
        {
            exit.write(AppExit::Success);
        }
    }
}

fn handle_render_keys(
    mut keys: MessageReader<KeyboardInput>,
    mut cameras: Query<(Entity, &mut Strategy3d, Option<&EdgeOverlay>)>,
    mut commands: Commands,
) {
    for key in keys.read() {
        if key.state == ButtonState::Released {
            continue;
        }
        if key.logical_key == Key::Character("t".into()) {
            for (_, mut strategy, _) in &mut cameras {
                *strategy = next_strategy(*strategy);
            }
        } else if key.logical_key == Key::Character("e".into()) {
            toggle_edges(&cameras, &mut commands);
        }
    }
}

// Luminance edges only: depth edges draw nothing at this scene's scale.
fn toggle_edges(
    cameras: &Query<(Entity, &mut Strategy3d, Option<&EdgeOverlay>)>,
    commands: &mut Commands,
) {
    for (camera, _, overlay) in cameras {
        if overlay.is_some() {
            commands.entity(camera).remove::<EdgeOverlay>();
        } else {
            commands
                .entity(camera)
                .insert(EdgeOverlay::default().with_source(EdgeSource::Luminance));
        }
    }
}

fn next_strategy(strategy: Strategy3d) -> Strategy3d {
    match strategy {
        Strategy3d::Halfblocks => {
            Strategy3d::Luminance(LuminanceRamp::new(RAMP_SHADING).with_scale(LUMINANCE_SCALE))
        }
        Strategy3d::Luminance(ramp) if ramp.characters == RAMP_SHADING => {
            Strategy3d::Luminance(LuminanceRamp::default().with_scale(LUMINANCE_SCALE))
        }
        Strategy3d::Luminance(_) => Strategy3d::Braille,
        _ => Strategy3d::Halfblocks,
    }
}
