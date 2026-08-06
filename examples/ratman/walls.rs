//! Drawing a wall tile: lines along the sides that face open ground, so
//! wall regions read as outlined blocks rather than solid slabs.

use std::ops::Range;

use bevy_ecs::prelude::Commands;
use bevy_math::IVec2;
use bevy_transform::components::Transform;
use plurimus::core::ratatui_core::style::Color;
use plurimus::render2d::PixelBlock;

use crate::game::Spawned;
use crate::maze::{Maze, TILE, Tile, tile_center};

/// Wall lines sit this many units in from the tile edge they face,
/// leaving a twenty-unit gap across a one-tile corridor — enough for an
/// eighteen-unit sprite to pass without touching them.
const WALL_INSET: usize = 5;
const WALL_THICKNESS: usize = 2;
pub const WALL_COLOR: Color = Color::Rgb(40, 60, 220);
const WALL_PIXEL: char = 'w';

/// Draws a wall tile as lines along the sides that face open ground, so
/// solid wall regions read as outlined blocks and single-tile runs as
/// bars.
pub fn spawn_wall(commands: &mut Commands, maze: &Maze, tile: IVec2) {
    let mut rows = vec![vec!['.'; TILE as usize]; TILE as usize];
    draw_faces(&mut rows, maze, tile);
    draw_corners(&mut rows, maze, tile);
    let bitmap: Vec<String> = rows.into_iter().map(|row| row.iter().collect()).collect();
    commands.spawn((
        Spawned,
        PixelBlock {
            rows: bitmap,
            palette: vec![(WALL_PIXEL, WALL_COLOR)],
            mirrored: false,
        },
        Transform::from_translation(tile_center(tile).extend(0.0)),
    ));
}

fn draw_faces(rows: &mut [Vec<char>], maze: &Maze, tile: IVec2) {
    let full = 0..TILE as usize;
    for delta in [IVec2::NEG_Y, IVec2::Y, IVec2::NEG_X, IVec2::X] {
        if maze.tile(tile + delta) == Tile::Wall {
            continue;
        }
        let past_the_middle = delta.x + delta.y > 0;
        if delta.x == 0 {
            fill(rows, face_span(past_the_middle), full.clone());
        } else {
            fill(rows, full.clone(), face_span(past_the_middle));
        }
    }
}

/// Turns the corner where a wall region ends: with both neighbors walled
/// and the diagonal open, the two lines that stop at this tile need an
/// elbow to meet.
fn draw_corners(rows: &mut [Vec<char>], maze: &Maze, tile: IVec2) {
    for (across, down) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        let sides = [IVec2::new(across, 0), IVec2::new(0, down)];
        if sides
            .iter()
            .any(|&side| maze.tile(tile + side) != Tile::Wall)
        {
            continue;
        }
        if maze.tile(tile + IVec2::new(across, down)) == Tile::Wall {
            continue;
        }
        let (right, below) = (across > 0, down > 0);
        fill(rows, face_span(below), arm_span(right));
        fill(rows, arm_span(below), face_span(right));
    }
}

/// Where a line sits along the axis it faces across.
fn face_span(positive: bool) -> Range<usize> {
    let start = if positive {
        TILE as usize - WALL_INSET - WALL_THICKNESS
    } else {
        WALL_INSET
    };
    start..start + WALL_THICKNESS
}

/// The half of the tile a corner elbow reaches into.
fn arm_span(positive: bool) -> Range<usize> {
    let middle = TILE as usize - WALL_INSET - WALL_THICKNESS;
    if positive {
        middle..TILE as usize
    } else {
        0..middle + WALL_THICKNESS
    }
}

fn fill(rows: &mut [Vec<char>], row_span: Range<usize>, col_span: Range<usize>) {
    for row in row_span {
        for col in col_span.clone() {
            rows[row][col] = WALL_PIXEL;
        }
    }
}
