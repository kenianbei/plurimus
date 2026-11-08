//! Copies 2d views and sprites into the render world.
//!
//! Each frame every drawable is flattened into a form the rasterizer can walk
//! without touching the main world again: glyphs and glyph blocks become rows
//! of text, pixels and pixel blocks become colored cells in a shared arena.
//! Collapsing the single and block cases into one shape here is what lets the
//! rasterizer sort and draw them in a single pass.

use std::ops::Range;

use bevy_camera::visibility::RenderLayers;
use bevy_camera::{Camera, Camera2d};
use bevy_ecs::prelude::{Entity, Query, Res, ResMut, Resource, With};
use bevy_math::{Vec2, Vec3};
use bevy_transform::components::GlobalTransform;
use plurimus_core::{CellSize, Extract, MainEntity};
use ratatui_core::style::{Color, Style};

use crate::projection::SubcellMode;
use crate::sprite::{Glyph, GlyphBlock, Pixel, PixelBlock};

/// Per-camera 2d views, rebuilt every frame.
#[derive(Resource, Default)]
pub(crate) struct ExtractedViews2d(pub(crate) Vec<ExtractedView2d>);

/// A 2d camera's main-world entity and what projecting through it needs.
pub(crate) struct ExtractedView2d {
    pub(crate) entity: MainEntity,
    pub(crate) camera: Camera,
    pub(crate) transform: GlobalTransform,
    pub(crate) cell: Vec2,
    pub(crate) layers: RenderLayers,
    pub(crate) mode: SubcellMode,
}

/// All 2d sprites, rebuilt every frame.
#[derive(Resource, Default)]
pub(crate) struct ExtractedSprites2d {
    pub(crate) glyphs: Vec<ExtractedGlyph>,
    pub(crate) pixels: Vec<ExtractedPixelBlock>,
    /// Row-major backing store the pixel blocks index into; sorting the
    /// blocks leaves their ranges valid.
    pub(crate) pixel_cells: Vec<Option<Color>>,
}

/// A glyph or glyph block, uniformly block-shaped: a `Glyph` extracts
/// as a single-row block.
pub(crate) struct ExtractedGlyph {
    pub(crate) position: Vec3,
    pub(crate) entity: MainEntity,
    pub(crate) rows: Vec<String>,
    pub(crate) style: Style,
    pub(crate) layers: RenderLayers,
}

/// A pixel or pixel block, uniformly block-shaped: a `Pixel` extracts as
/// a 1×1 block.
pub(crate) struct ExtractedPixelBlock {
    pub(crate) position: Vec3,
    pub(crate) entity: MainEntity,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) cells: Range<usize>,
    pub(crate) layers: RenderLayers,
}

type Placed<'w, 's, T> = Query<
    'w,
    's,
    (
        Entity,
        &'static T,
        &'static GlobalTransform,
        Option<&'static RenderLayers>,
    ),
>;

pub(crate) fn extract_views_2d(
    cameras: Extract<
        Query<
            (
                Entity,
                &Camera,
                &GlobalTransform,
                Option<&RenderLayers>,
                Option<&SubcellMode>,
            ),
            With<Camera2d>,
        >,
    >,
    cell: Extract<Res<CellSize>>,
    mut views: ResMut<ExtractedViews2d>,
) {
    views.0.clear();
    for (entity, camera, transform, layers, mode) in &cameras {
        views.0.push(ExtractedView2d {
            entity: MainEntity::from(entity),
            camera: camera.clone(),
            transform: *transform,
            cell: cell.vec(),
            layers: layers.cloned().unwrap_or_default(),
            mode: mode.copied().unwrap_or_default(),
        });
    }
}

pub(crate) fn extract_sprites_2d(
    glyphs: Extract<Placed<Glyph>>,
    glyph_blocks: Extract<Placed<GlyphBlock>>,
    pixels: Extract<Placed<Pixel>>,
    pixel_blocks: Extract<Placed<PixelBlock>>,
    mut sprites: ResMut<ExtractedSprites2d>,
) {
    sprites.glyphs.clear();
    sprites.pixels.clear();
    sprites.pixel_cells.clear();
    extract_glyphs(&glyphs, &mut sprites);
    extract_glyph_blocks(&glyph_blocks, &mut sprites);
    extract_pixels(&pixels, &mut sprites);
    extract_pixel_blocks(&pixel_blocks, &mut sprites);
}

fn extract_glyphs(glyphs: &Placed<Glyph>, sprites: &mut ExtractedSprites2d) {
    for (entity, glyph, transform, layers) in glyphs {
        sprites.glyphs.push(ExtractedGlyph {
            position: transform.translation(),
            entity: MainEntity::from(entity),
            rows: vec![glyph.symbol.clone()],
            style: glyph.style,
            layers: layers.cloned().unwrap_or_default(),
        });
    }
}

fn extract_glyph_blocks(blocks: &Placed<GlyphBlock>, sprites: &mut ExtractedSprites2d) {
    for (entity, block, transform, layers) in blocks {
        sprites.glyphs.push(ExtractedGlyph {
            position: transform.translation(),
            entity: MainEntity::from(entity),
            rows: block.rows.clone(),
            style: block.style,
            layers: layers.cloned().unwrap_or_default(),
        });
    }
}

fn extract_pixels(pixels: &Placed<Pixel>, sprites: &mut ExtractedSprites2d) {
    for (entity, pixel, transform, layers) in pixels {
        let start = sprites.pixel_cells.len();
        sprites.pixel_cells.push(Some(pixel.color));
        sprites.pixels.push(ExtractedPixelBlock {
            position: transform.translation(),
            entity: MainEntity::from(entity),
            width: 1,
            height: 1,
            cells: start..sprites.pixel_cells.len(),
            layers: layers.cloned().unwrap_or_default(),
        });
    }
}

fn extract_pixel_blocks(blocks: &Placed<PixelBlock>, sprites: &mut ExtractedSprites2d) {
    for (entity, block, transform, layers) in blocks {
        let start = sprites.pixel_cells.len();
        let width = resolve_pixels(block, &mut sprites.pixel_cells);
        sprites.pixels.push(ExtractedPixelBlock {
            position: transform.translation(),
            entity: MainEntity::from(entity),
            width,
            height: block.rows.len(),
            cells: start..sprites.pixel_cells.len(),
            layers: layers.cloned().unwrap_or_default(),
        });
    }
}

/// Appends `block`'s rows to `cells` as row-major colors padded to the
/// widest row, mirrored if the block is, and returns that width.
fn resolve_pixels(block: &PixelBlock, cells: &mut Vec<Option<Color>>) -> usize {
    let width = block
        .rows
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap_or(0);
    for row in &block.rows {
        let start = cells.len();
        cells.extend(row.chars().map(|symbol| palette_color(block, symbol)));
        cells.resize(start + width, None);
        if block.mirrored {
            cells[start..start + width].reverse();
        }
    }
    width
}

fn palette_color(block: &PixelBlock, symbol: char) -> Option<Color> {
    block
        .palette
        .iter()
        .find(|(key, _)| *key == symbol)
        .map(|(_, color)| *color)
}
