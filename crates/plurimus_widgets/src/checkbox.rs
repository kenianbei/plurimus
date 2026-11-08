//! The checkbox, `bevy_ui_widgets`' own [`Checkbox`] drawn in cells.
//!
//! Upstream's observers emit [`ValueChange<bool>`](plurimus_ui::ValueChange)
//! carrying the negation of the current [`Checked`](plurimus_ui::Checked)
//! state rather than applying it, so an app can veto or transform the
//! toggle. Attach [`checkbox_self_update`](crate::checkbox_self_update) for
//! the uncontrolled behavior.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::Res;
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::text::Line;
use plurimus_ui::bevy_ui_widgets::Checkbox;
use ratatui_widgets::paragraph::Paragraph;

use plurimus_core::UiWidget;
use plurimus_ui::UiLabel;
use plurimus_ui::{Hovered, UiTheme};
use plurimus_ui::{LabeledQuery, StylistCache, decorate, restyle};

/// Spawn bundle for a standard checkbox: a tab stop emitting
/// [`ValueChange<bool>`](plurimus_ui::ValueChange).
///
/// A press on it focuses it whatever
/// [`PressFocusDisabled`](plurimus_ui::PressFocusDisabled) says, since
/// upstream's own press handler sets focus.
pub fn checkbox(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        Checkbox,
        Hovered::default(),
        StylistCache::default(),
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
