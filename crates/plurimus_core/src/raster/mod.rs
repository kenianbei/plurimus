//! Drawing below cell resolution, and resolving it back to cells.
//!
//! A terminal cell is a coarse pixel, so pipelines address a finer grid and
//! let this module collapse it: [`HalfblockGrid`] takes 1×2 points per cell
//! and resolves them to a half-block glyph with a foreground and a background
//! color, while [`BrailleGrid`] takes 2×4 dots that resolve to one braille
//! glyph in a single averaged color. [`DepthGrid`] carries the depth that
//! lets 3d occlude across cameras, [`blit_halfblocks`] and [`blit_braille`]
//! scale a pixel image into either grid, and [`ColorDepth`] downsamples the
//! finished frame for terminals that cannot show true color.

mod average;
mod blit;
mod braille;
mod color;
mod depth;
mod grid;

pub use average::LinearColorSum;
pub use blit::{PixelGrid, blit_braille, blit_halfblocks, letterbox};
pub use braille::BrailleGrid;
pub use color::{ColorDepth, downsample, linear_cell_color};
pub(crate) use color::{downsample_frame, extract_color_depth};
pub use depth::DepthGrid;
pub use grid::HalfblockGrid;
