//! Pixel-art bitmaps and the palettes that color them.
//!
//! The player is the ratatui rat and the ghosts are the bevy bird, each
//! traced from its logo at eighteen units square: the widest that still
//! clears the wall lines of a corridor, and enough to keep the ear, the
//! whiskers, and the bird's eye legible.

use plurimus::core::ratatui_core::style::Color;

pub const RAT_YELLOW: Color = Color::Rgb(250, 220, 60);
const EYE_WHITE: Color = Color::Rgb(245, 245, 245);
const EYE_PUPIL: Color = Color::Rgb(25, 25, 45);

pub const GHOST_RED: Color = Color::Rgb(240, 60, 60);
pub const GHOST_PINK: Color = Color::Rgb(250, 160, 200);
pub const GHOST_CYAN: Color = Color::Rgb(90, 220, 240);
pub const GHOST_ORANGE: Color = Color::Rgb(250, 170, 70);
pub const GHOST_FRIGHTENED: Color = Color::Rgb(50, 60, 200);

const BODY_PIXEL: char = 'g';

/// The ratatui rat, turned to face right: the ear stands up, the body
/// leads, and the whiskers fan out ahead of the snout.
pub const RAT: &str = "\
.......yyy........
.......yyyy.......
.......yyyyy......
.......yyyyyy.....
......yyyyyyy.....
yyyyyyyyyyyyy.....
yyyyy.ykyy........
.yyyyyykyy........
..yyyyyyyyyy......
...yyyyyyyyy......
....yyyyy.yyy.....
....yyyyyyy..y....
....yyyyyy.y..y...
..yyyyyyy......y..
..yyyyyy...y....y.
..y.yyy..........y
..y..y...........y
...yy...........y.";

/// The bevy bird. The logo stacks three birds, so this traces only the
/// foremost one; its eye is the notch that bird leaves in its own head.
pub const BIRD: &str = "\
............ggg...
...........ggwgggg
..........gggwggg.
..........ggggggg.
....gggggggggggg..
....gggggggggggg..
.......gggggggggg.
.....gggggggggggg.
...ggggggggggggggg
..gggggggggggggggg
.ggggggggggggggggg
.........ggggggggg
..........ggggggg.
..........ggggggg.
..........gggggg..
.........gggggg...
......ggggggg.....
....gggggg........";

pub const CHEESE: &str = "\
.hhhh.
hhhhhh
hhchhh
hhhhch
hhhhhh
.hhhh.";

pub const POWER_CHEESE: &str = "\
...pppp...
..pppppp..
.pppppppp.
pppphppppp
pppppppppp
ppphppppph
pppppppppp
.pppppppp.
..pppppp..
...pppp...";

pub const RAT_PALETTE: [(char, Color); 2] = [('y', RAT_YELLOW), ('k', EYE_PUPIL)];

pub const CHEESE_GOLD: Color = Color::Rgb(240, 200, 90);

pub const CHEESE_PALETTE: [(char, Color); 2] =
    [('h', CHEESE_GOLD), ('c', Color::Rgb(190, 140, 40))];

pub const POWER_PALETTE: [(char, Color); 2] = [
    ('p', Color::Rgb(255, 225, 120)),
    ('h', Color::Rgb(200, 150, 50)),
];

/// The dim half of the power cheese pulse.
pub const POWER_PALETTE_DIM: [(char, Color); 2] = [
    ('p', Color::Rgb(175, 135, 55)),
    ('h', Color::Rgb(120, 90, 30)),
];

/// The bird palette tinted for one ghost; only the eye stays light when
/// the ghost is frightened.
#[must_use]
pub fn bird_palette(body: Color) -> Vec<(char, Color)> {
    vec![(BODY_PIXEL, body), ('w', EYE_WHITE)]
}
