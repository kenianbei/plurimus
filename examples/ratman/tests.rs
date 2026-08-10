//! Headless coverage: the maze parses and connects, the rules hold,
//! and every layer of the scene reaches the composed frame.

use bevy_math::IVec2;
use bevy_time::TimeUpdateStrategy;
use plurimus::core::TerminalSize;
use plurimus::term::KeyCode;
use plurimus_test::{composed_frame, composed_styled_frame, write_key};

use crate::actor::{Actor, Dir, advance};
use crate::chase::best_direction;
use crate::game::{Lives, Phase, Player, Score, add_game};
use crate::ghosts::{Ghost, GhostState};
use crate::maze::{COLS, Cheese, Maze, ROWS, Tile};
use crate::sprites::{CHEESE_GOLD, GHOST_RED, RAT_YELLOW};
use crate::walls::WALL_COLOR;
use crate::*;

const STEP: Duration = Duration::from_millis(50);

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin, Plugin2d));
    app.insert_resource(TerminalSize {
        cols: 280,
        rows: 76,
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
    add_game(&mut app);
    app.update();
    app
}

fn player_actor(app: &mut App) -> &Actor {
    let world = app.world_mut();
    let mut players = world.query_filtered::<&Actor, bevy_ecs::prelude::With<Player>>();
    players.iter(world).next().unwrap()
}

fn set_player(app: &mut App, tile: IVec2, direction: Dir) {
    let world = app.world_mut();
    let mut players = world.query_filtered::<&mut Actor, bevy_ecs::prelude::With<Player>>();
    let mut player = players.iter_mut(world).next().unwrap();
    player.reset(tile, direction);
}

fn cheese_count(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut cheese = world.query::<&Cheese>();
    cheese.iter(world).count()
}

#[test]
fn every_map_row_is_the_declared_width() {
    let maze = Maze::default();
    for row in 0..ROWS {
        for col in 0..COLS {
            let _ = maze.tile(IVec2::new(col, row));
        }
    }
    assert_eq!(maze.tile(IVec2::new(0, 0)), Tile::Wall);
    assert_eq!(maze.tile(IVec2::new(1, 1)), Tile::Open);
    assert_eq!(maze.tile(IVec2::ZERO + IVec2::new(13, 7)), Tile::Door);
}

#[test]
fn the_tunnel_row_wraps_between_both_edges() {
    let maze = Maze::default();
    let left = IVec2::new(0, 8);
    assert!(maze.is_walkable(left, false));
    assert!(maze.is_walkable(IVec2::new(COLS - 1, 8), false));
    assert!(maze.is_walkable(left + Dir::Left.delta(), false));
}

/// Which tiles the player can stand on, flooded out from its spawn. The
/// ghost house is not among them, since the player cannot pass the door.
fn player_reachable(maze: &Maze) -> Vec<bool> {
    let mut seen = vec![false; (COLS * ROWS) as usize];
    let mut queue = vec![maze.player_spawn];
    seen[index_of(maze.player_spawn)] = true;
    while let Some(tile) = queue.pop() {
        for direction in Dir::ALL {
            let next = crate::maze::wrap(tile + direction.delta());
            if next.y < 0 || next.y >= ROWS || !maze.is_walkable(next, false) {
                continue;
            }
            if !seen[index_of(next)] {
                seen[index_of(next)] = true;
                queue.push(next);
            }
        }
    }
    seen
}

fn index_of(tile: IVec2) -> usize {
    (tile.y * COLS + tile.x) as usize
}

/// Every cheese has to be reachable from the spawn without passing
/// the ghost house door, or the game could never be won.
#[test]
fn all_cheese_is_reachable_from_the_player_spawn() {
    let maze = Maze::default();
    let seen = player_reachable(&maze);
    for (tile, _) in &maze.cheese {
        assert!(seen[index_of(*tile)], "unreachable {tile}");
    }
}

/// Ghosts are useless if they cannot find the door, and no rule of the
/// game would notice: every spawn has to reach open ground.
#[test]
fn every_ghost_spawn_finds_its_way_out_of_the_house() {
    let maze = Maze::default();
    let outside = player_reachable(&maze);
    let target = IVec2::new(1, 1);
    for &spawn in &maze.ghost_spawns {
        let mut ghost = Actor::new(spawn, Dir::Up, 1.0).opening_doors();
        let escaped = (0..40).any(|_| {
            ghost.queued = best_direction(&maze, &ghost, target, false);
            advance(&mut ghost, &maze, 1.0);
            outside[index_of(ghost.tile)]
        });
        assert!(escaped, "the ghost at {spawn} never left the house");
    }
}

#[test]
fn the_player_wraps_through_the_tunnel() {
    let mut app = headless_app();
    set_player(&mut app, IVec2::new(0, 8), Dir::Left);

    for _ in 0..6 {
        app.update();
    }

    assert_eq!(player_actor(&mut app).tile.x, COLS - 1);
}

#[test]
fn walls_stop_the_player() {
    let mut app = headless_app();
    set_player(&mut app, IVec2::new(1, 13), Dir::Left);

    for _ in 0..10 {
        app.update();
    }

    let player = player_actor(&mut app);
    assert_eq!(player.tile, IVec2::new(1, 13));
    assert_eq!(player.progress, 0.0);
}

#[test]
fn a_queued_turn_commits_at_the_next_tile_center() {
    let mut app = headless_app();
    set_player(&mut app, IVec2::new(1, 13), Dir::Right);
    write_key(&mut app, KeyCode::Up);

    for _ in 0..10 {
        app.update();
    }

    let player = player_actor(&mut app);
    assert_eq!(player.direction, Dir::Up);
    assert!(player.tile.y < 13);
}

#[test]
fn eating_cheese_scores_and_clears_the_tile() {
    let mut app = headless_app();
    let before = cheese_count(&mut app);
    set_player(&mut app, IVec2::new(1, 13), Dir::Right);

    for _ in 0..8 {
        app.update();
    }

    assert!(cheese_count(&mut app) < before);
    assert!(app.world().resource::<Score>().0 > 0);
}

#[test]
fn a_power_cheese_frightens_the_ghosts_and_they_can_be_eaten() {
    let mut app = headless_app();
    set_player(&mut app, IVec2::new(1, 12), Dir::Up);
    app.update();
    app.update();

    assert!(ghost_states(&mut app).contains(&GhostState::Frightened));

    let tile = player_actor(&mut app).tile;
    set_player(&mut app, tile, Dir::Up);
    set_ghost_tile(&mut app, tile);
    app.update();

    assert!(ghost_states(&mut app).contains(&GhostState::Eaten));
    assert_eq!(app.world().resource::<Lives>().0, 3);
}

#[test]
fn a_chasing_ghost_costs_a_life() {
    let mut app = headless_app();
    let tile = player_actor(&mut app).tile;
    set_ghost_tile(&mut app, tile);

    app.update();

    assert_eq!(app.world().resource::<Lives>().0, 2);
    assert_eq!(*app.world().resource::<Phase>(), Phase::Playing);
}

#[test]
fn losing_every_life_ends_the_game() {
    let mut app = headless_app();
    for _ in 0..3 {
        let tile = player_actor(&mut app).tile;
        set_ghost_tile(&mut app, tile);
        app.update();
    }

    assert_eq!(app.world().resource::<Lives>().0, 0);
    assert_eq!(*app.world().resource::<Phase>(), Phase::Lost);
}

#[test]
fn eating_the_last_cheese_wins() {
    let mut app = headless_app();
    let world = app.world_mut();
    let mut cheese =
        world.query_filtered::<bevy_ecs::prelude::Entity, bevy_ecs::prelude::With<Cheese>>();
    let all: Vec<_> = cheese.iter(world).collect();
    for entity in all {
        world.despawn(entity);
    }

    app.update();

    assert_eq!(*app.world().resource::<Phase>(), Phase::Won);
}

#[test]
fn restarting_rebuilds_the_maze_and_the_counters() {
    let mut app = headless_app();
    let full = cheese_count(&mut app);
    set_player(&mut app, IVec2::new(1, 13), Dir::Right);
    for _ in 0..8 {
        app.update();
    }
    assert!(cheese_count(&mut app) < full);

    write_key(&mut app, KeyCode::Char('r'));
    app.update();
    app.update();

    assert_eq!(cheese_count(&mut app), full);
    assert_eq!(app.world().resource::<Score>().0, 0);
    assert_eq!(app.world().resource::<Lives>().0, 3);
}

#[test]
fn a_small_terminal_pauses_the_game() {
    let mut app = headless_app();
    app.insert_resource(TerminalSize { cols: 80, rows: 24 });
    set_player(&mut app, IVec2::new(1, 13), Dir::Right);

    for _ in 0..8 {
        app.update();
    }

    let player = player_actor(&mut app);
    assert_eq!(player.tile, IVec2::new(1, 13));
    assert_eq!(player.progress, 0.0);
}

/// Every layer has to reach the frame: the wall lines, the cheese,
/// the rat, and a ghost each own a color nothing else uses.
#[test]
fn the_maze_and_its_actors_rasterize() {
    let mut app = headless_app();
    app.update();

    let frame = composed_styled_frame(&app);
    for color in [WALL_COLOR, CHEESE_GOLD, RAT_YELLOW, GHOST_RED] {
        assert!(frame.contains(&format!("{color:?}")), "missing {color:?}");
    }
}

#[test]
fn a_small_terminal_says_so() {
    let mut app = headless_app();
    app.insert_resource(TerminalSize { cols: 80, rows: 24 });

    app.update();

    assert!(composed_frame(&app).contains("too small"));
}

fn ghost_states(app: &mut App) -> Vec<GhostState> {
    let world = app.world_mut();
    let mut ghosts = world.query::<&Ghost>();
    ghosts.iter(world).map(|ghost| ghost.state).collect()
}

/// Drops the first ghost straight onto `tile` so a contact happens
/// this frame, whatever the ghosts were doing.
fn set_ghost_tile(app: &mut App, tile: IVec2) {
    let world = app.world_mut();
    let mut ghosts = world.query_filtered::<&mut Actor, bevy_ecs::prelude::With<Ghost>>();
    let mut ghost = ghosts.iter_mut(world).next().unwrap();
    ghost.reset(tile, Dir::Left);
}
