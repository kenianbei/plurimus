//! The push button: the simplest widget, and the one whose component other
//! widgets compose to inherit activation.
//!
//! [`Button`] carries no state of its own - it is a marker that makes an
//! entity a target of the activation path in [`crate::activate`], which is
//! why `menu_button()` requires it alongside [`crate::MenuButton`].

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{Component, Res};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::Alignment;
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::paragraph::Paragraph;

use plurimus_core::UiWidget;
use plurimus_ui::UiLabel;
use plurimus_ui::{Hovered, UiTheme};
use plurimus_ui::{LabeledQuery, StylistCache, decorate, restyle};

/// A push button. Emits [`Activate`](crate::Activate) on click or
/// Enter/Space.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache)]
pub struct Button;

/// Spawn bundle for a standard button.
pub fn button(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        Button,
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
