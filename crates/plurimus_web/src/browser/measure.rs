//! Sizing a font's cell the way the renderer will, before it exists.

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use super::document;
use crate::fit::{DRAW_OFFSET, SCRATCH_SIDE, inked_size};

/// The glyph whose inked box is a cell.
const REFERENCE_GLYPH: &str = "█";

/// A scratch canvas that measures cells for one font family.
pub(crate) struct CellMeter {
    context: CanvasRenderingContext2d,
    family: String,
}

impl CellMeter {
    pub(crate) fn new(family: &str) -> Option<Self> {
        let canvas: HtmlCanvasElement =
            document().create_element("canvas").ok()?.dyn_into().ok()?;
        canvas.set_width(SCRATCH_SIDE);
        canvas.set_height(SCRATCH_SIDE);
        let context: CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;
        context.set_text_baseline("top");
        context.set_text_align("left");
        context.set_fill_style_str("white");
        Some(Self {
            context,
            family: family.to_owned(),
        })
    }

    /// The cell, in physical pixels, of a font `px` physical pixels tall.
    pub(crate) fn measure(&self, px: f32) -> Option<(u32, u32)> {
        let side = f64::from(SCRATCH_SIDE);
        let context = &self.context;
        context.clear_rect(0.0, 0.0, side, side);
        context.set_font(&format!("{px}px '{}', monospace", self.family));
        context
            .fill_text(REFERENCE_GLYPH, DRAW_OFFSET, DRAW_OFFSET)
            .ok()?;
        let image = context.get_image_data(0.0, 0.0, side, side).ok()?;
        inked_size(&image.data(), SCRATCH_SIDE)
    }
}
