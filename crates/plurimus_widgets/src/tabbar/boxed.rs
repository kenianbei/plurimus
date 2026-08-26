//! A boxed item, and the glyphs that join it to the bar's baseline.
//!
//! `border::Set` carries corners and sides only, so a box that meets a
//! baseline takes its junctions from the matching `line::Set`: an active box
//! opens onto the baseline through two turned corners, a closed one sits on
//! it through two tees. A border with no line set - the quadrant blocks -
//! cannot join and draws closed.

use plurimus_core::Edge;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::symbols::line;
use plurimus_core::ratatui_core::text::Line;
use plurimus_core::ratatui_core::widgets::Widget;
use ratatui_widgets::block::{Block, Padding};
use ratatui_widgets::borders::BorderType;

pub(crate) const fn border_lines(border: BorderType) -> Option<line::Set<'static>> {
    match border {
        BorderType::Plain
        | BorderType::LightDoubleDashed
        | BorderType::HeavyDoubleDashed
        | BorderType::LightTripleDashed
        | BorderType::HeavyTripleDashed
        | BorderType::LightQuadrupleDashed
        | BorderType::HeavyQuadrupleDashed => Some(line::NORMAL),
        BorderType::Rounded => Some(line::ROUNDED),
        BorderType::Double => Some(line::DOUBLE),
        BorderType::Thick => Some(line::THICK),
        BorderType::QuadrantInside | BorderType::QuadrantOutside => None,
    }
}

/// The edge a look joins on, with the glyphs its border joins in.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Joint {
    pub(crate) edge: Edge,
    pub(crate) lines: line::Set<'static>,
}

pub(crate) struct Boxed {
    pub(crate) label: Line<'static>,
    pub(crate) border: BorderType,
    pub(crate) padding: u16,
    pub(crate) style: Style,
    pub(crate) joint: Option<Joint>,
    pub(crate) active: bool,
}

impl Widget for &Boxed {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let block = Block::bordered()
            .border_type(self.border)
            .padding(Padding::horizontal(self.padding))
            .style(self.style);
        let label = Rect {
            height: 1,
            ..block.inner(area)
        };
        Widget::render(block, area, buffer);
        Widget::render(&self.label, label, buffer);
        if let Some(joint) = self.joint {
            join(area, joint, self.active, self.style, buffer);
        }
    }
}

// An active box turns its two corners on the joined edge outward and
// clears the edge between them, so the baseline runs into the box; a
// closed one tees its corners into the baseline it sits on.
fn join(area: Rect, joint: Joint, active: bool, style: Style, buffer: &mut Buffer) {
    let (left, right, top, bottom) = (
        area.left(),
        area.right().saturating_sub(1),
        area.top(),
        area.bottom().saturating_sub(1),
    );
    let lines = joint.lines;
    let (corners, open, tee) = match joint.edge {
        Edge::Bottom => (
            [(left, bottom), (right, bottom)],
            [lines.bottom_right, lines.bottom_left],
            lines.horizontal_up,
        ),
        Edge::Top => (
            [(left, top), (right, top)],
            [lines.top_right, lines.top_left],
            lines.horizontal_down,
        ),
        Edge::Right => (
            [(right, top), (right, bottom)],
            [lines.bottom_right, lines.top_right],
            lines.vertical_left,
        ),
        Edge::Left => (
            [(left, top), (left, bottom)],
            [lines.bottom_left, lines.top_left],
            lines.vertical_right,
        ),
    };
    if active {
        let [(x, y), (far_x, far_y)] = corners;
        let edge = Rect::new(x, y, far_x - x + 1, far_y - y + 1);
        for position in edge.positions() {
            if let Some(cell) = buffer.cell_mut(position) {
                cell.set_symbol(" ").set_style(style);
            }
        }
    }
    let glyphs = if active { open } else { [tee; 2] };
    for ((x, y), glyph) in corners.into_iter().zip(glyphs) {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_symbol(glyph).set_style(style);
        }
    }
}
