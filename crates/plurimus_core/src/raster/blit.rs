//! Getting a pixel image onto a subcell grid.
//!
//! [`PixelGrid`] borrows pixels in either layout the pipelines produce -
//! RGBA8 bytes or XRGB32 words - so a GPU readback is sampled where it lies
//! instead of being converted first. [`letterbox`] works out the centered,
//! aspect-preserving rectangle to draw into, and [`blit_halfblocks`] and
//! [`blit_braille`] sample the image into that rectangle with a
//! nearest-neighbor walk. Every rectangle here is in a grid's subcell
//! coordinates rather than cells, so the same fit covers twice the rows on a
//! halfblock grid and four times as many on braille.

use ratatui_core::layout::Rect;
use ratatui_core::style::Color;

use super::{BrailleGrid, HalfblockGrid};

const RGBA_CHANNELS: usize = 4;

enum PixelData<'a> {
    Rgba8(&'a [u8]),
    Xrgb32(&'a [u32]),
}

/// A borrowed pixel grid to blit from, rows top to bottom.
pub struct PixelGrid<'a> {
    pixels: PixelData<'a>,
    width: usize,
    height: usize,
}

impl<'a> PixelGrid<'a> {
    /// Tightly packed RGBA bytes; fully transparent pixels are skipped
    /// when blitted, leaving their subcells untouched for compositing.
    ///
    /// Debug-asserts that `pixels` holds at least `width * height` pixels.
    #[must_use]
    pub fn rgba8(pixels: &'a [u8], width: usize, height: usize) -> Self {
        debug_assert!(pixels.len() >= width * height * RGBA_CHANNELS);
        Self {
            pixels: PixelData::Rgba8(pixels),
            width,
            height,
        }
    }

    /// Packed `0x00RRGGBB` words; always opaque, high byte ignored.
    ///
    /// Debug-asserts that `pixels` holds at least `width * height` pixels.
    #[must_use]
    pub fn xrgb32(pixels: &'a [u32], width: usize, height: usize) -> Self {
        debug_assert!(pixels.len() >= width * height);
        Self {
            pixels: PixelData::Xrgb32(pixels),
            width,
            height,
        }
    }

    /// Pixel columns.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Pixel rows.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Fills `out` with the grid as tightly packed RGBA8 bytes
    /// (`width * height * 4`), reusing its allocation across calls.
    /// RGBA sources copy through with their alpha intact; XRGB sources
    /// emit alpha 255 with the high byte dropped.
    pub fn write_rgba8(&self, out: &mut Vec<u8>) {
        out.clear();
        let pixel_count = self.width * self.height;
        match self.pixels {
            PixelData::Rgba8(bytes) => {
                out.extend_from_slice(&bytes[..pixel_count * RGBA_CHANNELS]);
            }
            PixelData::Xrgb32(words) => {
                out.reserve(pixel_count * RGBA_CHANNELS);
                for &word in &words[..pixel_count] {
                    let [red, green, blue] = xrgb_channels(word);
                    out.extend_from_slice(&[red, green, blue, u8::MAX]);
                }
            }
        }
    }

    fn color_at(&self, x: usize, y: usize) -> Option<Color> {
        let index = y * self.width + x;
        match self.pixels {
            PixelData::Rgba8(bytes) => {
                let start = index * RGBA_CHANNELS;
                rgba_color(&bytes[start..start + RGBA_CHANNELS])
            }
            PixelData::Xrgb32(words) => Some(xrgb_color(words[index])),
        }
    }
}

fn rgba_color(pixel: &[u8]) -> Option<Color> {
    if pixel[3] == 0 {
        return None;
    }
    Some(Color::Rgb(pixel[0], pixel[1], pixel[2]))
}

const fn xrgb_color(pixel: u32) -> Color {
    let [red, green, blue] = xrgb_channels(pixel);
    Color::Rgb(red, green, blue)
}

