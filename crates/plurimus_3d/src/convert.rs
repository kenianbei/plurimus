//! Pure pixel-to-cell converters over readback pixel grids.
//!
//! Each strategy is a plain function from pixels to cells with no world
//! access, which is what makes them testable in isolation and swappable at
//! runtime. Halfblock keeps two colors per cell; the ramp strategies trade
//! color for shape, picking a character by luminance or depth.

use bevy_color::{LinearRgba, Luminance};
use bevy_math::UVec2;
use plurimus_core::raster::{
    BrailleGrid, HalfblockGrid, LinearColorSum, PixelGrid, blit_braille, blit_halfblocks,
    linear_cell_color,
};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

use crate::sample::{ALPHA, DepthSource, sample_cell_pair, sample_depth_pair};
use crate::strategy::{DepthRamp, LuminanceRamp, Strategy3d};

/// Per-camera source data a conversion reads: color pixels always,
/// depths only when the camera reads them back.
pub(crate) struct FrameSources<'a> {
    pub(crate) pixels: &'a [u8],
    pub(crate) size: UVec2,
    pub(crate) depth: Option<DepthSource<'a>>,
}

/// Per-camera scratch buffers reused across conversions.
#[derive(Default)]
pub(crate) struct Scratch3d {
    halfblock: HalfblockGrid,
    braille: BrailleGrid,
}

/// A camera buffer plus the cell rectangle the conversion writes into -
/// the whole buffer area normally, a centered sub-rect when
/// letterboxing. Cells outside `dest` are never touched.
pub(crate) struct Canvas<'a> {
    pub(crate) buffer: &'a mut Buffer,
    pub(crate) dest: Rect,
}

impl<'a> Canvas<'a> {
    pub(crate) const fn new(buffer: &'a mut Buffer, dest: Rect) -> Self {
        Self { buffer, dest }
    }

    #[cfg(test)]
    pub(crate) const fn full(buffer: &'a mut Buffer) -> Self {
        let dest = buffer.area;
        Self { buffer, dest }
    }
}

impl Strategy3d {
    /// Converts a tightly packed RGBA8 grid into the canvas's `dest`
    /// cells, scaling when the resolutions differ. Fully transparent
    /// pixels touch nothing. `scratch` is reused across calls.
    pub(crate) fn convert(
        &self,
        sources: &FrameSources<'_>,
        scratch: &mut Scratch3d,
        canvas: &mut Canvas<'_>,
    ) {
        let (pixels, size) = (sources.pixels, sources.size);
        if size.x == 0 || size.y == 0 {
            return;
        }
        match self {
            Self::Halfblocks => convert_halfblocks(pixels, size, &mut scratch.halfblock, canvas),
            Self::Luminance(ramp) => convert_luminance(pixels, size, ramp, canvas),
            Self::Braille => convert_braille(pixels, size, &mut scratch.braille, canvas),
            Self::Depth(ramp) => convert_depth(sources, ramp, canvas),
            Self::None => {}
        }
    }
}

fn convert_halfblocks(
    pixels: &[u8],
    size: UVec2,
    grid: &mut HalfblockGrid,
    canvas: &mut Canvas<'_>,
) {
    let source = PixelGrid::rgba8(pixels, size.x as usize, size.y as usize);
    grid.reset(canvas.dest);
    let fit = grid.subcell_area();
    blit_halfblocks(grid, &source, fit);
    grid.resolve_into(canvas.buffer);
}

fn convert_braille(pixels: &[u8], size: UVec2, grid: &mut BrailleGrid, canvas: &mut Canvas<'_>) {
    let source = PixelGrid::rgba8(pixels, size.x as usize, size.y as usize);
    grid.reset(canvas.dest);
    let fit = grid.subcell_area();
    blit_braille(grid, &source, fit);
    grid.resolve_into(canvas.buffer);
}

fn convert_luminance(pixels: &[u8], size: UVec2, ramp: &LuminanceRamp, canvas: &mut Canvas<'_>) {
    let dest = canvas.dest;
    for row in 0..dest.height {
        for column in 0..dest.width {
            let (upper, lower) = sample_cell_pair(pixels, size, dest, (column, row));
            if upper[ALPHA] == 0 && lower[ALPHA] == 0 {
                continue;
            }
            let linear = average_linear(upper, lower);
            let position = (dest.left() + column, dest.top() + row);
            write_ramp_cell(
                canvas.buffer,
                position,
                (linear.luminance() * ramp.scale, linear),
                ramp.characters,
            );
        }
    }
}

