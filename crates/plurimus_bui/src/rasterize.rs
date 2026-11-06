//! Paints extracted `bevy_ui` nodes into camera buffers.
//!
//! Nodes under no widget paint in the ui pass ahead of every widget; the
//! nodes a widget frames paint as its [`FramedNodes`], in the widget sort
//! directly after it. Each node paints in the order extraction settled -
//! shadow, background, border, then text - and each layer is clipped to the
//! node's inherited clip before a cell is touched, so nothing outside a
//! scroll container is ever written.

use bevy_color::LinearRgba;
use bevy_ecs::prelude::{Query, Res};
use bevy_math::Vec2;
use bevy_text::Justify;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::ratatui_core::style::{Color, Modifier};
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_core::{CameraBuffer, SourceCamera, camera_buffer_mut};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use plurimus_core::raster::linear_cell_color;

use super::decorate::{ResolvedGradient, RoundedCorners, dim_toward, sample_gradients};
use super::extract::{BorderSides, ExtractedBuiNode, ExtractedBuiNodes, TextRun};
use super::rect::CellBox;
use super::text::{Seg, line_width, wrap_spans};

pub(crate) fn rasterize_bui(
    nodes: Res<ExtractedBuiNodes>,
    mut cameras: Query<(&SourceCamera, &mut CameraBuffer)>,
) {
    for node in &nodes.nodes {
        let Some(mut buffer) = camera_buffer_mut(&mut cameras, node.camera) else {
            continue;
        };
        paint(node, &mut buffer.0, nodes.cell);
    }
}

/// The nodes one widget frames, drawn directly after it as a widget of
/// their own. The area handed in is the whole camera, which the nodes' own
/// rects and clips already address.
pub(crate) struct FramedNodes {
    pub(crate) nodes: Vec<ExtractedBuiNode>,
    pub(crate) cell: Vec2,
}

impl Widget for &FramedNodes {
    fn render(self, _: Rect, buffer: &mut Buffer) {
        for node in &self.nodes {
            paint(node, buffer, self.cell);
        }
    }
}

