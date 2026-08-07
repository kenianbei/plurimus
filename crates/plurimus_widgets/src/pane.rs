//! A bordered container whose border restyles while focus is inside it.
//!
//! The pane itself is not focusable; it reacts to
//! [`FocusWithin`](plurimus_ui::FocusWithin), which `plurimus_ui` maintains on
//! every ancestor of the focused entity. That is what lets a pane highlight
//! while a widget nested anywhere inside it holds focus, without the pane
//! knowing what its children are.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{Component, Has, Query, Res};
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::block::Block;

use super::{UiLabel, placeholder};
use crate::stylist::{Stylable, StylistCache, UiStyle};
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::FocusWithin;

/// A bordered region titled by its [`UiLabel`]. Not focusable itself:
/// "focused" means [`FocusWithin`] - focus sits on a descendant.
#[derive(Component, Debug, Clone, Copy)]
#[require(StylistCache)]
pub struct Pane;

/// Spawn bundle for a pane; parent widgets to it so the border follows
/// their focus.
pub fn pane(title: impl Into<Line<'static>>) -> impl Bundle {
    (Pane, UiLabel(title.into()), placeholder())
}

pub(crate) fn style_panes(
    theme: Res<UiTheme>,
    mut panes: Query<
        (
            Has<FocusWithin>,
            &UiLabel,
            Option<&UiStyle>,
            &mut StylistCache,
            &mut UiWidget,
        ),
        Stylable<Pane>,
    >,
) {
    for (focused, label, over, mut cache, mut widget) in &mut panes {
        let next = StylistCache::focus_only(focused, over);
        if !theme.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        let style = next.style(&theme);
        *widget = UiWidget::new(
            Block::bordered()
                .title(label.0.clone())
                .border_style(style)
                .title_style(style),
        );
    }
}
