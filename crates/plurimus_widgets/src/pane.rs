//! A bordered container whose border restyles while focus is inside it.
//!
//! The pane itself is not focusable; it asks `bevy_input_focus` whether
//! focus sits within it. That is what lets a pane highlight while a widget
//! nested anywhere inside it holds focus, without the pane knowing what its
//! children are.

use std::borrow::Cow;

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::prelude::{Component, Entity, Query, Res};
use bevy_input_focus::{IsFocused, IsFocusedHelper};
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::Line;
use plurimus_core::ratatui_core::widgets::Widget;
use ratatui_widgets::block::Block;

use plurimus_core::UiWidget;
use plurimus_ui::{
    InteractionState, Stylable, StylistCache, TextOverflow, UiLabel, UiStyle, UiTheme,
};

/// A bordered region titled by its [`UiLabel`]. Not focusable itself:
/// "focused" means focus sits on a descendant.
#[derive(Component, Debug, Clone, Copy)]
#[require(StylistCache)]
pub struct Pane;

/// Spawn bundle for a pane; parent widgets to it so the border follows
/// their focus.
pub fn pane(title: impl Into<Line<'static>>) -> impl Bundle {
    (Pane, UiLabel(title.into()), UiWidget::default())
}

pub(crate) fn style_panes(
    theme: Res<UiTheme>,
    focus: IsFocusedHelper,
    mut panes: Query<
        (
            Entity,
            Ref<UiLabel>,
            Option<&UiStyle>,
            &mut StylistCache,
            &mut UiWidget,
        ),
        Stylable<Pane>,
    >,
) {
    for (pane, label, over, mut cache, mut widget) in &mut panes {
        let focused = focus.is_focus_within(pane);
        let next = StylistCache::new(InteractionState::default().with_focused(focused), over);
        if !cache.redraws(next, theme.is_changed() || label.is_changed()) {
            continue;
        }
        let style = next.style(&theme);
        *widget = UiWidget::new(PaneFrame {
            drawn: framed(label.0.clone(), style),
            title: label.0.clone(),
            style,
            overflow: theme.overflow,
        });
    }
}

// The title runs between the two side borders.
const BORDER_CELLS: u16 = 2;

/// The bordered frame as the stylist built it, refitted only on a frame
/// where its title does not fit between the borders.
struct PaneFrame {
    drawn: Block<'static>,
    title: Line<'static>,
    style: Style,
    overflow: TextOverflow,
}

impl Widget for &PaneFrame {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let room = area.width.saturating_sub(BORDER_CELLS);
        match self.overflow.fit(&self.title, room) {
            Cow::Borrowed(_) => Widget::render(&self.drawn, area, buffer),
            Cow::Owned(title) => Widget::render(framed(title, self.style), area, buffer),
        }
    }
}

fn framed(title: Line<'static>, style: Style) -> Block<'static> {
    Block::bordered()
        .title(title)
        .border_style(style)
        .title_style(style)
}
