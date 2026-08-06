//! Pixel sampling shared by the converters: indexing, cell-to-source
//! mapping, and supersample reduction.

use bevy_math::UVec2;
use plurimus_core::raster::LinearColorSum;
use ratatui_core::layout::Rect;

pub(crate) const CHANNELS: usize = 4;
pub(crate) const ALPHA: usize = 3;

/// A camera's read-back reverse-Z depths.
#[derive(Clone, Copy)]
pub(crate) struct DepthSource<'a> {
    pub(crate) depths: &'a [f32],
    pub(crate) size: UVec2,
}

pub(crate) fn pixel_at(pixels: &[u8], size: UVec2, x: usize, y: usize) -> [u8; 4] {
    let start = (y * size.x as usize + x) * CHANNELS;
    pixels[start..start + CHANNELS]
        .try_into()
        .expect("rgba8 pixel")
}

/// Dest-cell → source coordinates: the source column and the two
/// vertical subcell rows, ratio-mapped; identity at the steady-state
/// 1 px per half-cell sizing.
fn cell_source_coords(size: UVec2, dest: Rect, cell: (u16, u16)) -> (usize, usize, usize) {
    let (column, row) = cell;
    let x = usize::from(column) * size.x as usize / usize::from(dest.width);
    let sub_rows = usize::from(dest.height) * 2;
    let upper_y = usize::from(row) * 2 * size.y as usize / sub_rows;
    let lower_y = (usize::from(row) * 2 + 1) * size.y as usize / sub_rows;
    (x, upper_y, lower_y)
}

/// A dest cell's two vertical color samples.
pub(crate) fn sample_cell_pair(
    pixels: &[u8],
    size: UVec2,
    dest: Rect,
    cell: (u16, u16),
) -> ([u8; 4], [u8; 4]) {
    let (x, upper_y, lower_y) = cell_source_coords(size, dest, cell);
    (
        pixel_at(pixels, size, x, upper_y),
        pixel_at(pixels, size, x, lower_y),
    )
}

/// A dest cell's two vertical depth samples.
pub(crate) fn sample_depth_pair(
    depth: DepthSource<'_>,
    dest: Rect,
    cell: (u16, u16),
) -> (f32, f32) {
    let (x, upper_y, lower_y) = cell_source_coords(depth.size, dest, cell);
    let width = depth.size.x as usize;
    (
        depth.depths[upper_y * width + x],
        depth.depths[lower_y * width + x],
    )
}

/// Box-filters each `factor`×`factor` block down to one pixel in linear
/// space, writing into `out` and returning the reduced size. Alpha is
/// coverage: a block is transparent only when every pixel in it is, and
/// only opaque pixels contribute to the averaged color.
pub(crate) fn box_reduce(pixels: &[u8], size: UVec2, factor: u32, out: &mut Vec<u8>) -> UVec2 {
    let reduced = size / factor;
    out.clear();
    out.reserve((reduced.x * reduced.y) as usize * CHANNELS);
    for y in 0..reduced.y {
        for x in 0..reduced.x {
            let block = reduce_block(pixels, size, factor, UVec2::new(x, y));
            out.extend_from_slice(&block);
        }
    }
    reduced
}

fn reduce_block(pixels: &[u8], size: UVec2, factor: u32, block: UVec2) -> [u8; 4] {
    let mut sum = LinearColorSum::default();
    for y in block.y * factor..(block.y + 1) * factor {
        for x in block.x * factor..(block.x + 1) * factor {
            let [red, green, blue, alpha] = pixel_at(pixels, size, x as usize, y as usize);
            if alpha != 0 {
                sum.add([red, green, blue]);
            }
        }
    }
    match sum.resolve_srgb() {
        Some([red, green, blue]) => [red, green, blue, u8::MAX],
        None => [0; CHANNELS],
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::UVec2;

    use super::box_reduce;

    #[test]
    fn blocks_average_in_linear_space() {
        let pixels: Vec<u8> = [[255u8, 255, 255, 255], [0, 0, 0, 255]].repeat(2).concat();
        let mut out = Vec::new();

        let reduced = box_reduce(&pixels, UVec2::new(2, 2), 2, &mut out);

        assert_eq!(reduced, UVec2::new(1, 1));
        assert_eq!(out, [188, 188, 188, 255]);
    }

    #[test]
    fn alpha_counts_as_coverage() {
        let pixels: Vec<u8> = [
            [255u8, 0, 0, 255],
            [0, 255, 0, 0],
            [0, 255, 0, 0],
            [0, 255, 0, 0],
        ]
        .concat();
        let mut out = Vec::new();

        box_reduce(&pixels, UVec2::new(2, 2), 2, &mut out);

        assert_eq!(out, [255, 0, 0, 255]);
    }

    #[test]
    fn fully_transparent_blocks_stay_transparent() {
        let pixels = vec![0u8; 2 * 2 * 4];
        let mut out = Vec::new();

        box_reduce(&pixels, UVec2::new(2, 2), 2, &mut out);

        assert_eq!(out, [0, 0, 0, 0]);
    }
}