fn convert_depth(sources: &FrameSources<'_>, ramp: &DepthRamp, canvas: &mut Canvas<'_>) {
    let Some(depth) = sources.depth else {
        return;
    };
    let dest = canvas.dest;
    for row in 0..dest.height {
        for column in 0..dest.width {
            let (upper, lower) =
                sample_cell_pair(sources.pixels, sources.size, dest, (column, row));
            if upper[ALPHA] == 0 && lower[ALPHA] == 0 {
                continue;
            }
            let (upper_depth, lower_depth) = sample_depth_pair(depth, dest, (column, row));
            let position = (dest.left() + column, dest.top() + row);
            write_ramp_cell(
                canvas.buffer,
                position,
                (
                    upper_depth.max(lower_depth) * ramp.scale,
                    average_linear(upper, lower),
                ),
                ramp.characters,
            );
        }
    }
}

/// Writes one ramp-indexed cell: the (already scaled) level picks the
/// character, the linear color becomes the foreground.
fn write_ramp_cell(
    buffer: &mut Buffer,
    position: (u16, u16),
    (level, linear): (f32, LinearRgba),
    characters: &[char],
) {
    let index = (level.clamp(0.0, 1.0) * (characters.len() - 1) as f32).round() as usize;
    if let Some(cell) = buffer.cell_mut(position) {
        cell.set_char(characters[index]);
        cell.set_fg(linear_cell_color(linear));
    }
}

fn average_linear(a: [u8; 4], b: [u8; 4]) -> LinearRgba {
    let mut sum = LinearColorSum::default();
    for [red, green, blue, _] in [a, b] {
        sum.add([red, green, blue]);
    }
    sum.resolve_linear().expect("two samples were added")
}

#[cfg(test)]
mod tests {
    use bevy_math::UVec2;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Color;

    use super::{Canvas, FrameSources, Scratch3d};
    use crate::sample::DepthSource;
    use crate::strategy::{DepthRamp, LuminanceRamp, RAMP_SHADING, Strategy3d};

    fn rgba_grid(pixels: &[[u8; 4]]) -> Vec<u8> {
        pixels.iter().flatten().copied().collect()
    }

