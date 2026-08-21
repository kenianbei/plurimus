//! The checkbox: a toggle whose checked state lives outside the widget.
//!
//! Activation emits [`ValueChange<bool>`](plurimus_ui::ValueChange) carrying
//! the negation of the current [`Checked`](plurimus_ui::Checked) state rather
//! than applying it, so an app can veto or transform the toggle. Attach
//! [`checkbox_self_update`](crate::checkbox_self_update) for the uncontrolled
//! behavior.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{Component, Res};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::paragraph::Paragraph;

use plurimus_core::UiWidget;
use plurimus_ui::UiLabel;
use plurimus_ui::{Hovered, UiTheme};
use plurimus_ui::{LabeledQuery, StylistCache, decorate, restyle};

/// A toggle. Emits [`ValueChange<bool>`](plurimus_ui::ValueChange); attach
/// [`checkbox_self_update`](crate::checkbox_self_update) for uncontrolled
/// behavior.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache)]
pub struct Checkbox;

/// Spawn bundle for a standard checkbox.
pub fn checkbox(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        Checkbox,
        UiLabel(label.into()),
        TabIndex(0),
        UiWidget::default(),
    )
}

pub(crate) fn style_checkboxes(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut boxes: LabeledQuery<Checkbox>,
) {
    restyle(
        &theme,
        theme.is_changed(),
        &focus,
        &mut boxes,
        |state, label, style| {
            let mark = if state.checked() { "[x] " } else { "[ ] " };
            UiWidget::new(Paragraph::new(decorate(mark, label, "")).style(style))
        },
    );
}
