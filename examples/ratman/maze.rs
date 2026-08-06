//! The maze map: tile grid, world geometry, and the entities drawn for it.

use bevy_ecs::prelude::{Commands, Component, Query, Res, ResMut, Resource};
use bevy_math::{IVec2, Vec2};
use bevy_time::{Time, Timer, TimerMode};
use bevy_transform::components::Transform;
use plurimus::core::ratatui_core::style::Color;
use plurimus::render2d::PixelBlock;

use crate::game::Spawned;
use crate::sprites::{CHEESE, CHEESE_PALETTE, POWER_CHEESE, POWER_PALETTE, POWER_PALETTE_DIM};
use crate::walls::spawn_wall;

/// World units per maze tile. One unit is one halfblock subcell, so a
/// tile is ten cells wide and five terminal rows tall.
pub const TILE: f32 = 10.0;
pub const COLS: i32 = 28;
pub const ROWS: i32 = 15;

/// Cells the maze needs, plus the HUD row.
pub const REQUIRED_COLS: u16 = COLS as u16 * TILE as u16;
pub const REQUIRED_ROWS: u16 = ROWS as u16 * TILE as u16 / 2 + 1;

const PULSE_SECONDS: f32 = 0.4;

const WALL: char = '#';
const DOOR: char = '-';
const CHEESE_TILE: char = '.';
const POWER_TILE: char = 'o';
const PLAYER_SPAWN: char = 'P';
const GHOST_SPAWN: char = 'g';

/// Twenty-eight tiles wide and fifteen tall: a central ghost house and
/// wrapping side tunnels, kept short enough that a terminal can still
/// show sprites large enough to read.
const MAP: &str = "\
############################
#............##............#
#.####.#####.##.#####.####.#
#o####.#####.##.#####.####o#
#..........................#
#.####.##.########.##.####.#
#......##....g.....##......#
######.##.###--###.##.######
..........#g g g #..........
######.##.########.##.######
#............##............#
#.####.#####.##.#####.####.#
#o..##.......##.......##..o#
#............P.............#
############################";

/// What a tile does to movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Wall,
    /// The ghost house door: ghosts pass, the player does not.
    Door,
    Open,
}

/// The parsed map: what each tile is, and where everything starts.
#[derive(Resource)]
pub struct Maze {
    tiles: Vec<Tile>,
    pub player_spawn: IVec2,
    pub ghost_spawns: Vec<IVec2>,
    pub cheese: Vec<(IVec2, bool)>,
    door: IVec2,
}

/// Drives the power cheese blink.
#[derive(Resource)]
pub struct Pulse {
    timer: Timer,
    dim: bool,
}

impl Default for Pulse {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(PULSE_SECONDS, TimerMode::Repeating),
            dim: false,
        }
    }
}

/// A cheese the player has yet to eat.
#[derive(Component)]
pub struct Cheese {
    pub tile: IVec2,
    pub is_power: bool,
}

impl Default for Maze {
    fn default() -> Self {
        Self::parse(MAP)
    }
}

impl Maze {
    fn parse(map: &str) -> Self {
        let mut maze = Self {
            tiles: Vec::with_capacity((COLS * ROWS) as usize),
            player_spawn: IVec2::ZERO,
            ghost_spawns: Vec::new(),
            cheese: Vec::new(),
            door: IVec2::ZERO,
        };
        for (row, line) in map.lines().enumerate() {
            for (col, symbol) in line.chars().enumerate() {
                let tile = IVec2::new(col as i32, row as i32);
                maze.tiles.push(read_tile(symbol));
                maze.record_contents(symbol, tile);
            }
        }
        maze
    }

    fn record_contents(&mut self, symbol: char, tile: IVec2) {
        match symbol {
            CHEESE_TILE => self.cheese.push((tile, false)),
            POWER_TILE => self.cheese.push((tile, true)),
            PLAYER_SPAWN => self.player_spawn = tile,
            GHOST_SPAWN => self.ghost_spawns.push(tile),
            DOOR => self.door = tile,
            _ => {}
        }
    }

    /// Where eaten ghosts reassemble: the tile inside the house, just
    /// through the door.
    #[must_use]
    pub fn ghost_house(&self) -> IVec2 {
        self.door + IVec2::Y
    }

    #[must_use]
    pub fn tile(&self, tile: IVec2) -> Tile {
        if tile.y < 0 || tile.y >= ROWS {
            return Tile::Wall;
        }
        self.tiles[(tile.y * COLS + wrap(tile).x) as usize]
    }

    /// Whether an actor may stand on `tile`; only ghosts pass the door.
    #[must_use]
    pub fn is_walkable(&self, tile: IVec2, opens_doors: bool) -> bool {
        match self.tile(tile) {
            Tile::Open => true,
            Tile::Door => opens_doors,
            Tile::Wall => false,
        }
    }
}

fn read_tile(symbol: char) -> Tile {
    match symbol {
        WALL => Tile::Wall,
        DOOR => Tile::Door,
        _ => Tile::Open,
    }
}

/// Wraps a tile column through the tunnels; rows do not wrap.
#[must_use]
pub fn wrap(tile: IVec2) -> IVec2 {
    IVec2::new(tile.x.rem_euclid(COLS), tile.y)
}

/// The world position of a tile's center, with the maze centered on the
/// origin and Y up.
#[must_use]
pub fn tile_center(tile: IVec2) -> Vec2 {
    Vec2::new(
        (tile.x as f32 - (COLS - 1) as f32 / 2.0) * TILE,
        ((ROWS - 1) as f32 / 2.0 - tile.y as f32) * TILE,
    )
}

pub fn spawn_maze(commands: &mut Commands, maze: &Maze) {
    for row in 0..ROWS {
        for col in 0..COLS {
            let tile = IVec2::new(col, row);
            if maze.tile(tile) == Tile::Wall {
                spawn_wall(commands, maze, tile);
            }
        }
    }
    for &(tile, is_power) in &maze.cheese {
        spawn_cheese(commands, tile, is_power);
    }
}

/// Blinks every power cheese together, so the four of them read as the
/// prize they are.
pub fn pulse_power_cheese(
    time: Res<Time>,
    mut pulse: ResMut<Pulse>,
    mut cheese: Query<(&Cheese, &mut PixelBlock)>,
) {
    if !pulse.timer.tick(time.delta()).just_finished() {
        return;
    }
    pulse.dim = !pulse.dim;
    for (morsel, mut sprite) in &mut cheese {
        if morsel.is_power {
            sprite.palette = power_palette(pulse.dim);
        }
    }
}

fn power_palette(dim: bool) -> Vec<(char, Color)> {
    if dim {
        POWER_PALETTE_DIM.into()
    } else {
        POWER_PALETTE.into()
    }
}

fn spawn_cheese(commands: &mut Commands, tile: IVec2, is_power: bool) {
    let (bitmap, palette) = if is_power {
        (POWER_CHEESE, POWER_PALETTE)
    } else {
        (CHEESE, CHEESE_PALETTE)
    };
    commands.spawn((
        Spawned,
        Cheese { tile, is_power },
        PixelBlock::new(bitmap, palette),
        Transform::from_translation(tile_center(tile).extend(1.0)),
    ));
}
