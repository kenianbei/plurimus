//! Renders extracted 2d sprites into their camera buffers.
//!
//! Pixels are drawn beneath glyphs, each group sorted by transform `z` with
//! the entity as tie-break so equal depths stay stable between frames. Pixels
//! go through the camera's subcell grid and glyphs are stamped straight into
//! the cell buffer, which is why a glyph always covers a pixel sharing its
//! cell regardless of depth.

use bevy_ecs::prelude::{Local, Query, Res, ResMut};
use bevy_math::FloatOrd;
use plurimus_core::raster::{BrailleGrid, HalfblockGrid};
use plurimus_core::{CameraBuffer, MainEntity, camera_buffer_mut};
use ratatui_core::buffer::{Buffer, Cell};
use ratatui_core::style::{Color, Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::extract::{
    ExtractedGlyph, ExtractedPixelBlock, ExtractedSprites2d, ExtractedView2d, ExtractedViews2d,
};
use crate::projection::SubcellMode;

const TRANSPARENT_CELL: &str = " ";

/// Per-mode scratch grids reused across frames.
#[derive(Default)]
pub(crate) struct SubcellGrids {
    halfblock: HalfblockGrid,
    braille: BrailleGrid,
}

pub(crate) fn rasterize_2d(
    views: Res<ExtractedViews2d>,
    mut sprites: ResMut<ExtractedSprites2d>,
    mut cameras: Query<(MainEntity, &mut CameraBuffer)>,
    mut grids: Local<SubcellGrids>,
) {
    sprites
        .glyphs
        .sort_by_key(|glyph| (FloatOrd(glyph.position.z), glyph.entity));
    sprites
        .pixels
        .sort_by_key(|block| (FloatOrd(block.position.z), block.entity));
    let sprites = sprites.into_inner();
    for view in &views.0 {
        let Some(mut buffer) = camera_buffer_mut(&mut cameras, view.entity) else {
            continue;
        };
        draw_pixels(view, sprites, &mut grids, &mut buffer.0);
        draw_glyphs(view, &sprites.glyphs, &mut buffer.0);
    }
}

fn draw_pixels(
    camera: &ExtractedView2d,
    sprites: &ExtractedSprites2d,
    grids: &mut SubcellGrids,
    buffer: &mut Buffer,
) {
    if sprites.pixels.is_empty() {
        return;
    }
    match camera.mode {
        SubcellMode::Halfblock => {
            draw_halfblock_pixels(camera, sprites, &mut grids.halfblock, buffer);
        }
        SubcellMode::Braille => draw_braille_pixels(camera, sprites, &mut grids.braille, buffer),
    }
}

fn draw_halfblock_pixels(
    camera: &ExtractedView2d,
    sprites: &ExtractedSprites2d,
    grid: &mut HalfblockGrid,
    buffer: &mut Buffer,
) {
    grid.reset(buffer.area);
    for block in &sprites.pixels {
        if !camera.layers.intersects(&block.layers) {
            continue;
        }
        let Some(anchor) = camera.project_halfblock(block.position) else {
            continue;
        };
        for (x, sub_y, color) in block_pixels(&sprites.pixel_cells, block, anchor) {
            grid.set(x, sub_y, color);
        }
    }
    grid.resolve_into(buffer);
}

fn draw_braille_pixels(
    camera: &ExtractedView2d,
    sprites: &ExtractedSprites2d,
    grid: &mut BrailleGrid,
    buffer: &mut Buffer,
) {
    grid.reset(buffer.area);
    for block in &sprites.pixels {
        if !camera.layers.intersects(&block.layers) {
            continue;
        }
        let Some(anchor) = camera.project_braille(block.position) else {
            continue;
        };
        for (sub_x, sub_y, color) in block_pixels(&sprites.pixel_cells, block, anchor) {
            grid.set(sub_x, sub_y, color);
        }
    }
    grid.resolve_into(buffer);
}

/// Places a block's pixels around its anchor subcell, dropping
/// transparent cells and coordinates outside the subcell space; the grids
/// clip the rest.
fn block_pixels<'a>(
    cells: &'a [Option<Color>],
    block: &ExtractedPixelBlock,
    anchor: (i32, i32),
) -> impl Iterator<Item = (u16, u16, Color)> + 'a {
    let width = block.width;
    let origin_x = anchor.0 - width as i32 / 2;
    let origin_y = anchor.1 - block.height as i32 / 2;
    cells[block.cells.clone()]
        .iter()
        .enumerate()
        .filter_map(move |(index, cell)| {
            let color = (*cell)?;
            let x = u16::try_from(origin_x + (index % width) as i32).ok()?;
            let y = u16::try_from(origin_y + (index / width) as i32).ok()?;
            Some((x, y, color))
        })
}

fn draw_glyphs(camera: &ExtractedView2d, glyphs: &[ExtractedGlyph], buffer: &mut Buffer) {
    for glyph in glyphs {
        if !camera.layers.intersects(&glyph.layers) {
            continue;
        }
        let Some((anchor_column, anchor_row)) = camera.project_cell(glyph.position) else {
            continue;
        };
        let origin_column = anchor_column - block_width(&glyph.rows) / 2;
        let origin_row = anchor_row - glyph.rows.len() as i32 / 2;
        for (offset, row) in glyph.rows.iter().enumerate() {
            stamp_row(
                buffer,
                row,
                glyph.style,
                (origin_column, origin_row + offset as i32),
            );
        }
    }
}

fn block_width(rows: &[String]) -> i32 {
    rows.iter()
        .map(|row| row.graphemes(true).map(grapheme_width).sum())
        .max()
        .unwrap_or(0)
}

fn grapheme_width(grapheme: &str) -> i32 {
    grapheme.width() as i32
}

fn stamp_row(buffer: &mut Buffer, row: &str, style: Style, origin: (i32, i32)) {
    let (mut column, row_index) = origin;
    for grapheme in row.graphemes(true) {
        let width = grapheme_width(grapheme);
        if width == 0 {
            continue;
        }
        if grapheme != TRANSPARENT_CELL {
            stamp_grapheme(buffer, grapheme, style, (column, row_index));
        }
        column += width;
    }
}

fn stamp_grapheme(buffer: &mut Buffer, grapheme: &str, style: Style, position: (i32, i32)) {
    let (column, row) = position;
    let Some(cell) = cell_at(buffer, column, row) else {
        return;
    };
    cell.set_symbol(grapheme);
    cell.set_style(style);
    for continuation in column + 1..column + grapheme_width(grapheme) {
        if let Some(hidden) = cell_at(buffer, continuation, row) {
            hidden.reset();
        }
    }
}

fn cell_at(buffer: &mut Buffer, column: i32, row: i32) -> Option<&mut Cell> {
    buffer.cell_mut((u16::try_from(column).ok()?, u16::try_from(row).ok()?))
}
