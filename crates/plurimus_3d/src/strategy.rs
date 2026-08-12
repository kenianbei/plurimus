//! Pixel-to-cell conversion strategies.
//!
//! [`Strategy3d`] chooses how a rendered image becomes text: halfblocks for
//! the most color, a luminance ramp for shape in a single color, or a depth
//! ramp keyed on distance instead of brightness. The choice is per camera and
//! can change at runtime, since the converters hold no state.

use bevy_ecs::prelude::Component;
use bevy_math::UVec2;

/// How a 3d camera's rendered pixels become terminal cells.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub enum Strategy3d {
    /// `▀`/`▄` color pairs at half-cell resolution.
    #[default]
    Halfblocks,
    /// Characters chosen by brightness, colored by the averaged pixel.
    Luminance(LuminanceRamp),
    /// Braille dots at 2×4 subcell resolution; foreground-only, so cell
    /// backgrounds show the camera background through unset dots.
    Braille,
    /// Characters chosen by camera depth (nearer is denser), colored by
    /// the scene pixels. Requires [`DepthReadback`](crate::DepthReadback)
    /// on the camera; without a depth frame nothing draws.
    Depth(DepthRamp),
    /// No base conversion: the camera renders and reads back but writes
    /// no cells - alone it shows only its background; paired with
    /// [`EdgeOverlay`](crate::EdgeOverlay) it renders an edges-only
    /// wireframe.
    None,
}

impl Strategy3d {
    /// Render-target pixel density per terminal cell.
    pub(crate) const fn pixels_per_cell(&self) -> UVec2 {
        match self {
            Self::Halfblocks | Self::Luminance(_) | Self::Depth(_) | Self::None => UVec2::new(1, 2),
            Self::Braille => UVec2::new(2, 4),
        }
    }
}

/// Brightness-to-character mapping for [`Strategy3d::Luminance`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuminanceRamp {
    /// Characters from darkest to brightest.
    pub characters: &'static [char],
    /// Multiplier applied to relative luminance before indexing; typical
    /// lit scenes sit low in linear space, so the default boosts them.
    pub scale: f32,
}

/// Depth-to-character mapping for [`Strategy3d::Depth`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthRamp {
    /// Characters from farthest to nearest.
    pub characters: &'static [char],
    /// Multiplier applied to reverse-Z depth before indexing; raw
    /// depths sit near zero for most scenes, so the default boosts
    /// them.
    pub scale: f32,
}

const DEPTH_SCALE_DEFAULT: f32 = 30.0;

impl Default for DepthRamp {
    fn default() -> Self {
        Self {
            characters: RAMP_ASCII,
            scale: DEPTH_SCALE_DEFAULT,
        }
    }
}

/// ASCII density ramp.
pub const RAMP_ASCII: &[char] = &[' ', '.', ':', '+', '=', '!', '*', '?', '#', '%', '&', '@'];

/// Shade-block ramp.
pub const RAMP_SHADING: &[char] = &[' ', '░', '▒', '▓', '█'];

/// Lower-block eighths ramp.
pub const RAMP_BLOCKS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Braille-density ramp.
pub const RAMP_BRAILLE: &[char] = &[' ', '⠂', '⠒', '⠖', '⠶', '⠷', '⠿', '⡿', '⣿'];

impl Default for LuminanceRamp {
    fn default() -> Self {
        Self {
            characters: RAMP_ASCII,
            scale: 10.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_targets_render_at_dot_density() {
        assert_eq!(Strategy3d::Braille.pixels_per_cell(), UVec2::new(2, 4));
        assert_eq!(Strategy3d::Halfblocks.pixels_per_cell(), UVec2::new(1, 2));
    }
}
