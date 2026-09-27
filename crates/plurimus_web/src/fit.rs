//! Choosing the font size, and with it the grid, once at startup.

/// Font size used when no fit is stated, in CSS pixels.
const DEFAULT_FONT_PX: f32 = 16.0;

/// The smallest font size a fit will choose, in CSS pixels.
const MIN_FONT_PX: u16 = 4;

/// The largest font size the renderer can measure, in physical pixels: it
/// sizes a cell by drawing `█` 16 pixels into a 128-pixel scratch canvas,
/// so a taller glyph would be cut off and mismeasured.
const MAX_MEASURABLE_PX: f32 = 90.0;

/// How the font size is chosen, and so how many cells the canvas holds.
///
/// Evaluated once, when the backend is built: a later resize changes the
/// grid, not the font, because the glyph atlas is made for one size.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GridFit {
    /// The largest whole font size whose grid holds at least this many
    /// columns and rows - a minimum the app needs, such as a game board.
    Cells(u16, u16),
    /// The largest whole font size whose grid holds at least this many
    /// columns.
    Columns(u16),
    /// This font size, in CSS pixels.
    Px(f32),
}

impl Default for GridFit {
    fn default() -> Self {
        Self::Px(DEFAULT_FONT_PX)
    }
}

/// The area the canvas fills, in CSS pixels, and the display's pixel ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Surface {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) pixel_ratio: f64,
}

/// The font size, in CSS pixels, that `fit` asks for on `surface`.
///
/// `measure` sizes a cell in physical pixels at a physical font size, the
/// way the renderer will; a fit that no size meets settles on the smallest,
/// and the app's own size guard then says so.
pub(crate) fn font_px(
    fit: GridFit,
    surface: Surface,
    mut measure: impl FnMut(f32) -> Option<(u32, u32)>,
) -> f32 {
    let needed = match fit {
        GridFit::Px(px) => return px,
        GridFit::Cells(columns, rows) => (columns, rows),
        GridFit::Columns(columns) => (columns, 0),
    };
    let fits = |px: u16, measure: &mut dyn FnMut(f32) -> Option<(u32, u32)>| {
        grid_at(px, surface, measure)
            .is_some_and(|(columns, rows)| columns >= needed.0 && rows >= needed.1)
    };
    let largest = largest_measurable(surface.pixel_ratio);
    let (mut low, mut high) = (MIN_FONT_PX, largest);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if fits(middle, &mut measure) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    f32::from(low)
}

/// The grid a font of `px` CSS pixels gives `surface`.
fn grid_at(
    px: u16,
    surface: Surface,
    measure: &mut dyn FnMut(f32) -> Option<(u32, u32)>,
) -> Option<(u16, u16)> {
    let physical = f32::from(px) * surface.pixel_ratio as f32;
    let (cell_width, cell_height) = measure(physical)?;
    if cell_width == 0 || cell_height == 0 {
        return None;
    }
    let cells = |extent: f64, cell: u32| {
        (extent * surface.pixel_ratio / f64::from(cell))
            .floor()
            .clamp(0.0, f64::from(u16::MAX)) as u16
    };
    Some((
        cells(surface.width, cell_width),
        cells(surface.height, cell_height),
    ))
}

/// The alpha at or above which a scratch-canvas pixel counts as inked,
/// matching the renderer's own threshold.
const INK_ALPHA: u8 = 128;

/// Bytes per pixel in canvas image data.
const RGBA: usize = 4;

/// The inked width and height of what was drawn on a scratch canvas `side`
/// pixels square, from its RGBA bytes; `None` when nothing inked.
pub(crate) fn inked_size(rgba: &[u8], side: u32) -> Option<(u32, u32)> {
    let side = side as usize;
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (index, pixel) in rgba.as_chunks::<RGBA>().0.iter().enumerate() {
        if pixel[RGBA - 1] < INK_ALPHA {
            continue;
        }
        let (x, y) = (index % side, index / side);
        bounds = Some(bounds.map_or((x, y, x, y), |(left, top, right, bottom)| {
            (left.min(x), top.min(y), right.max(x), bottom.max(y))
        }));
    }
    bounds.map(|(left, top, right, bottom)| ((right - left + 1) as u32, (bottom - top + 1) as u32))
}

fn largest_measurable(pixel_ratio: f64) -> u16 {
    let largest = (f64::from(MAX_MEASURABLE_PX) / pixel_ratio.max(1.0)).floor() as u16;
    largest.max(MIN_FONT_PX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A font whose cell is 0.5 by 1.0 of its size, rounded as pixels are.
    fn half_width(px: f32) -> (u32, u32) {
        ((px * 0.5).round() as u32, px.round() as u32)
    }

    /// Like the renderer's scan, a glyph too small to leave a pixel
    /// measures nothing.
    fn measured(px: f32) -> Option<(u32, u32)> {
        (px >= 1.0).then(|| half_width(px))
    }

    const WINDOW: Surface = Surface {
        width: 1600.0,
        height: 900.0,
        pixel_ratio: 1.0,
    };

    #[test]
    fn inked_size_spans_only_opaque_enough_pixels() {
        let side = 4;
        let mut rgba = vec![0_u8; side * side * RGBA];
        let mut ink = |x: usize, y: usize, alpha: u8| rgba[(y * side + x) * RGBA + 3] = alpha;
        ink(1, 1, 255);
        ink(2, 3, 128);
        ink(3, 0, 127);
        assert_eq!(inked_size(&rgba, side as u32), Some((2, 3)));
        assert_eq!(inked_size(&vec![0; side * side * RGBA], side as u32), None);
    }

    #[test]
    fn a_stated_size_is_taken_as_is() {
        assert_eq!(font_px(GridFit::Px(13.5), WINDOW, measured), 13.5);
    }

    #[test]
    fn cells_fit_picks_the_largest_size_that_holds_them() {
        let px = font_px(GridFit::Cells(280, 76), WINDOW, measured);
        let (columns, rows) = grid_at(px as u16, WINDOW, &mut measured).unwrap();
        assert!(columns >= 280 && rows >= 76, "{columns}x{rows} at {px}");
        let (columns, rows) = grid_at(px as u16 + 1, WINDOW, &mut measured).unwrap();
        assert!(columns < 280 || rows < 76, "one size larger still fits");
    }

    #[test]
    fn columns_fit_ignores_rows() {
        let px = font_px(GridFit::Columns(200), WINDOW, measured);
        assert_eq!(px, 16.0);
    }

    #[test]
    fn a_denser_display_still_holds_the_cells() {
        let retina = Surface {
            pixel_ratio: 2.0,
            ..WINDOW
        };
        let px = font_px(GridFit::Cells(280, 76), retina, measured);
        let (columns, rows) = grid_at(px as u16, retina, &mut measured).unwrap();
        assert!(columns >= 280 && rows >= 76, "{columns}x{rows} at {px}");
    }

    #[test]
    fn an_unmeetable_fit_settles_on_the_smallest_size() {
        let px = font_px(GridFit::Cells(10_000, 10_000), WINDOW, measured);
        assert_eq!(px, f32::from(MIN_FONT_PX));
    }

    #[test]
    fn sizes_past_what_can_be_measured_are_never_chosen() {
        let px = font_px(GridFit::Cells(1, 1), WINDOW, measured);
        assert_eq!(px, MAX_MEASURABLE_PX);
    }
}
