//! Scene setup, player control, and the rules that end a life or a game.

use bevy_app::{App, Startup, Update};
use bevy_ecs::prelude::{
    Commands, Component, Entity, IntoScheduleConfigs, Query, Res, ResMut, Resource, With, Without,
};
use bevy_ecs::system::SystemParam;
use bevy_time::Time;
use bevy_transform::components::Transform;
use plurimus::core::{Background, Edge, TerminalCamera, TerminalSize, Viewport};
use plurimus::render2d::{PixelBlock, Projection2d};

use crate::actor::{Actor, Dir, advance};
use crate::ghosts::{self, Ghost, GhostState, Ghosts, spawn_ghosts};
use crate::hud::{self, HudCamera, spawn_hud};
use crate::input::handle_keys;
use crate::maze::{
    Cheese, Maze, Pulse, REQUIRED_COLS, REQUIRED_ROWS, TILE, pulse_power_cheese, spawn_maze,
};
use crate::sprites::{RAT, RAT_PALETTE};

const PLAYER_SPEED: f32 = 5.0;
const CHEESE_POINTS: u32 = 10;
const POWER_POINTS: u32 = 50;
const GHOST_POINTS: u32 = 200;
pub const STARTING_LIVES: u32 = 3;
/// How close the player and a ghost must come, in world units, to touch.
const CONTACT_RANGE: f32 = TILE * 0.75;

#[derive(Component)]
pub struct Player;

/// Marks everything a restart clears and spawns again.
#[derive(Component)]
pub struct Spawned;

#[derive(Resource, Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Phase {
    #[default]
    Playing,
    Won,
    Lost,
}

#[derive(Resource)]
pub struct Score(pub u32);

#[derive(Resource)]
pub struct Lives(pub u32);

/// Set while the level still has to be built, so one system spawns the
/// maze and its actors for both the first game and every restart.
#[derive(Resource)]
pub struct LevelPending(pub bool);

/// The state a single round carries: its counters and the ghost clock.
#[derive(SystemParam)]
pub struct Round<'w> {
    pub phase: ResMut<'w, Phase>,
    pub score: ResMut<'w, Score>,
    pub lives: ResMut<'w, Lives>,
    pub ghosts: ResMut<'w, Ghosts>,
}

pub fn add_game(app: &mut App) {
    app.init_resource::<Maze>();
    app.init_resource::<Phase>();
    app.init_resource::<Ghosts>();
    app.insert_resource(Score(0));
    app.insert_resource(Lives(STARTING_LIVES));
    app.insert_resource(LevelPending(true));
    app.init_resource::<Pulse>();
    app.add_systems(Startup, spawn_scene);
    app.add_systems(
        Update,
        (
            (
                ghosts::steer_ghosts,
                move_actors,
                eat_cheese,
                ghosts::tick_modes,
                resolve_contacts,
                check_win,
            )
                .chain()
                .run_if(is_running),
            (
                spawn_level,
                pulse_power_cheese,
                hud::update_hud,
                hud::update_notice,
                handle_keys,
            ),
            // Drawing reads what the rules just decided, and runs even
            // while they are paused so the scene stays on screen.
            (draw_actors, ghosts::dress_ghosts).after(check_win),
        ),
    );
}

fn is_running(phase: Res<Phase>, size: Res<TerminalSize>, pending: Res<LevelPending>) -> bool {
    *phase == Phase::Playing && terminal_fits(*size) && !pending.0
}

#[must_use]
pub const fn terminal_fits(size: TerminalSize) -> bool {
    size.cols >= REQUIRED_COLS && size.rows >= REQUIRED_ROWS
}

