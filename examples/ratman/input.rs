//! Keys: steering the rat, restarting a round, and leaving.

use bevy_app::AppExit;
use bevy_ecs::prelude::{Commands, Entity, MessageReader, MessageWriter, Query, ResMut, With};
use bevy_ecs::system::SystemParam;
use plurimus::input::{KeyCode, KeyKind, KeyMessage};

use crate::actor::{Actor, Dir};
use crate::game::{LevelPending, Phase, Player, Round, STARTING_LIVES, Spawned};
use crate::ghosts::Ghosts;

pub fn handle_keys(
    mut keys: MessageReader<KeyMessage>,
    mut exit: MessageWriter<AppExit>,
    mut players: Query<&mut Actor, With<Player>>,
    mut restart: Restart,
) {
    for key in keys.read() {
        if key.kind == KeyKind::Release {
            continue;
        }
        let ctrl_c = key.modifiers.ctrl && key.code == KeyCode::Char('c');
        if key.code == KeyCode::Char('q') || ctrl_c {
            exit.write(AppExit::Success);
        }
        if key.code == KeyCode::Char('r') {
            restart.run();
        }
        if let Some(direction) = steer(key.code) {
            for mut actor in &mut players {
                actor.queued = direction;
            }
        }
    }
}

const fn steer(code: KeyCode) -> Option<Dir> {
    match code {
        KeyCode::Up | KeyCode::Char('w') => Some(Dir::Up),
        KeyCode::Down | KeyCode::Char('s') => Some(Dir::Down),
        KeyCode::Left | KeyCode::Char('a') => Some(Dir::Left),
        KeyCode::Right | KeyCode::Char('d') => Some(Dir::Right),
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
