//! The push button, `bevy_ui_widgets`' own [`Button`] drawn in cells.
//!
//! Upstream's observers activate it - a click that ends on it, or Enter or
//! space while it holds focus - so this file is its constructor and its
//! stylist. `menu_button()` composes the same [`Button`] to inherit that.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::Res;
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::Alignment;
use plurimus_core::ratatui_core::text::Line;
use plurimus_ui::bevy_ui_widgets::Button;
use ratatui_widgets::paragraph::Paragraph;

use plurimus_core::UiWidget;
use plurimus_ui::UiLabel;
use plurimus_ui::{Hovered, UiTheme};
use plurimus_ui::{LabeledQuery, StylistCache, decorate, restyle};

/// Spawn bundle for a standard button: a tab stop emitting
/// [`Activate`](crate::Activate).
pub fn button(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        StylistCache::default(),
        UiLabel(label.into()),
        TabIndex(0),
        UiWidget::default(),
    )
}

pub(crate) fn style_buttons(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut buttons: LabeledQuery<Button>,
) {
    restyle(
        &theme,
        theme.is_changed(),
        &focus,
        &mut buttons,
        |_, label, style| {
            UiWidget::new(
                Paragraph::new(decorate("[ ", label, " ]"))
                    .style(style)
                    .alignment(Alignment::Center),
            )
        },
    );
}
