//! Radio buttons and the group that makes them exclusive, `bevy_ui_widgets`'
//! own [`RadioButton`] and [`RadioGroup`] drawn in cells.
//!
//! The group is the tab stop and the addressable value: arrows, Home and End
//! move the check among its enabled options while it holds focus, and every
//! selection emits [`ValueChange<Entity>`](plurimus_ui::ValueChange) on the
//! group naming the option. A press on an option focuses the group, the
//! nearest `TabIndex` above it, and the checked option draws focused while
//! the group holds focus, so the group's focus shows on the option it would
//! move from.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{ChildOf, Query, Ref, Res};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::text::Line;
use plurimus_ui::bevy_ui_widgets::{RadioButton, RadioGroup};

use crate::fitted::{FittedLabel, LabelFrame};
use plurimus_core::UiWidget;
use plurimus_ui::{Hovered, StateQuery, Stylable, StylistCache, UiLabel, UiTheme, observed};

/// Spawn bundle for a radio option; parent it to a [`radio_group`]
/// directly, since the group's updater and its focus display both read the
/// option's own parent.
pub fn radio(label: impl Into<Line<'static>>) -> impl Bundle {
    (
        RadioButton,
        Hovered::default(),
        StylistCache::default(),
        UiLabel(label.into()),
        UiWidget::default(),
    )
}

/// Spawn bundle for the group its [`radio`] options are children of.
///
/// It draws nothing, but carries a widget so directional navigation can
/// reach it: give it a `UiArea` spanning its options, as a tab bar is given
/// one, or it fills its camera. Attach
/// [`radio_self_update`](crate::radio_self_update) for uncontrolled
/// behavior.
#[must_use]
pub fn radio_group() -> impl Bundle {
    (RadioGroup, TabIndex(0), UiWidget::default())
}

type Radios<'w, 's> = Query<
    'w,
    's,
    (
        StateQuery<'static>,
        Ref<'static, UiLabel>,
        Option<&'static ChildOf>,
        &'static mut StylistCache,
        &'static mut UiWidget,
    ),
    Stylable<RadioButton>,
>;

pub(crate) fn style_radios(theme: Res<UiTheme>, focus: Res<InputFocus>, mut radios: Radios) {
    for (state, label, parent, mut cache, mut widget) in &mut radios {
        let is_group_focused = parent.is_some_and(|parent| focus.get() == Some(parent.parent()));
        let next = observed(state, &focus, 0);
        let shown = next.with_focused(next.state().focused || (is_group_focused && next.checked()));
        if !cache.redraws(shown, theme.is_changed() || label.is_changed()) {
            continue;
        }
        let mark = if shown.checked() { "(•) " } else { "( ) " };
        let frame = LabelFrame {
            prefix: mark,
            ..LabelFrame::PLAIN
        };
        let style = shown.style(&theme);
        *widget = UiWidget::new(FittedLabel::new(&label.0, style, frame, theme.overflow));
    }
}
