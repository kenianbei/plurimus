//! Keys: steering the rat, restarting a round, and leaving.

use bevy_app::AppExit;
use bevy_ecs::prelude::{Commands, Entity, MessageReader, MessageWriter, Query, ResMut, With};
use bevy_ecs::system::SystemParam;
use plurimus::term::HeldModifiers;
use plurimus::term::bevy_input::ButtonState;
use plurimus::term::bevy_input::keyboard::{Key, KeyboardInput};

use crate::actor::{Actor, Dir};
use crate::game::{LevelPending, Phase, Player, Round, STARTING_LIVES, Spawned};
use crate::ghosts::Ghosts;

pub fn handle_keys(
    mut keys: MessageReader<KeyboardInput>,
    held: HeldModifiers,
    mut exit: MessageWriter<AppExit>,
    mut players: Query<&mut Actor, With<Player>>,
    mut restart: Restart,
) {
    for key in keys.read() {
        if key.state == ButtonState::Released {
            continue;
        }
        let ctrl_c = held.get().ctrl && key.logical_key == Key::Character("c".into());
        if key.logical_key == Key::Character("q".into()) || ctrl_c {
            exit.write(AppExit::Success);
        }
        if key.logical_key == Key::Character("r".into()) {
            restart.run();
        }
        if let Some(direction) = steer(&key.logical_key) {
            for mut actor in &mut players {
                actor.queued = direction;
            }
        }
    }
}

fn steer(key: &Key) -> Option<Dir> {
    match key {
        Key::ArrowUp => Some(Dir::Up),
        Key::ArrowDown => Some(Dir::Down),
        Key::ArrowLeft => Some(Dir::Left),
        Key::ArrowRight => Some(Dir::Right),
        Key::Character(c) => match c.as_str() {
            "w" => Some(Dir::Up),
            "s" => Some(Dir::Down),
            "a" => Some(Dir::Left),
            "d" => Some(Dir::Right),
            _ => None,
        },
        _ => None,
    }
}

/// Everything a restart clears, so `r` starts a fresh game from any
/// state.
#[derive(SystemParam)]
pub struct Restart<'w, 's> {
    commands: Commands<'w, 's>,
    round: Round<'w>,
    pending: ResMut<'w, LevelPending>,
    spawned: Query<'w, 's, Entity, With<Spawned>>,
}

impl Restart<'_, '_> {
    fn run(&mut self) {
        for entity in &self.spawned {
            self.commands.entity(entity).despawn();
        }
        *self.round.phase = Phase::Playing;
        self.round.score.0 = 0;
        self.round.lives.0 = STARTING_LIVES;
        *self.round.ghosts = Ghosts::default();
        self.pending.0 = true;
    }
}
