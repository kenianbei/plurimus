//! Per-cell scalar fields the edge overlay runs its sobel over.

use core::ops::Range;

use bevy_color::Luminance;
use bevy_math::UVec2;
use plurimus_core::raster::LinearColorSum;
use ratatui_core::layout::Rect;

use super::EdgeSource;
use crate::convert::FrameSources;
use crate::sample::{DepthSource, pixel_at};

const SOBEL_X: [[f32; 3]; 3] = [[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]];
const SOBEL_Y: [[f32; 3]; 3] = [[-1.0, -2.0, -1.0], [0.0, 0.0, 0.0], [1.0, 2.0, 1.0]];

/// One field per enabled source; `None` when the source is disabled or
/// its frame data is missing.
pub(super) struct EdgeFields {
    pub(super) luminance: Option<Vec<f32>>,
    pub(super) depth: Option<Vec<f32>>,
}

impl EdgeFields {
    pub(super) fn resolve(source: EdgeSource, sources: &FrameSources<'_>, dest: Rect) -> Self {
        Self {
            luminance: source
                .reads_luminance()
                .then(|| cell_luminances(sources.pixels, sources.size, dest)),
            depth: sources
                .depth
                .filter(|_| source.reads_depth())
                .map(|depth| cell_depths(depth, dest)),
        }
    }
}

/// Per-dest-cell linear luminance, box-averaged over each cell's source
/// pixel block; transparent pixels count as black.
fn cell_luminances(pixels: &[u8], size: UVec2, dest: Rect) -> Vec<f32> {
    field(dest, size, |columns, rows| {
        let mut sum = LinearColorSum::default();
        for y in rows {
            for x in columns.clone() {
                let [red, green, blue, alpha] = pixel_at(pixels, size, x, y);
                sum.add(if alpha == 0 {
                    [0; 3]
                } else {
                    [red, green, blue]
                });
            }
        }
        sum.resolve_linear()
            .map_or(0.0, |linear| linear.luminance())
    })
}

/// Per-dest-cell reversed-z depth, box-averaged over the same block
/// mapping as [`cell_luminances`].
fn cell_depths(depth: DepthSource<'_>, dest: Rect) -> Vec<f32> {
    let width = depth.size.x as usize;
    field(dest, depth.size, |columns, rows| {
        let samples = (columns.len() * rows.len()) as f32;
        let mut sum = 0.0;
        for y in rows {
            for x in columns.clone() {
                sum += depth.depths[y * width + x];
            }
        }
        sum / samples
    })
}

/// One value per dest cell, built from the source block it covers.
fn field(
    dest: Rect,
    size: UVec2,
    mut block_value: impl FnMut(Range<usize>, Range<usize>) -> f32,
) -> Vec<f32> {
    let grid = (usize::from(dest.width), usize::from(dest.height));
    let mut values = Vec::with_capacity(grid.0 * grid.1);
    for row in 0..grid.1 {
        for column in 0..grid.0 {
            let (columns, rows) = cell_block(size, (column, row), grid);
            values.push(block_value(columns, rows));
        }
    }
    values
}

/// The source block a dest cell covers, ratio-mapped and always at
/// least one sample wide.
fn cell_block(
    size: UVec2,
    cell: (usize, usize),
    grid: (usize, usize),
) -> (Range<usize>, Range<usize>) {
    let (source_w, source_h) = (size.x as usize, size.y as usize);
    let (column, row) = cell;
    let (columns, rows) = grid;
    let x0 = (column * source_w / columns).min(source_w - 1);
    let x1 = ((column + 1) * source_w / columns).clamp(x0 + 1, source_w);
    let y0 = (row * source_h / rows).min(source_h - 1);
    let y1 = ((row + 1) * source_h / rows).clamp(y0 + 1, source_h);
    (x0..x1, y0..y1)
}

pub(super) fn sobel_at(values: &[f32], width: u16, cell: (u16, u16)) -> (f32, f32) {
    let width = usize::from(width);
    let (column, row) = (usize::from(cell.0), usize::from(cell.1));
    let (mut gx, mut gy) = (0.0, 0.0);
    for (dy, kernel_row) in SOBEL_X.iter().enumerate() {
        for (dx, weight_x) in kernel_row.iter().enumerate() {
            let value = values[(row + dy - 1) * width + column + dx - 1];
            gx += weight_x * value;
            gy += SOBEL_Y[dy][dx] * value;
        }
    }
    (gx, gy)
}

#[cfg(test)]
mod tests {
    use bevy_math::UVec2;
    use ratatui_core::layout::Rect;

    use super::{DepthSource, cell_depths, sobel_at};

    #[test]
    fn depth_cells_average_their_source_block() {
        let depths = [0.0, 0.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5];
        let source = DepthSource {
            depths: &depths,
            size: UVec2::new(4, 2),
        };

        let cells = cell_depths(source, Rect::new(0, 0, 2, 1));

        assert_eq!(cells, [0.25, 0.75]);
    }

    #[test]
    fn sobel_reports_the_step_axis() {
        let step: Vec<f32> = (0..9).map(|index| f32::from(index % 3 == 2)).collect();

        let (gx, gy) = sobel_at(&step, 3, (1, 1));

        assert_eq!((gx, gy), (4.0, 0.0));
    }
}
