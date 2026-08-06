//! Copies 2d views and sprites into the render world.
//!
//! Each frame every drawable is flattened into a form the rasterizer can walk
//! without touching the main world again: glyphs and glyph blocks become rows
//! of text, pixels and pixel blocks become colored cells in a shared arena.
//! Collapsing the single and block cases into one shape here is what lets the
//! rasterizer sort and draw them in a single pass.

use std::ops::Range;

use bevy_ecs::prelude::{Entity, ResMut, Resource};
use bevy_math::Vec2;
use bevy_transform::components::GlobalTransform;
use plurimus_core::MainWorld;
use ratatui_core::style::{Color, Style};

use crate::layers::RenderLayers;
use crate::projection::{Projection2d, SubcellMode};
use crate::sprite::{Glyph, GlyphBlock, Pixel, PixelBlock};

/// Per-camera 2d views, rebuilt every frame.
#[derive(Resource, Default)]
pub(crate) struct ExtractedViews2d(pub(crate) Vec<ExtractedView2d>);

/// The main-world camera entity and its projection state.
pub(crate) struct ExtractedView2d {
    pub(crate) camera: Entity,
    pub(crate) center: Vec2,
    pub(crate) projection: Projection2d,
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
    pub(crate) position: Vec2,
    pub(crate) z: f32,
    pub(crate) entity: Entity,
    pub(crate) rows: Vec<String>,
    pub(crate) style: Style,
    pub(crate) layers: RenderLayers,
}

/// A pixel or pixel block, uniformly block-shaped: a `Pixel` extracts as
/// a 1×1 block.
pub(crate) struct ExtractedPixelBlock {
    pub(crate) position: Vec2,
    pub(crate) z: f32,
    pub(crate) entity: Entity,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) cells: Range<usize>,
    pub(crate) layers: RenderLayers,
}

pub(crate) fn extract_views_2d(
    mut main_world: ResMut<MainWorld>,
    mut views: ResMut<ExtractedViews2d>,
) {
    views.0.clear();
    let mut cameras = main_world.query::<(
        Entity,
        &Projection2d,
        &GlobalTransform,
        Option<&RenderLayers>,
        Option<&SubcellMode>,
    )>();
    for (camera, projection, transform, layers, mode) in cameras.iter(&main_world) {
        views.0.push(ExtractedView2d {
            camera,
            center: transform.translation().truncate(),
            projection: *projection,
            layers: layers.copied().unwrap_or_default(),
            mode: mode.copied().unwrap_or_default(),
        });
    }
}

pub(crate) fn extract_sprites_2d(
    mut main_world: ResMut<MainWorld>,
    mut sprites: ResMut<ExtractedSprites2d>,
) {
    sprites.glyphs.clear();
    sprites.pixels.clear();
    sprites.pixel_cells.clear();
    extract_glyphs(&mut main_world, &mut sprites);
    extract_glyph_blocks(&mut main_world, &mut sprites);
    extract_pixels(&mut main_world, &mut sprites);
    extract_pixel_blocks(&mut main_world, &mut sprites);
}

fn extract_glyphs(main_world: &mut MainWorld, sprites: &mut ExtractedSprites2d) {
    let mut glyphs =
        main_world.query::<(Entity, &Glyph, &GlobalTransform, Option<&RenderLayers>)>();
    for (entity, glyph, transform, layers) in glyphs.iter(main_world) {
        let translation = transform.translation();
        sprites.glyphs.push(ExtractedGlyph {
            position: translation.truncate(),
            z: translation.z,
            entity,
            rows: vec![glyph.symbol.clone()],
            style: glyph.style,
            layers: layers.copied().unwrap_or_default(),
        });
    }
}

fn extract_glyph_blocks(main_world: &mut MainWorld, sprites: &mut ExtractedSprites2d) {
    let mut blocks =
        main_world.query::<(Entity, &GlyphBlock, &GlobalTransform, Option<&RenderLayers>)>();
    for (entity, block, transform, layers) in blocks.iter(main_world) {
        let translation = transform.translation();
        sprites.glyphs.push(ExtractedGlyph {
            position: translation.truncate(),
            z: translation.z,
            entity,
            rows: block.rows.clone(),
            style: block.style,
            layers: layers.copied().unwrap_or_default(),
        });
    }
}

fn extract_pixels(main_world: &mut MainWorld, sprites: &mut ExtractedSprites2d) {
    let mut pixels =
        main_world.query::<(Entity, &Pixel, &GlobalTransform, Option<&RenderLayers>)>();
    for (entity, pixel, transform, layers) in pixels.iter(main_world) {
        let translation = transform.translation();
        let start = sprites.pixel_cells.len();
        sprites.pixel_cells.push(Some(pixel.color));
        sprites.pixels.push(ExtractedPixelBlock {
            position: translation.truncate(),
            z: translation.z,
            entity,
            width: 1,
            height: 1,
            cells: start..sprites.pixel_cells.len(),
            layers: layers.copied().unwrap_or_default(),
        });
    }
}

fn extract_pixel_blocks(main_world: &mut MainWorld, sprites: &mut ExtractedSprites2d) {
    let mut blocks =
        main_world.query::<(Entity, &PixelBlock, &GlobalTransform, Option<&RenderLayers>)>();
    for (entity, block, transform, layers) in blocks.iter(main_world) {
        let translation = transform.translation();
        let start = sprites.pixel_cells.len();
        let width = resolve_pixels(block, &mut sprites.pixel_cells);
        sprites.pixels.push(ExtractedPixelBlock {
            position: translation.truncate(),
            z: translation.z,
            entity,
            width,
            height: block.rows.len(),
            cells: start..sprites.pixel_cells.len(),
            layers: layers.copied().unwrap_or_default(),
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