fn paint(node: &ExtractedBuiNode, buffer: &mut Buffer, cell: Vec2) {
    draw_shadows(node, buffer);
    let bounds = clipped(node.rect, buffer.area, node.clip);
    if bounds.is_empty() {
        return;
    }
    if let Some(background) = node.background {
        fill(buffer, bounds, background);
    }
    draw_gradient_background(node, buffer, bounds, cell);
    if let Some(sides) = &node.border {
        BorderPainter {
            rect: node.rect,
            sides,
            corners: node.decoration.corners,
            gradients: &node.decoration.border_gradients,
            bounds,
            cell,
        }
        .paint(buffer);
    }
    if let Some(ring) = &node.outline {
        let color = Some(ring.color);
        BorderPainter {
            rect: ring.rect,
            sides: &BorderSides {
                top: color,
                right: color,
                bottom: color,
                left: color,
            },
            corners: RoundedCorners::default(),
            gradients: &[],
            bounds: clipped(ring.rect, buffer.area, node.clip),
            cell,
        }
        .paint(buffer);
    }
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

fn clipped(rect: CellBox, area: Rect, clip: Option<Rect>) -> Rect {
    let bounds = rect.clamped(area);
    clip.map_or(bounds, |clip| bounds.intersection(clip))
}

// Shadows land outside the node rect, so they use their own bounds.
fn draw_shadows(node: &ExtractedBuiNode, buffer: &mut Buffer) {
    let node_cells = node.rect.clamped(buffer.area);
    for shadow in &node.decoration.shadows {
        let area = clipped(shadow.rect, buffer.area, node.clip);
        for position in area.positions() {
            if !node_cells.contains(position) {
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

fn draw_gradient_background(
    node: &ExtractedBuiNode,
    buffer: &mut Buffer,
    bounds: Rect,
    cell: Vec2,
) {
    if node.decoration.background_gradients.is_empty() {
        return;
    }
    for position in bounds.positions() {
        let point = cell_center(node.rect, position, cell);
        let Some(color) = sample_gradients(&node.decoration.background_gradients, point) else {
            continue;
        };
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_symbol(" ");
            cell.set_bg(linear_cell_color(color));
        }
    }
}

// Gradients are laid out in pixels, so a cell samples at its center in them.
fn cell_center(rect: CellBox, position: Position, cell: Vec2) -> Vec2 {
    Vec2::new(
        (i32::from(position.x) - rect.left) as f32 + 0.5,
        (i32::from(position.y) - rect.top) as f32 + 0.5,
    ) * cell
}

/// Draws a ring of line glyphs along `rect`'s edges: a node's border, or
/// its outline, which is a square border with no gradient.
struct BorderPainter<'a> {
    rect: CellBox,
    sides: &'a BorderSides,
    corners: RoundedCorners,
    gradients: &'a [ResolvedGradient],
    bounds: Rect,
    cell: Vec2,
}

impl BorderPainter<'_> {
    fn paint(&self, buffer: &mut Buffer) {
        let (sides, rect) = (self.sides, self.rect);
        let (left, right) = (rect.left, rect.right - 1);
        let (top, bottom) = (rect.top, rect.bottom - 1);
        // A corner needs both of its sides; a lone side runs end to end.
        for x in (self.bounds.left()..self.bounds.right()).map(i32::from) {
            let at_corner = (
                x == left && sides.left.is_some(),
                x == right && sides.right.is_some(),
            );
            self.put(buffer, (x, top), self.symbol(at_corner, true), sides.top);
            self.put(
                buffer,
                (x, bottom),
                self.symbol(at_corner, false),
                sides.bottom,
            );
        }
        for y in (self.bounds.top()..self.bounds.bottom()).map(i32::from) {
            if (y == top && sides.top.is_some()) || (y == bottom && sides.bottom.is_some()) {
                continue;
            }
            self.put(buffer, (left, y), "│", sides.left);
            self.put(buffer, (right, y), "│", sides.right);
        }
    }

    fn symbol(&self, (at_left, at_right): (bool, bool), at_top: bool) -> &'static str {
        let corners = self.corners;
        let pick = |rounded: bool, round, square| if rounded { round } else { square };
        match (at_left, at_right, at_top) {
            (true, false, true) => pick(corners.top_left, "╭", "┌"),
            (false, true, true) => pick(corners.top_right, "╮", "┐"),
            (true, false, false) => pick(corners.bottom_left, "╰", "└"),
            (false, true, false) => pick(corners.bottom_right, "╯", "┘"),
            _ => "─",
        }
    }

    fn put(&self, buffer: &mut Buffer, (x, y): (i32, i32), symbol: &str, color: Option<Color>) {
        let Some(color) = color else {
            return;
        };
        let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
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
        if self.gradients.is_empty() {
            return side;
        }
        sample_gradients(
            self.gradients,
            cell_center(self.rect, Position::new(x, y), self.cell),
        )
        .map_or(side, linear_cell_color)
    }
}

fn draw_text(buffer: &mut Buffer, bounds: Rect, text: &TextRun) {
    let content = text.content;
    let visible = content.clamped(bounds);
    if visible.is_empty() {
        return;
    }
    let width = (content.right - content.left) as usize;
    let limit = text.text.is_wrapping.then_some(width);
    let lines = wrap_spans(&text.text.spans, limit);
    let hidden_rows = (i32::from(visible.top()) - content.top) as usize;
    for (y, line) in (visible.top()..visible.bottom()).zip(lines.iter().skip(hidden_rows)) {
        let slack = width.saturating_sub(line_width(line));
        // Text is left-to-right here, so start is left and justified text
        // has no inter-word spacing to add.
        let indent = match text.text.justify {
            Justify::Left | Justify::Start | Justify::Justified => 0,
            Justify::Center => slack / 2,
            Justify::Right | Justify::End => slack,
        };
        draw_line(buffer, line, content.left + indent as i32, y, visible);
    }
}

// A cluster straddling the left edge is left undrawn rather than shifted
// into view, so every drawn cluster sits in its laid-out column.
fn draw_line(buffer: &mut Buffer, line: &[Seg], left: i32, y: u16, visible: Rect) {
    let mut column = left;
    for (content, style) in line {
        for cluster in content.graphemes(true) {
            let start = column;
            column += cluster.width() as i32;
            let Ok(x) = u16::try_from(start) else {
                continue;
            };
            if x >= visible.right() {
                return;
            }
            if x >= visible.left() && !style.add_modifier.contains(Modifier::HIDDEN) {
                buffer.set_stringn(x, y, cluster, usize::from(visible.right() - x), *style);
            }
        }
    }
}
