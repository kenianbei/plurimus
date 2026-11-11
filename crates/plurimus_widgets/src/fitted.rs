//! A one-row label fitted to the rect it is drawn into, which a stylist
//! does not know when it restyles: cut by the theme's [`TextOverflow`] with
//! its decoration kept, so a too-narrow button still ends in ` ]`.

use std::borrow::Cow;

use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Alignment, Rect};
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_ui::{TextOverflow, decorate};
use ratatui_widgets::block::{Block, Padding};
use ratatui_widgets::paragraph::Paragraph;

/// What surrounds a label inside its rect: text on either side, padding
/// beyond that, and where the whole sits.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LabelFrame {
    pub(crate) prefix: &'static str,
    pub(crate) suffix: &'static str,
    pub(crate) padding: u16,
    pub(crate) alignment: Alignment,
}

impl LabelFrame {
    pub(crate) const PLAIN: Self = Self {
        prefix: "",
        suffix: "",
        padding: 0,
        alignment: Alignment::Left,
    };

    fn paragraph(self, label: &Line<'static>, style: Style) -> Paragraph<'static> {
        let paragraph = Paragraph::new(decorate(self.prefix, label, self.suffix))
            .style(style)
            .alignment(self.alignment);
        if self.padding == 0 {
            return paragraph;
        }
        paragraph.block(Block::new().padding(Padding::horizontal(self.padding)))
    }

    fn reserved(self) -> u16 {
        let decoration = Line::from(self.prefix).width() + Line::from(self.suffix).width();
        u16::try_from(decoration)
            .unwrap_or(u16::MAX)
            .saturating_add(self.padding.saturating_mul(2))
    }
}

/// The label as its stylist built it, kept beside its parts so a frame
/// where it fits draws the prebuilt paragraph and allocates nothing.
pub(crate) struct FittedLabel {
    drawn: Paragraph<'static>,
    label: Line<'static>,
    style: Style,
    frame: LabelFrame,
    reserved: u16,
    overflow: TextOverflow,
}

impl FittedLabel {
    pub(crate) fn new(
        label: &Line<'static>,
        style: Style,
        frame: LabelFrame,
        overflow: TextOverflow,
    ) -> Self {
        Self {
            drawn: frame.paragraph(label, style),
            label: label.clone(),
            style,
            frame,
            reserved: frame.reserved(),
            overflow,
        }
    }
}

impl Widget for &FittedLabel {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let room = area.width.saturating_sub(self.reserved);
        match self.overflow.fit(&self.label, room) {
            Cow::Borrowed(_) => Widget::render(&self.drawn, area, buffer),
            Cow::Owned(fitted) => {
                Widget::render(self.frame.paragraph(&fitted, self.style), area, buffer);
            }
        }
    }
}
