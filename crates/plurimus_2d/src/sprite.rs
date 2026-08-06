//! Sprite components rasterized by the 2d pipeline.

use bevy_ecs::prelude::Component;
use bevy_transform::components::Transform;
use ratatui_core::style::{Color, Style};

/// A one-cell sprite drawn at its transform's projected cell.
///
/// Multi-cell shapes compose from child entities; transform propagation
/// keeps them parent-relative. Z-ordering follows the global transform's
/// `z`, higher in front.
#[derive(Component, Debug, Clone)]
#[require(Transform)]
pub struct Glyph {
    /// Cell content; a single grapheme cluster.
    pub symbol: String,
    /// Style applied to the cell.
    pub style: Style,
}

impl Glyph {
    /// A glyph with the default style.
    #[must_use]
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            style: Style::new(),
        }
    }

    /// Sets the glyph's style.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

/// A rectangular text-art sprite stamped centered on its transform's
/// projected cell, one grapheme per cell.
///
/// `' '` cells are transparent - the world content beneath shows
/// through; spell opaque space with a fill character or a styled
/// backdrop entity. Wide graphemes occupy their display width in cells.
/// Z-ordering matches [`Glyph`].
#[derive(Component, Debug, Clone)]
#[require(Transform)]
pub struct GlyphBlock {
    /// Rows of cell content, top to bottom.
    pub rows: Vec<String>,
    /// Style applied to every stamped cell.
    pub style: Style,
}

impl GlyphBlock {
    /// A block from newline-separated text with the default style.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            rows: text.into().split('\n').map(String::from).collect(),
            style: Style::new(),
        }
    }

    /// Sets the block's style.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

/// A subcell point drawn at halfblock resolution, beneath all glyphs.
///
/// Z-ordering against other pixels and [`PixelBlock`]s follows the global
/// transform's `z`, higher in front.
#[derive(Component, Debug, Clone, Copy)]
#[require(Transform)]
pub struct Pixel {
    /// Color of the halfblock point.
    pub color: Color,
}

/// A rectangular pixel-art sprite stamped centered on its transform's
/// projected position, one bitmap pixel per subcell.
///
/// Each character of [`rows`](Self::rows) indexes
/// [`palette`](Self::palette); characters the palette does not map are
/// transparent, and short rows pad to the widest. Like [`GlyphBlock`] the
/// sprite is screen-space sized: one pixel per subcell of the camera's
/// [`SubcellMode`](crate::SubcellMode) rather than per world unit, so it
/// keeps its resolution as the camera zooms. Z-ordering matches [`Pixel`],
/// against which it interleaves.
#[derive(Component, Debug, Clone, Default)]
#[require(Transform)]
pub struct PixelBlock {
    /// Rows of palette-indexed pixels, top to bottom.
    pub rows: Vec<String>,
    /// Maps row characters to their color; unlisted characters are
    /// transparent.
    pub palette: Vec<(char, Color)>,
    /// Whether to stamp the bitmap mirrored horizontally.
    pub mirrored: bool,
}

impl PixelBlock {
    /// A block from newline-separated rows over `palette`.
    #[must_use]
    pub fn new(text: impl Into<String>, palette: impl Into<Vec<(char, Color)>>) -> Self {
        Self {
            rows: text.into().split('\n').map(String::from).collect(),
            palette: palette.into(),
            mirrored: false,
        }
    }

    /// Sets whether the bitmap stamps mirrored horizontally.
    #[must_use]
    pub fn mirrored(mut self, mirrored: bool) -> Self {
        self.mirrored = mirrored;
        self
    }
}
