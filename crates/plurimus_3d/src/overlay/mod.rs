//! CPU sobel edge overlay drawn over converted frames.

mod fields;

use bevy_ecs::prelude::Component;
use ratatui_core::style::Color;

use crate::convert::{Canvas, FrameSources};
use fields::{EdgeFields, sobel_at};

const DEFAULT_THRESHOLD: f32 = 0.2;
const DEFAULT_DEPTH_THRESHOLD: f32 = 0.002;
const DIRECTION_BUCKET_DEGREES: f32 = 45.0;

/// Directional characters for [`EdgeOverlay`], one per gradient bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EdgeCharacters {
    /// Horizontal edges.
    pub horizontal: char,
    /// Vertical edges.
    pub vertical: char,
    /// Rising diagonals.
    pub rising: char,
    /// Falling diagonals.
    pub falling: char,
}

impl Default for EdgeCharacters {
    fn default() -> Self {
        Self {
            horizontal: '―',
            vertical: '|',
            rising: '/',
            falling: '\\',
        }
    }
}

/// What an [`EdgeOverlay`] measures its gradients over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum EdgeSource {
    /// Shading gradients of the color frame.
    #[default]
    Luminance,
    /// Depth discontinuities: true silhouettes, blind to shading and to
    /// the black cells occlusion masking leaves behind. Requires
    /// [`DepthReadback`](crate::DepthReadback) on the camera; with no
    /// depth frame nothing is drawn.
    Depth,
    /// Both, drawing wherever either clears its own threshold.
    Both,
}

impl EdgeSource {
    const fn reads_luminance(self) -> bool {
        matches!(self, Self::Luminance | Self::Both)
    }

    const fn reads_depth(self) -> bool {
        matches!(self, Self::Depth | Self::Both)
    }
}

/// Overwrites cells on strong gradients with directional edge
/// characters, after the base strategy has drawn. Composes with any
/// [`Strategy3d`](crate::Strategy3d); over `Strategy3d::None` it
/// renders an edges-only wireframe. Cells at or below the threshold are
/// untouched, so the base render shows through.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct EdgeOverlay {
    /// Which frame data the gradients are measured over.
    pub source: EdgeSource,
    /// Minimum sobel gradient magnitude, over linear luminance, that
    /// draws an edge.
    pub threshold: f32,
    /// Minimum sobel gradient magnitude over reversed-z depth. Depth is
    /// `near / distance` and is never linearized, so a useful threshold
    /// scales with the scene. The default was measured on geometry tens
    /// of units out, where silhouettes reach ~0.007 and ground slope
    /// stays below ~0.002; a scene an order of magnitude closer wants an
    /// order of magnitude more.
    pub depth_threshold: f32,
    /// Characters per edge direction.
    pub characters: EdgeCharacters,
    /// Foreground for edge cells; `None` keeps each cell's own color.
    pub color: Option<Color>,
}

impl EdgeOverlay {
    /// Sets which frame data the gradients are measured over.
    #[must_use]
    pub const fn with_source(mut self, source: EdgeSource) -> Self {
        self.source = source;
        self
    }

    /// Sets the luminance gradient magnitude that draws an edge.
    #[must_use]
    pub const fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Sets the depth gradient magnitude that draws an edge.
    #[must_use]
    pub const fn with_depth_threshold(mut self, depth_threshold: f32) -> Self {
        self.depth_threshold = depth_threshold;
        self
    }

    /// Sets the characters drawn per edge direction.
    #[must_use]
    pub const fn with_characters(mut self, characters: EdgeCharacters) -> Self {
        self.characters = characters;
        self
    }

    /// Sets the foreground for edge cells; `None` keeps each cell's own.
    #[must_use]
    pub const fn with_color(mut self, color: Option<Color>) -> Self {
        self.color = color;
        self
    }
}

impl Default for EdgeOverlay {
    fn default() -> Self {
        Self {
            source: EdgeSource::default(),
            threshold: DEFAULT_THRESHOLD,
            depth_threshold: DEFAULT_DEPTH_THRESHOLD,
            characters: EdgeCharacters::default(),
            color: None,
        }
    }
}

pub(crate) fn apply_edge_overlay(
    overlay: &EdgeOverlay,
    sources: &FrameSources<'_>,
    canvas: &mut Canvas<'_>,
) {
    let dest = canvas.dest;
    if sources.size.x == 0 || sources.size.y == 0 || dest.width < 3 || dest.height < 3 {
        return;
    }
    let fields = EdgeFields::resolve(overlay.source, sources, dest);
    let scoring = Scoring::new(overlay);
    for row in 1..dest.height - 1 {
        for column in 1..dest.width - 1 {
            let Some(direction) = edge_at(&fields, &scoring, dest.width, (column, row)) else {
                continue;
            };
            let position = (dest.x + column, dest.y + row);
            write_edge_cell(canvas.buffer, position, overlay, direction);
        }
    }
}

/// Reciprocal squared thresholds, hoisted out of the cell loop: scoring
/// is then one multiply, and squares order the same as magnitudes.
struct Scoring {
    luminance: f32,
    depth: f32,
}

impl Scoring {
    fn new(overlay: &EdgeOverlay) -> Self {
        Self {
            luminance: 1.0 / (overlay.threshold * overlay.threshold),
            depth: 1.0 / (overlay.depth_threshold * overlay.depth_threshold),
        }
    }
}

/// The strongest enabled gradient, when it clears its threshold. Scores
/// are multiples of each source's own threshold - the only calibration
/// the two domains share, so mistuning one skews the arbitration.
fn edge_at(
    fields: &EdgeFields,
    scoring: &Scoring,
    width: u16,
    cell: (u16, u16),
) -> Option<Direction> {
    let scored = |values: &Option<Vec<f32>>, inverse: f32| {
        let values = values.as_ref()?;
        let (gx, gy) = sobel_at(values, width, cell);
        Some((gx.mul_add(gx, gy * gy) * inverse, gx, gy))
    };
    let (score, gx, gy) = scored(&fields.luminance, scoring.luminance)
        .into_iter()
        .chain(scored(&fields.depth, scoring.depth))
        .max_by(|left, right| left.0.total_cmp(&right.0))?;
    (score > 1.0).then(|| direction(gx, gy))
}

/// Buckets the gradient angle into the four edge directions; the edge
/// runs perpendicular to the gradient.
fn direction(gx: f32, gy: f32) -> Direction {
    let angle = gy.atan2(gx).to_degrees().rem_euclid(180.0);
    let half_bucket = DIRECTION_BUCKET_DEGREES / 2.0;
    match ((angle + half_bucket) / DIRECTION_BUCKET_DEGREES) as u32 % 4 {
        0 => Direction::Vertical,
        1 => Direction::Rising,
        2 => Direction::Horizontal,
        _ => Direction::Falling,
    }
}

enum Direction {
    Horizontal,
    Vertical,
    Rising,
    Falling,
}

fn write_edge_cell(
    buffer: &mut ratatui_core::buffer::Buffer,
    position: (u16, u16),
    overlay: &EdgeOverlay,
    direction: Direction,
) {
    let Some(cell) = buffer.cell_mut(position) else {
        return;
    };
    cell.set_char(match direction {
        Direction::Horizontal => overlay.characters.horizontal,
        Direction::Vertical => overlay.characters.vertical,
        Direction::Rising => overlay.characters.rising,
        Direction::Falling => overlay.characters.falling,
    });
    if let Some(color) = overlay.color {
        cell.set_fg(color);
    }
}

#[cfg(test)]
mod tests;
