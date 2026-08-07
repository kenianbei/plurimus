//! Paints extracted `bevy_ui` nodes into camera buffers.
//!
//! Runs in the ui pass ahead of every widget, so a `bevy_ui` tree always sits
//! beneath widget content in the same camera. Each node paints in the order
//! extraction settled - shadow, background, border, then text - and each
//! layer is clipped to the node's inherited clip before a cell is touched, so
//! nothing outside a scroll container is ever written.

use bevy_color::LinearRgba;
use bevy_ecs::prelude::{Query, Res};
use bevy_math::Vec2;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::ratatui_core::style::Color;
use plurimus_core::{CameraBuffer, SourceCamera, camera_buffer_mut};

use plurimus_core::raster::linear_cell_color;

use super::decorate::{dim_toward, sample_gradients};
use super::extract::{ExtractedBuiNode, ExtractedBuiNodes, TextRun};
use super::text::wrap_spans;

pub(crate) fn rasterize_bui(
    nodes: Res<ExtractedBuiNodes>,
    mut cameras: Query<(&SourceCamera, &mut CameraBuffer)>,
) {
    for node in &nodes.0 {
        let Some(mut buffer) = camera_buffer_mut(&mut cameras, node.camera) else {
            continue;
        };
        paint(node, &mut buffer.0);
    }
}

fn paint(node: &ExtractedBuiNode, buffer: &mut Buffer) {
    draw_shadows(node, buffer);
    let bounds = clipped(node.rect, buffer.area, node.clip);
    if bounds.is_empty() {
        return;
    }
    if let Some(background) = node.background {
        fill(buffer, bounds, background);
    }
    draw_gradient_background(node, buffer, bounds);
    BorderPainter { node, bounds }.paint(buffer);
    if let Some(text) = &node.text {
        draw_text(buffer, bounds, text);
    }
}

fn fill(buffer: &mut Buffer, area: Rect, color: Color) {
    for position in area.positions() {
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_symbol(" ");
            cell.set_bg(color);
        }
    }
}

fn clipped(rect: Rect, area: Rect, clip: Option<Rect>) -> Rect {
    let bounds = rect.intersection(area);
    clip.map_or(bounds, |clip| bounds.intersection(clip))
}

// Shadows land outside the node rect, so they use their own bounds.
fn draw_shadows(node: &ExtractedBuiNode, buffer: &mut Buffer) {
    for shadow in &node.decoration.shadows {
        let area = clipped(shadow.rect, buffer.area, node.clip);
        for position in area.positions() {
            if !node.rect.contains(position) {
                dim_cell(buffer, position, shadow.color);
            }
        }
    }
}

fn dim_cell(buffer: &mut Buffer, position: Position, color: LinearRgba) {
    let Some(cell) = buffer.cell_mut(position) else {
        return;
    };
    let existing = match cell.bg {
        Color::Rgb(red, green, blue) => Some([red, green, blue]),
        _ => None,
    };
    cell.set_bg(dim_toward(existing, color));
}

fn draw_gradient_background(node: &ExtractedBuiNode, buffer: &mut Buffer, bounds: Rect) {
    if node.decoration.background_gradients.is_empty() {
        return;
    }
    for position in bounds.positions() {
        let point = cell_center(node.rect, position);
        let Some(color) = sample_gradients(&node.decoration.background_gradients, point) else {
            continue;
        };
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_symbol(" ");
            cell.set_bg(linear_cell_color(color));
        }
    }
}

fn cell_center(rect: Rect, position: Position) -> Vec2 {
    Vec2::new(
        f32::from(position.x.saturating_sub(rect.x)) + 0.5,
        f32::from(position.y.saturating_sub(rect.y)) + 0.5,
    )
}

struct BorderPainter<'a> {
    node: &'a ExtractedBuiNode,
    bounds: Rect,
}

impl BorderPainter<'_> {
    fn paint(&self, buffer: &mut Buffer) {
        let Some(sides) = &self.node.border else {
            return;
        };
        let rect = self.node.rect;
        let (left, right) = (rect.left(), rect.right().saturating_sub(1));
        let (top, bottom) = (rect.top(), rect.bottom().saturating_sub(1));
        for x in rect.left()..rect.right() {
            let at_edge = (x == left, x == right);
            self.put(buffer, (x, top), self.symbol(at_edge, true), sides.top);
            self.put(
                buffer,
                (x, bottom),
                self.symbol(at_edge, false),
                sides.bottom,
            );
        }
        for y in rect.top()..rect.bottom() {
            if y == top || y == bottom {
                continue;
            }
            self.put(buffer, (left, y), "│", sides.left);
            self.put(buffer, (right, y), "│", sides.right);
        }
    }

    fn symbol(&self, (at_left, at_right): (bool, bool), at_top: bool) -> &'static str {
        let corners = &self.node.decoration.corners;
        let pick = |rounded: bool, round, square| if rounded { round } else { square };
        match (at_left, at_right, at_top) {
            (true, false, true) => pick(corners.top_left, "╭", "┌"),
            (false, true, true) => pick(corners.top_right, "╮", "┐"),
            (true, false, false) => pick(corners.bottom_left, "╰", "└"),
            (false, true, false) => pick(corners.bottom_right, "╯", "┘"),
            _ => "─",
        }
    }

    fn put(&self, buffer: &mut Buffer, (x, y): (u16, u16), symbol: &str, color: Option<Color>) {
        let Some(color) = color else {
            return;
        };
        if !self.bounds.contains((x, y).into()) {
            return;
        }
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_symbol(symbol);
            cell.set_fg(self.cell_color((x, y), color));
        }
    }

    fn cell_color(&self, (x, y): (u16, u16), side: Color) -> Color {
        let gradients = &self.node.decoration.border_gradients;
        if gradients.is_empty() {
            return side;
        }
        sample_gradients(gradients, cell_center(self.node.rect, Position::new(x, y)))
            .map_or(side, linear_cell_color)
    }
}

fn draw_text(buffer: &mut Buffer, bounds: Rect, text: &TextRun) {
    let lines = wrap_spans(&text.spans, Some(text.content.width as usize));
    for (row, line) in lines.iter().enumerate() {
        let y = text.content.y.saturating_add(row as u16);
        if y >= text.content.bottom() || y < bounds.top() || y >= bounds.bottom() {
            continue;
        }
        let end = text.content.right().min(bounds.right());
        let mut x = text.content.x.max(bounds.left());
        for (content, style) in line {
            if x >= end {
                break;
            }
            (x, _) = buffer.set_stringn(x, y, content, usize::from(end - x), *style);
        }
    }
}
