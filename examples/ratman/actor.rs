//! Grid movement shared by the player and the ghosts.
//!
//! An actor always sits on a tile and slides toward the next one along
//! its facing; turns commit when it arrives, so movement stays on the
//! grid however smoothly it is drawn. Reversing is the exception and
//! takes effect immediately, so a turn back never feels dropped.

use bevy_ecs::prelude::Component;
use bevy_math::{IVec2, Vec2};

use crate::maze::{Maze, TILE, tile_center, wrap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    /// Every direction in the ghosts' tie-break order: one with two
    /// equally good turns takes the one listed first.
    pub const ALL: [Self; 4] = [Self::Up, Self::Left, Self::Down, Self::Right];

    #[must_use]
    pub const fn delta(self) -> IVec2 {
        match self {
            Self::Up => IVec2::NEG_Y,
            Self::Down => IVec2::Y,
            Self::Left => IVec2::NEG_X,
            Self::Right => IVec2::X,
        }
    }

    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// The world-space heading: the same step, with Y flipped because
    /// tile rows run down while world Y runs up.
    #[must_use]
    pub const fn heading(self) -> Vec2 {
        let step = self.delta();
        Vec2::new(step.x as f32, -step.y as f32)
    }
}

#[derive(Component)]
pub struct Actor {
    pub tile: IVec2,
    pub direction: Dir,
    /// The direction to take at the next tile center, or now if it is a
    /// reversal.
    pub queued: Dir,
    /// How far the actor has slid toward the next tile, in `0.0..1.0`.
    pub progress: f32,
    /// Tiles per second.
    pub speed: f32,
    pub opens_doors: bool,
}

impl Actor {
    #[must_use]
    pub const fn new(tile: IVec2, direction: Dir, speed: f32) -> Self {
        Self {
            tile,
            direction,
            queued: direction,
            progress: 0.0,
            speed,
            opens_doors: false,
        }
    }

    #[must_use]
    pub const fn opening_doors(mut self) -> Self {
        self.opens_doors = true;
        self
    }

    #[must_use]
    pub fn position(&self) -> Vec2 {
        tile_center(self.tile) + self.direction.heading() * TILE * self.progress
    }

    /// The tile the actor is sliding into.
    #[must_use]
    pub fn next_tile(&self) -> IVec2 {
        wrap(self.tile + self.direction.delta())
    }

    /// Where the actor's next turn takes effect: the tile it is sliding
    /// into, or the one it stands on once a wall has stopped it there —
    /// an actor pinned against a wall chooses from where it actually is.
    #[must_use]
    pub fn decision_tile(&self, maze: &Maze) -> IVec2 {
        if can_enter(self, maze, self.direction) {
            self.next_tile()
        } else {
            self.tile
        }
    }

    /// Puts the actor back on a tile center, facing `direction`.
    pub const fn reset(&mut self, tile: IVec2, direction: Dir) {
        self.tile = tile;
        self.direction = direction;
        self.queued = direction;
        self.progress = 0.0;
    }
}

/// Slides `actor` along the grid for `delta` seconds, turning where the
/// maze allows and stopping flush against walls.
pub fn advance(actor: &mut Actor, maze: &Maze, delta: f32) {
    reverse_if_queued(actor);
    if actor.progress == 0.0 && can_enter(actor, maze, actor.queued) {
        actor.direction = actor.queued;
    }
    if !can_enter(actor, maze, actor.direction) {
        actor.progress = 0.0;
        return;
    }
    actor.progress += actor.speed * delta;
    while actor.progress >= 1.0 {
        actor.progress -= 1.0;
        actor.tile = actor.next_tile();
        if can_enter(actor, maze, actor.queued) {
            actor.direction = actor.queued;
        }
        if !can_enter(actor, maze, actor.direction) {
            actor.progress = 0.0;
            break;
        }
    }
}

/// Turning back the way it came needs no tile center: the actor swaps
/// which tile it is leaving and keeps the ground it has covered.
fn reverse_if_queued(actor: &mut Actor) {
    if actor.progress == 0.0 || actor.queued != actor.direction.opposite() {
        return;
    }
    actor.tile = actor.next_tile();
    actor.direction = actor.queued;
    actor.progress = 1.0 - actor.progress;
}

#[must_use]
pub fn can_enter(actor: &Actor, maze: &Maze, direction: Dir) -> bool {
    maze.is_walkable(actor.tile + direction.delta(), actor.opens_doors)
}
