//! Cross-camera occlusion against the frame-wide shared depth grid.
//!
//! Camera buffers composite in order, so a later camera would otherwise paint
//! over an earlier one regardless of what is actually nearer. Sharing one
//! depth grid across the frame lets each 3d camera depth-test its cells
//! before writing, so two 3d views of the same scene interleave correctly
//! rather than layering by camera order.

use bevy_math::UVec2;
use plurimus_core::raster::DepthGrid;
use ratatui_core::layout::Rect;

use crate::sample::{ALPHA, CHANNELS, DepthSource, sample_depth_pair};

/// Depth-tests every dest cell's two subcells against the shared grid,
/// recording per-cell losses; a cell loses only when both its subcells
/// do. Returns whether anything lost.
pub(crate) fn record_losses(
    depth: DepthSource<'_>,
    dest: Rect,
    shared: &mut DepthGrid,
    losses: &mut Vec<bool>,
) -> bool {
    losses.clear();
    losses.resize(usize::from(dest.width) * usize::from(dest.height), false);
    let mut any_loss = false;
    for row in 0..dest.height {
        for column in 0..dest.width {
            let (upper, lower) = sample_depth_pair(depth, dest, (column, row));
            let sub_y = (dest.y + row) * 2;
            let upper_wins = shared
                .compare_and_update(dest.x + column, sub_y, upper)
                .unwrap_or(true);
            let lower_wins = shared
                .compare_and_update(dest.x + column, sub_y + 1, lower)
                .unwrap_or(true);
            if !upper_wins && !lower_wins {
                losses[usize::from(row) * usize::from(dest.width) + usize::from(column)] = true;
                any_loss = true;
            }
        }
    }
    any_loss
}

/// Alpha-zeroes the source-pixel spans of losing cells in place - the
/// transparent-pixels-touch-nothing contract then makes every strategy
/// and the edge overlay skip them unchanged.
pub(crate) fn mask_losing_cells(losses: &[bool], dest: Rect, pixels: &mut [u8], size: UVec2) {
    let (dest_width, dest_height) = (usize::from(dest.width), usize::from(dest.height));
    let (width, height) = (size.x as usize, size.y as usize);
    for (index, lost) in losses.iter().enumerate() {
        if !lost {
            continue;
        }
        let (column, row) = (index % dest_width, index / dest_width);
        let x_span =
            (column * width).div_ceil(dest_width)..((column + 1) * width).div_ceil(dest_width);
        for y in (row * height).div_ceil(dest_height)..((row + 1) * height).div_ceil(dest_height) {
            for x in x_span.clone() {
                pixels[(y * width + x) * CHANNELS + ALPHA] = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::UVec2;
    use plurimus_core::raster::DepthGrid;
    use ratatui_core::layout::Rect;

    use super::{mask_losing_cells, record_losses};
    use crate::sample::DepthSource;

    fn occlude(depths: &[f32], dest: Rect, shared: &mut DepthGrid, pixels: &mut [u8]) -> bool {
        let size = UVec2::new(u32::from(dest.width), u32::from(dest.height) * 2);
        let depth = DepthSource { depths, size };
        let mut losses = Vec::new();
        let masked = record_losses(depth, dest, shared, &mut losses);
        if masked {
            mask_losing_cells(&losses, dest, pixels, size);
        }
        masked
    }

    fn every_alpha_is(pixels: &[u8], alpha: u8) -> bool {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == alpha)
    }

    #[test]
    fn nearer_frames_pass_and_farther_frames_mask() {
        let area = Rect::new(0, 0, 2, 1);
        let mut shared = DepthGrid::new(area);
        let mut pixels = vec![255u8; 2 * 2 * 4];

        assert!(!occlude(&[0.8; 4], area, &mut shared, &mut pixels));
        assert!(every_alpha_is(&pixels, 255));

        assert!(occlude(&[0.5; 4], area, &mut shared, &mut pixels));
        assert!(every_alpha_is(&pixels, 0));
    }

    #[test]
    fn cells_mask_independently() {
        let area = Rect::new(0, 0, 2, 1);
        let mut shared = DepthGrid::new(area);
        let mut pixels = vec![255u8; 2 * 2 * 4];

        occlude(&[0.9, 0.0, 0.9, 0.0], area, &mut shared, &mut [255u8; 16]);
        occlude(&[0.5; 4], area, &mut shared, &mut pixels);

        assert_eq!(pixels[3], 0);
        assert_eq!(pixels[4 + 3], 255);
    }

    #[test]
    fn losing_nothing_leaves_the_shared_grid_authoritative() {
        let area = Rect::new(0, 0, 1, 1);
        let mut shared = DepthGrid::new(area);
        let mut pixels = vec![255u8; 2 * 4];

        assert!(!occlude(&[0.4, 0.4], area, &mut shared, &mut pixels));
        assert_eq!(shared.get(0, 0), Some(0.4));
        assert_eq!(shared.get(0, 1), Some(0.4));
    }
}