    fn sources(pixels: &[u8], size: UVec2) -> FrameSources<'_> {
        FrameSources {
            pixels,
            size,
            depth: None,
        }
    }

    fn depth_sources<'a>(pixels: &'a [u8], depths: &'a [f32], size: UVec2) -> FrameSources<'a> {
        FrameSources {
            pixels,
            size,
            depth: Some(DepthSource { depths, size }),
        }
    }

    #[test]
    fn depth_picks_nearer_denser_characters_and_scene_color() {
        let ramp = DepthRamp {
            characters: RAMP_SHADING,
            scale: 1.0,
        };
        let pixels = rgba_grid(&[[255, 0, 0, 255]; 4]);
        let depths = [0.9, 0.01, 0.9, 0.01];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Depth(ramp).convert(
            &depth_sources(&pixels, &depths, UVec2::new(2, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "█");
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((0, 0)).unwrap().fg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn depth_without_a_depth_frame_draws_nothing() {
        let pixels = rgba_grid(&[[255, 0, 0, 255], [0, 0, 255, 255]]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Depth(DepthRamp::default()).convert(
            &sources(&pixels, UVec2::new(1, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer, Buffer::empty(Rect::new(0, 0, 1, 1)));
    }

    #[test]
    fn depth_skips_fully_transparent_cells() {
        let pixels = rgba_grid(&[[255, 0, 0, 0], [0, 0, 255, 0]]);
        let depths = [0.9, 0.9];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Depth(DepthRamp::default()).convert(
            &depth_sources(&pixels, &depths, UVec2::new(1, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn halfblocks_map_pixel_pairs_to_fg_bg() {
        let pixels = rgba_grid(&[[255, 0, 0, 255], [0, 0, 255, 255]]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Halfblocks.convert(
            &sources(&pixels, UVec2::new(1, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        let cell = buffer.cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn transparent_pixels_leave_cells_untouched() {
        let pixels = rgba_grid(&[[255, 0, 0, 0], [0, 0, 255, 0]]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Halfblocks.convert(
            &sources(&pixels, UVec2::new(1, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn luminance_picks_darkest_and_brightest_ramp_ends() {
        let ramp = LuminanceRamp {
            characters: RAMP_SHADING,
            scale: 1.0,
        };
        let pixels = rgba_grid(&[
            [3, 3, 3, 255],
            [255, 255, 255, 255],
            [3, 3, 3, 255],
            [255, 255, 255, 255],
        ]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Luminance(ramp).convert(
            &sources(&pixels, UVec2::new(2, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "█");
        assert_eq!(buffer.cell((1, 0)).unwrap().fg, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn luminance_skips_fully_transparent_cells_only() {
        let ramp = LuminanceRamp {
            characters: RAMP_SHADING,
            scale: 1.0,
        };
        let pixels = rgba_grid(&[
            [0, 0, 0, 0],
            [255, 255, 255, 255],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        buffer.cell_mut((0, 0)).unwrap().set_symbol("K");
        let mut scratch = Scratch3d::default();

        Strategy3d::Luminance(ramp).convert(
            &sources(&pixels, UVec2::new(2, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "K");
        assert_ne!(buffer.cell((1, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn oversized_luminance_frames_scale_down_instead_of_cropping() {
        let ramp = LuminanceRamp {
            characters: RAMP_SHADING,
            scale: 1.0,
        };
        let mut pixels = vec![[3u8, 3, 3, 255]; 4 * 4];
        for row in 0..4 {
            pixels[row * 4 + 2] = [255, 255, 255, 255];
            pixels[row * 4 + 3] = [255, 255, 255, 255];
        }
        let pixels = rgba_grid(&pixels);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Luminance(ramp).convert(
            &sources(&pixels, UVec2::new(4, 4)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "█");
    }

    #[test]
    fn letterboxed_dest_leaves_the_bars_untouched() {
        let pixels = rgba_grid(&[[255, 0, 0, 255], [0, 0, 255, 255]]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        let mut scratch = Scratch3d::default();
        let mut canvas = Canvas::new(&mut buffer, Rect::new(1, 0, 1, 1));

        Strategy3d::Halfblocks.convert(
            &sources(&pixels, UVec2::new(1, 2)),
            &mut scratch,
            &mut canvas,
        );

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "▀");
        assert_eq!(buffer.cell((2, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn braille_maps_dots_and_averages_their_color() {
        let mut pixels = vec![[0u8, 0, 255, 255]; 2 * 4];
        pixels[0] = [255, 0, 0, 255];
        let pixels = rgba_grid(&pixels);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Braille.convert(
            &sources(&pixels, UVec2::new(2, 4)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        let cell = buffer.cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "⣿");
        assert_eq!(cell.bg, Color::Reset);
        let Color::Rgb(red, _, blue) = cell.fg else {
            panic!("expected an averaged rgb foreground, got {:?}", cell.fg);
        };
        assert!(blue > red);
    }

    #[test]
    fn none_writes_no_cells() {
        let pixels = rgba_grid(&[[255, 0, 0, 255], [0, 0, 255, 255]]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::None.convert(
            &sources(&pixels, UVec2::new(1, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        assert_eq!(buffer, Buffer::empty(Rect::new(0, 0, 1, 1)));
    }

    #[test]
    fn oversized_frames_scale_down_to_the_buffer_area() {
        let pixels = vec![200; 8 * 4 * 4];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Halfblocks.convert(
            &sources(&pixels, UVec2::new(8, 4)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        let cell = buffer.cell((1, 0)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(200, 200, 200));
    }

    #[test]
    fn undersized_frames_scale_up_to_the_buffer_area() {
        let pixels = rgba_grid(&[[255, 0, 0, 255], [0, 0, 255, 255]]);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));
        let mut scratch = Scratch3d::default();

        Strategy3d::Halfblocks.convert(
            &sources(&pixels, UVec2::new(1, 2)),
            &mut scratch,
            &mut Canvas::full(&mut buffer),
        );

        for x in 0..2 {
            let cell = buffer.cell((x, 0)).unwrap();
            assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
            assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
        }
    }
}
