//! Where each ghost is headed, and the turn that gets it there.

use bevy_math::IVec2;

use crate::actor::{Actor, Dir};
use crate::ghosts::{Ghost, GhostState, Personality};
use crate::maze::{Maze, wrap};

/// Tiles the ambusher aims ahead of the player, and the range inside
/// which the skittish bird loses its nerve and heads for its corner.
const AMBUSH_LEAD: i32 = 4;
const SKITTISH_RANGE: f32 = 8.0;

/// What every ghost needs to know about the hunt this frame.
pub struct Hunt {
    pub player_tile: IVec2,
    pub player_dir: Dir,
    pub chaser_tile: IVec2,
    pub scattering: bool,
}

impl Ghost {
    pub fn target(&self, tile: IVec2, hunt: &Hunt) -> IVec2 {
        match self.state {
            GhostState::Eaten => return self.home,
            GhostState::Frightened => return hunt.player_tile,
            GhostState::Chasing => {}
        }
        if hunt.scattering {
            return self.scatter_corner;
        }
        match self.personality {
            Personality::Chaser => hunt.player_tile,
            Personality::Ambusher => hunt.player_tile + hunt.player_dir.delta() * AMBUSH_LEAD,
            Personality::Flanker => hunt.player_tile * 2 - hunt.chaser_tile,
            Personality::Skittish => self.skittish_target(tile, hunt),
        }
    }

    fn skittish_target(&self, tile: IVec2, hunt: &Hunt) -> IVec2 {
        if tile_distance(tile, hunt.player_tile) > SKITTISH_RANGE {
            hunt.player_tile
        } else {
            self.scatter_corner
        }
    }
}

/// Picks the turn to take where the actor's next one takes effect: the
/// one that closes on `target`, or opens the most distance when fleeing.
/// Ghosts never turn back unless there is nowhere else to go.
pub fn best_direction(maze: &Maze, actor: &Actor, target: IVec2, fleeing: bool) -> Dir {
    let from = actor.decision_tile(maze);
    let back = actor.direction.opposite();
    let mut best: Option<(f32, Dir)> = None;
    for direction in Dir::ALL {
        if direction == back {
            continue;
        }
        let candidate = wrap(from + direction.delta());
        if !maze.is_walkable(candidate, actor.opens_doors) {
            continue;
        }
        let distance = tile_distance(candidate, target);
        let score = if fleeing { -distance } else { distance };
        if best.is_none_or(|(current, _)| score < current) {
            best = Some((score, direction));
        }
    }
    best.map_or(back, |(_, direction)| direction)
}

fn tile_distance(from: IVec2, to: IVec2) -> f32 {
    from.as_vec2().distance(to.as_vec2())
}