fn spawn_scene(mut commands: Commands) {
    commands.spawn((TerminalCamera::default(), Projection2d::default()));
    let hud_camera = commands
        .spawn((
            TerminalCamera {
                order: 1,
                viewport: Viewport::Docked {
                    edge: Edge::Top,
                    cells: 1,
                },
                ..TerminalCamera::default()
            },
            HudCamera,
        ))
        .id();
    let overlay = commands
        .spawn(TerminalCamera {
            order: 2,
            background: Background::Transparent,
            ..TerminalCamera::default()
        })
        .id();
    spawn_hud(&mut commands, hud_camera, overlay);
}

fn spawn_level(mut commands: Commands, maze: Res<Maze>, mut pending: ResMut<LevelPending>) {
    if !pending.0 {
        return;
    }
    pending.0 = false;
    spawn_maze(&mut commands, &maze);
    spawn_ghosts(&mut commands, &maze);
    commands.spawn((
        Spawned,
        Player,
        Actor::new(maze.player_spawn, Dir::Left, PLAYER_SPEED),
        PixelBlock::new(RAT, RAT_PALETTE).mirrored(true),
        Transform::default(),
    ));
}

fn move_actors(time: Res<Time>, maze: Res<Maze>, mut actors: Query<&mut Actor>) {
    for mut actor in &mut actors {
        advance(&mut actor, &maze, time.delta_secs());
    }
}

/// Puts every actor where it has slid to, facing the way it is going;
/// both sprites are drawn looking right.
fn draw_actors(mut actors: Query<(&Actor, &mut Transform, &mut PixelBlock)>) {
    for (actor, mut transform, mut sprite) in &mut actors {
        let position = actor.position();
        transform.translation.x = position.x;
        transform.translation.y = position.y;
        match actor.direction {
            Dir::Left => sprite.mirrored = true,
            Dir::Right => sprite.mirrored = false,
            Dir::Up | Dir::Down => {}
        }
    }
}

fn eat_cheese(
    mut commands: Commands,
    mut round: Round,
    players: Query<&Actor, With<Player>>,
    cheese: Query<(Entity, &Cheese)>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    for (entity, morsel) in &cheese {
        if morsel.tile != player.tile {
            continue;
        }
        commands.entity(entity).despawn();
        if morsel.is_power {
            round.score.0 += POWER_POINTS;
            round.ghosts.frighten();
        } else {
            round.score.0 += CHEESE_POINTS;
        }
    }
}

fn resolve_contacts(
    mut round: Round,
    maze: Res<Maze>,
    mut players: Query<&mut Actor, With<Player>>,
    mut ghosts: Query<(&mut Ghost, &mut Actor), Without<Player>>,
) {
    let Ok(mut player) = players.single_mut() else {
        return;
    };
    let mut caught = false;
    for (mut ghost, actor) in &mut ghosts {
        if actor.position().distance(player.position()) > CONTACT_RANGE {
            continue;
        }
        match ghost.state {
            GhostState::Frightened => {
                ghost.state = GhostState::Eaten;
                round.score.0 += GHOST_POINTS;
            }
            GhostState::Chasing => caught = true,
            GhostState::Eaten => {}
        }
    }
    if caught {
        take_life(&mut round, &mut player, maze.player_spawn);
        regroup_ghosts(&mut round, &mut ghosts);
    }
}

fn take_life(round: &mut Round, player: &mut Actor, spawn: bevy_math::IVec2) {
    round.lives.0 = round.lives.0.saturating_sub(1);
    if round.lives.0 == 0 {
        *round.phase = Phase::Lost;
    }
    player.reset(spawn, Dir::Left);
}

fn regroup_ghosts(
    round: &mut Round,
    ghosts: &mut Query<(&mut Ghost, &mut Actor), Without<Player>>,
) {
    round.ghosts.regroup();
    for (mut ghost, mut actor) in ghosts.iter_mut() {
        ghost.state = GhostState::Chasing;
        actor.reset(ghost.spawn, Dir::Left);
    }
}

fn check_win(cheese: Query<&Cheese>, mut phase: ResMut<Phase>) {
    if cheese.iter().next().is_none() {
        *phase = Phase::Won;
    }
}