const fn xrgb_channels(pixel: u32) -> [u8; 3] {
    [(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]
}

/// Centered min-scale fit of `source` pixels into `target`, both in
/// subcell space - pass a grid's subcell area
/// ([`HalfblockGrid::subcell_area`] or [`BrailleGrid::subcell_area`]).
/// Returns the content rectangle in the target's coordinates, ready for
/// the blit functions; empty input yields [`Rect::ZERO`].
#[must_use]
pub fn letterbox(source: (usize, usize), target: Rect) -> Rect {
    let (source_w, source_h) = source;
    let (target_w, target_h) = (usize::from(target.width), usize::from(target.height));
    if source_w == 0 || source_h == 0 || target_w == 0 || target_h == 0 {
        return Rect::ZERO;
    }
    let (fit_w, fit_h) = if target_w * source_h >= target_h * source_w {
        (target_h * source_w / source_h, target_h)
    } else {
        (target_w, target_w * source_h / source_w)
    };
    Rect::new(
        target.x + ((target_w - fit_w) / 2) as u16,
        target.y + ((target_h - fit_h) / 2) as u16,
        fit_w as u16,
        fit_h as u16,
    )
}

/// Nearest-neighbor blit of `pixels` into `fit`, given in the grid's
/// absolute subcell coordinates - [`letterbox`] output, or the grid's
/// whole [`subcell_area`](HalfblockGrid::subcell_area) for a stretch
/// fit. Subcells outside `fit` are untouched; what shows behind the
/// content is the camera's [`Background`](crate::Background).
pub fn blit_halfblocks(grid: &mut HalfblockGrid, pixels: &PixelGrid<'_>, fit: Rect) {
    sample_fit(pixels, fit, |x, sub_y, color| grid.set(x, sub_y, color));
}

/// Nearest-neighbor blit of `pixels` into `fit`, given in the grid's
/// absolute subcell coordinates - [`letterbox`] output, or the grid's
/// whole [`subcell_area`](BrailleGrid::subcell_area) for a stretch fit.
/// Subcells outside `fit` are untouched; what shows behind the content
/// is the camera's [`Background`](crate::Background).
pub fn blit_braille(grid: &mut BrailleGrid, pixels: &PixelGrid<'_>, fit: Rect) {
    sample_fit(pixels, fit, |sub_x, sub_y, color| {
        grid.set(sub_x, sub_y, color);
    });
}

fn sample_fit(pixels: &PixelGrid<'_>, fit: Rect, mut set: impl FnMut(u16, u16, Color)) {
    for row in 0..fit.height {
        let source_y = usize::from(row) * pixels.height / usize::from(fit.height);
        for column in 0..fit.width {
            let source_x = usize::from(column) * pixels.width / usize::from(fit.width);
            if let Some(color) = pixels.color_at(source_x, source_y) {
                set(fit.x + column, fit.y + row, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;

    use super::*;

    const RED: u32 = 0xff_00_00;
    const GREEN: u32 = 0x00_ff_00;
    const BLUE: u32 = 0x00_00_ff;
    const WHITE: u32 = 0xff_ff_ff;

    fn blit_into(buffer: &mut Buffer, pixels: &PixelGrid<'_>) {
        let mut grid = HalfblockGrid::new(buffer.area);
        let fit = letterbox((pixels.width, pixels.height), grid.subcell_area());
        blit_halfblocks(&mut grid, pixels, fit);
        grid.resolve_into(buffer);
    }

    #[test]
    fn quadrants_map_to_halfblock_cells() {
        let pixels = [RED, GREEN, BLUE, WHITE];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 2, 1));

        blit_into(&mut buffer, &PixelGrid::xrgb32(&pixels, 2, 2));

        let left = buffer.cell((0, 0)).unwrap();
        assert_eq!(left.symbol(), "▀");
        assert_eq!(left.fg, Color::Rgb(255, 0, 0));
        assert_eq!(left.bg, Color::Rgb(0, 0, 255));
        let right = buffer.cell((1, 0)).unwrap();
        assert_eq!(right.fg, Color::Rgb(0, 255, 0));
        assert_eq!(right.bg, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn cells_outside_the_fit_stay_untouched() {
        let pixels = [WHITE, WHITE, WHITE, WHITE];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 4, 1));

        blit_into(&mut buffer, &PixelGrid::xrgb32(&pixels, 2, 2));

        let bar = buffer.cell((0, 0)).unwrap();
        assert_eq!(bar.symbol(), " ");
        assert_eq!(bar.bg, Color::Reset);
        assert_eq!(buffer.cell((1, 0)).unwrap().fg, Color::Rgb(255, 255, 255));
        assert_eq!(buffer.cell((3, 0)).unwrap().bg, Color::Reset);
    }

    #[test]
    fn offset_areas_use_absolute_coordinates() {
        let pixels = [GREEN];
        let area = Rect::new(3, 2, 1, 1);
        let mut buffer = Buffer::empty(area);

        blit_into(&mut buffer, &PixelGrid::xrgb32(&pixels, 1, 1));

        assert_eq!(buffer.cell((3, 2)).unwrap().fg, Color::Rgb(0, 255, 0));
    }

    #[test]
    fn transparent_rgba_pixels_leave_their_subcells_unset() {
        let pixels: [u8; 8] = [255, 0, 0, 255, 0, 0, 255, 0];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));

        blit_into(&mut buffer, &PixelGrid::rgba8(&pixels, 1, 2));

        let cell = buffer.cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Reset);
    }

    #[test]
    fn xrgb_high_byte_is_ignored() {
        let pixels = [0xff_12_34_56u32];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));

        blit_into(&mut buffer, &PixelGrid::xrgb32(&pixels, 1, 1));

        assert_eq!(
            buffer.cell((0, 0)).unwrap().fg,
            Color::Rgb(0x12, 0x34, 0x56)
        );
    }

    #[test]
    fn blit_braille_fills_dots_and_skips_transparent_pixels() {
        let mut pixels = [255u8; 2 * 4 * 4];
        pixels[3] = 0;
        let source = PixelGrid::rgba8(&pixels, 2, 4);
        let mut grid = BrailleGrid::new(Rect::new(0, 0, 1, 1));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));

        let fit = grid.subcell_area();
        blit_braille(&mut grid, &source, fit);
        grid.resolve_into(&mut buffer);

        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "⣾");
    }

    #[test]
    fn write_rgba8_copies_rgba_sources_with_alpha_intact() {
        let pixels: [u8; 8] = [255, 0, 0, 255, 0, 0, 255, 0];
        let grid = PixelGrid::rgba8(&pixels, 1, 2);
        let mut out = Vec::new();

        grid.write_rgba8(&mut out);

        assert_eq!(out, pixels);
    }

    #[test]
    fn write_rgba8_makes_xrgb_sources_opaque() {
        let pixels = [0xff_12_34_56u32, RED];
        let grid = PixelGrid::xrgb32(&pixels, 2, 1);
        let mut out = Vec::new();

        grid.write_rgba8(&mut out);

        assert_eq!(out, [0x12, 0x34, 0x56, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn write_rgba8_reuses_the_buffer_across_sizes() {
        let large = [WHITE; 4];
        let small = [GREEN];
        let mut out = Vec::new();

        PixelGrid::xrgb32(&large, 2, 2).write_rgba8(&mut out);
        assert_eq!(out.len(), 16);
        PixelGrid::xrgb32(&small, 1, 1).write_rgba8(&mut out);

        assert_eq!(out, [0, 255, 0, 255]);
    }

    #[test]
    fn letterbox_fits_wide_source_by_width() {
        assert_eq!(
            letterbox((4, 2), Rect::new(0, 0, 4, 4)),
            Rect::new(0, 1, 4, 2)
        );
    }

    #[test]
    fn letterbox_fits_tall_source_by_height() {
        assert_eq!(
            letterbox((2, 2), Rect::new(0, 0, 10, 8)),
            Rect::new(1, 0, 8, 8)
        );
    }

    #[test]
    fn letterbox_offsets_by_the_target_origin() {
        assert_eq!(
            letterbox((2, 2), Rect::new(3, 4, 10, 8)),
            Rect::new(4, 4, 8, 8)
        );
    }

    #[test]
    fn letterbox_of_empty_input_is_empty() {
        assert_eq!(letterbox((0, 0), Rect::new(0, 0, 10, 8)), Rect::ZERO);
    }
}
