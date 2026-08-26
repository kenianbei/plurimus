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
use ratatui_widgets::block::Block;
use ratatui_widgets::borders::{BorderType, Borders};

use super::FRAME;

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

impl Joint {
    /// The glyph the bar's baseline is drawn in.
    pub(crate) const fn baseline(&self) -> &'static str {
        match self.edge {
            Edge::Top | Edge::Bottom => self.lines.horizontal,
            Edge::Left | Edge::Right => self.lines.vertical,
        }
    }
}

pub(crate) struct Boxed {
    pub(crate) label: Line<'static>,
    pub(crate) border: BorderType,
    pub(crate) style: Style,
    pub(crate) joint: Option<Joint>,
    pub(crate) active: bool,
}

impl Widget for &Boxed {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let open = self.joint.filter(|_| self.active).map(|joint| joint.edge);
        let borders = match open {
            Some(Edge::Top) => Borders::ALL.difference(Borders::TOP),
            Some(Edge::Bottom) => Borders::ALL.difference(Borders::BOTTOM),
            Some(Edge::Left) => Borders::ALL.difference(Borders::LEFT),
            Some(Edge::Right) => Borders::ALL.difference(Borders::RIGHT),
            None => Borders::ALL,
        };
        Widget::render(
            Block::new()
                .borders(borders)
                .border_type(self.border)
                .style(self.style),
            area,
            buffer,
        );
        let label = Rect::new(
            area.x.saturating_add(FRAME),
            area.y.saturating_add(FRAME),
            area.width.saturating_sub(2 * FRAME),
            1,
        )
        .intersection(area);
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
    let (corners, glyphs) = match (joint.edge, active) {
        (Edge::Bottom, true) => (
            [(left, bottom), (right, bottom)],
            [lines.bottom_right, lines.bottom_left],
        ),
        (Edge::Bottom, false) => ([(left, bottom), (right, bottom)], [lines.horizontal_up; 2]),
        (Edge::Top, true) => (
            [(left, top), (right, top)],
            [lines.top_right, lines.top_left],
        ),
        (Edge::Top, false) => ([(left, top), (right, top)], [lines.horizontal_down; 2]),
        (Edge::Right, true) => (
            [(right, top), (right, bottom)],
            [lines.bottom_right, lines.top_right],
        ),
        (Edge::Right, false) => ([(right, top), (right, bottom)], [lines.vertical_left; 2]),
        (Edge::Left, true) => (
            [(left, top), (left, bottom)],
            [lines.bottom_left, lines.top_left],
        ),
        (Edge::Left, false) => ([(left, top), (left, bottom)], [lines.vertical_right; 2]),
    };
    if active {
        clear_edge(area, joint.edge, style, buffer);
    }
    for ((x, y), glyph) in corners.into_iter().zip(glyphs) {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_symbol(glyph).set_style(style);
        }
    }
}

fn clear_edge(area: Rect, edge: Edge, style: Style, buffer: &mut Buffer) {
    let strip = match edge {
        Edge::Top => Rect::new(area.x, area.y, area.width, 1),
        Edge::Bottom => Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        Edge::Left => Rect::new(area.x, area.y, 1, area.height),
        Edge::Right => Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
    };
    for position in strip.intersection(area).positions() {
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_symbol(" ").set_style(style);
        }
    }
}
