//! Ghost behaviour: when they scatter, who they chase, and how they flee.

use bevy_ecs::prelude::{Commands, Component, Query, Res, ResMut, Resource, With, Without};
use bevy_math::IVec2;
use bevy_time::{Time, Timer, TimerMode};
use bevy_transform::components::Transform;
use plurimus::core::ratatui_core::style::Color;
use plurimus::render2d::PixelBlock;

use crate::actor::{Actor, Dir};
use crate::chase::{Hunt, best_direction};
use crate::game::{Player, Spawned};
use crate::maze::{COLS, Maze, ROWS, tile_center};
use crate::sprites::{
    BIRD, GHOST_CYAN, GHOST_FRIGHTENED, GHOST_ORANGE, GHOST_PINK, GHOST_RED, bird_palette,
};

const CHASE_SPEED: f32 = 4.4;
const FRIGHTENED_SPEED: f32 = 2.6;
const EATEN_SPEED: f32 = 9.0;
const SCATTER_SECONDS: f32 = 7.0;
const CHASE_SECONDS: f32 = 20.0;
const FRIGHTENED_SECONDS: f32 = 8.0;
/// Seconds between one ghost leaving the house and the next.
const RELEASE_STAGGER_SECONDS: f32 = 2.5;
const EATEN_COLOR: Color = Color::Rgb(80, 80, 110);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostState {
    Chasing,
    Frightened,
    Eaten,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Personality {
    /// Runs straight at the player.
    Chaser,
    /// Aims where the player is going.
    Ambusher,
    /// Aims at the player's far side from the chaser.
    Flanker,
    /// Hunts from afar, breaks off up close.
    Skittish,
}

#[derive(Component)]
pub struct Ghost {
    pub personality: Personality,
    pub state: GhostState,
    color: Color,
    pub scatter_corner: IVec2,
    pub home: IVec2,
    pub spawn: IVec2,
    /// Seconds on the clock before this ghost leaves the house.
    release_after: f32,
}

/// The shared scatter/chase clock and the frightened window.
#[derive(Resource)]
pub struct Ghosts {
    scattering: bool,
    phase: Timer,
    frightened: Timer,
    just_frightened: bool,
    elapsed: f32,
}

impl Default for Ghosts {
    fn default() -> Self {
        Self {
            scattering: true,
            phase: Timer::from_seconds(SCATTER_SECONDS, TimerMode::Once),
            frightened: spent_timer(FRIGHTENED_SECONDS),
            just_frightened: false,
            elapsed: 0.0,
        }
    }
}

/// A one-shot timer that has already run out, so its window is closed
/// until something reopens it.
fn spent_timer(seconds: f32) -> Timer {
    let mut timer = Timer::from_seconds(seconds, TimerMode::Once);
    timer.finish();
    timer
}

impl Ghosts {
    pub fn frighten(&mut self) {
        self.frightened.reset();
        self.just_frightened = true;
    }

    fn is_frightening(&self) -> bool {
        !self.frightened.is_finished()
    }

    /// Sends everyone back to their corners after a life is lost: the
    /// house releases them again on the original schedule.
    pub fn regroup(&mut self) {
        *self = Self::default();
    }
}

pub fn spawn_ghosts(commands: &mut Commands, maze: &Maze) {
    let crew = [
        (Personality::Chaser, GHOST_RED, IVec2::new(COLS - 2, 1)),
        (Personality::Ambusher, GHOST_PINK, IVec2::new(1, 1)),
        (
            Personality::Flanker,
            GHOST_CYAN,
            IVec2::new(COLS - 2, ROWS - 2),
        ),
        (Personality::Skittish, GHOST_ORANGE, IVec2::new(1, ROWS - 2)),
    ];
    let home = maze.ghost_house();
    for (index, &spawn) in maze.ghost_spawns.iter().enumerate() {
        let (personality, color, scatter_corner) = crew[index % crew.len()];
        commands.spawn((
            Spawned,
            Ghost {
                personality,
                state: GhostState::Chasing,
                color,
                scatter_corner,
                home,
                spawn,
                release_after: index as f32 * RELEASE_STAGGER_SECONDS,
            },
            Actor::new(spawn, Dir::Left, CHASE_SPEED).opening_doors(),
            PixelBlock::new(BIRD, bird_palette(color)),
            Transform::from_translation(tile_center(spawn).extend(2.0)),
        ));
    }
}

/// Advances the scatter/chase clock, applies a fresh power cheese, and
/// turns the ghosts around whenever the mode changes.
pub fn tick_modes(
    time: Res<Time>,
    mut state: ResMut<Ghosts>,
    mut ghosts: Query<(&mut Ghost, &mut Actor)>,
) {
    state.elapsed += time.delta_secs();
    if std::mem::take(&mut state.just_frightened) {
        for (mut ghost, mut actor) in &mut ghosts {
            if ghost.state == GhostState::Chasing {
                ghost.state = GhostState::Frightened;
                actor.queued = actor.direction.opposite();
            }
        }
    }
    if state.frightened.tick(time.delta()).just_finished() {
        calm_ghosts(&mut ghosts);
    }
    if state.is_frightening() {
        return;
    }
    if state.phase.tick(time.delta()).just_finished() {
        swap_phase(&mut state, &mut ghosts);
    }
}

fn calm_ghosts(ghosts: &mut Query<(&mut Ghost, &mut Actor)>) {
    for (mut ghost, _) in ghosts.iter_mut() {
        if ghost.state == GhostState::Frightened {
            ghost.state = GhostState::Chasing;
        }
    }
}

fn swap_phase(state: &mut Ghosts, ghosts: &mut Query<(&mut Ghost, &mut Actor)>) {
    state.scattering = !state.scattering;
    let seconds = if state.scattering {
        SCATTER_SECONDS
    } else {
        CHASE_SECONDS
    };
    state.phase = Timer::from_seconds(seconds, TimerMode::Once);
    for (_, mut actor) in ghosts.iter_mut() {
        actor.queued = actor.direction.opposite();
    }
}

pub fn steer_ghosts(
    maze: Res<Maze>,
    state: Res<Ghosts>,
    players: Query<&Actor, With<Player>>,
    mut ghosts: Query<(&mut Ghost, &mut Actor), Without<Player>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let hunt = Hunt {
        player_tile: player.tile,
        player_dir: player.direction,
        chaser_tile: chaser_tile(&ghosts),
        scattering: state.scattering,
    };
    for (mut ghost, mut actor) in &mut ghosts {
        if ghost.state == GhostState::Eaten && actor.tile == ghost.home {
            ghost.state = GhostState::Chasing;
        }
        if state.elapsed < ghost.release_after {
            actor.speed = 0.0;
            continue;
        }
        actor.speed = speed_for(ghost.state);
        let target = ghost.target(actor.tile, &hunt);
        let fleeing = ghost.state == GhostState::Frightened;
        actor.queued = best_direction(&maze, &actor, target, fleeing);
    }
}

fn chaser_tile(ghosts: &Query<(&mut Ghost, &mut Actor), Without<Player>>) -> IVec2 {
    ghosts
        .iter()
        .find(|(ghost, _)| ghost.personality == Personality::Chaser)
        .map_or(IVec2::ZERO, |(_, actor)| actor.tile)
}

const fn speed_for(state: GhostState) -> f32 {
    match state {
        GhostState::Chasing => CHASE_SPEED,
        GhostState::Frightened => FRIGHTENED_SPEED,
        GhostState::Eaten => EATEN_SPEED,
    }
}

impl Ghost {
    const fn body_color(&self) -> Color {
        match self.state {
            GhostState::Chasing => self.color,
            GhostState::Frightened => GHOST_FRIGHTENED,
            GhostState::Eaten => EATEN_COLOR,
        }
    }
}

/// Keeps each ghost's colors in step with its state.
pub fn dress_ghosts(mut ghosts: Query<(&Ghost, &mut PixelBlock)>) {
    for (ghost, mut sprite) in &mut ghosts {
        sprite.palette = bird_palette(ghost.body_color());
    }
}
