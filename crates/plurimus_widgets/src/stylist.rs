//! Stock stylist systems: rebuild a widget's `UiWidget` when its interaction
//! state changes.
//!
//! Rendering every widget every frame would be wasteful, so each stylist
//! compares the state it drew last time - hover, press, focus, disabled,
//! checked, and a hash of whatever value the widget carries - against the
//! current state, and rebuilds only on a difference. That comparison is what
//! [`StylistCache`] stores, and it is also why the stock stylists can be
//! replaced wholesale: an app that wants a different look writes its own
//! system and the rest of the widget is untouched.
//!
//! The stylists themselves live with their widgets; this file is only the
//! engine they share.

use std::hash::{DefaultHasher, Hash, Hasher};

use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Component, Has, Query, Res, With};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::style::Style;

use crate::UiLabel;
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::{Checked, Hovered, InteractionDisabled, Pressed};

/// Last state a stylist rendered, to skip redundant widget rebuilds.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct StylistCache {
    pub(crate) rendered: bool,
    hovered: bool,
    pressed: bool,
    disabled: bool,
    pub(crate) checked: bool,
    focused: bool,
    value_bits: u64,
}

impl StylistCache {
    pub(crate) fn style(&self, theme: &UiTheme) -> Style {
        theme.resolve(self.hovered, self.pressed, self.disabled, self.focused)
    }

    pub(crate) fn focus_only(focused: bool) -> Self {
        Self {
            rendered: true,
            focused,
            ..Self::default()
        }
    }
}

pub(crate) type StateQuery<'a> = (
    Entity,
    &'a Hovered,
    Has<Pressed>,
    Has<InteractionDisabled>,
    Has<Checked>,
);

pub(crate) type LabeledQuery<'w, 's, 'a, M> = Query<
    'w,
    's,
    (
        StateQuery<'a>,
        &'a UiLabel,
        &'a mut StylistCache,
        &'a mut UiWidget,
    ),
    With<M>,
>;

// Every stylist funnels widget-specific state through this one hash.
pub(crate) fn hashed_bits(state: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn observed(
    (entity, hovered, pressed, disabled, checked): (Entity, &Hovered, bool, bool, bool),
    focus: &InputFocus,
    value_bits: u64,
) -> StylistCache {
    StylistCache {
        rendered: true,
        hovered: hovered.0,
        pressed,
        disabled,
        checked,
        focused: focus.get() == Some(entity),
        value_bits,
    }
}

pub(crate) fn restyle<M: Component>(
    theme: &Res<UiTheme>,
    focus: &InputFocus,
    widgets: &mut LabeledQuery<M>,
    render: impl Fn(&StylistCache, &str, Style) -> UiWidget,
) {
    for (state, label, mut cache, mut widget) in widgets.iter_mut() {
        let next = observed(state, focus, 0);
        if !theme.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        *widget = render(&next, &label.0, next.style(theme));
    }
}
