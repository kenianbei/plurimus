//! Radio buttons and the group that makes them exclusive.
//!
//! Exclusivity is a parent-child relationship rather than a list held by the
//! group: activating a [`RadioButton`] walks up to the nearest [`RadioGroup`]
//! ancestor and emits [`ValueChange<Entity>`](plurimus_ui::ValueChange) on the
//! *group*, naming the option. The group is therefore the addressable value,
//! and options can nest inside layout containers without registering
//! themselves anywhere.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::{Component, Res};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::text::Line;
use ratatui_widgets::paragraph::Paragraph;

use crate::stylist::{LabeledQuery, StylistCache, decorate, restyle};
use crate::{UiLabel, placeholder};
use plurimus_core::UiWidget;
use plurimus_ui::{Hovered, UiTheme};

/// One option inside a [`RadioGroup`].
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache)]
pub struct RadioButton;

/// Exclusive-choice container; its [`RadioButton`] children select via
/// [`ValueChange<Entity>`](plurimus_ui::ValueChange) on the group. Attach
/// [`radio_self_update`](crate::radio_self_update) for uncontrolled behavior.
#[derive(Component, Debug, Clone, Copy)]
pub struct RadioGroup;

/// Spawn bundle for a radio option; parent it to a [`RadioGroup`].
pub fn radio(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        RadioButton,
        UiLabel(label.into()),
        TabIndex(0),
        placeholder(),
    )
}

pub(crate) fn style_radios(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut radios: LabeledQuery<RadioButton>,
) {
    restyle(&theme, &focus, &mut radios, |state, label, style| {
        let mark = if state.checked { "(•) " } else { "( ) " };
        UiWidget::new(Paragraph::new(decorate(mark, label, "")).style(style))
    });
}
