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
use bevy_ecs::prelude::{Component, Has, Query, Res, With, Without};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::{Line, Span};

use crate::UiLabel;
use crate::theme::UiTheme;
use plurimus_core::UiWidget;
use plurimus_ui::{Checked, Hovered, InteractionDisabled, Pressed};

/// Exempts an entity from the stock stylists, leaving its [`UiWidget`] to
/// the app. Behavior - selection, keys, scrolling, events - is untouched.
#[derive(Component, Debug, Clone, Copy)]
pub struct StylistDisabled;

/// Patched over the style an entity would otherwise resolve to.
///
/// Patched rather than substituted, so an override setting only `bg` keeps
/// the theme's foreground and modifiers, and a widget carrying one still
/// shows hover and focus. On a [`ListItem`](crate::ListItem) child it
/// styles that row, covering the full row width where a label's own line
/// style stops at the cursor gutter.
#[derive(Component, Debug, Clone, Copy)]
pub struct UiStyle(pub Style);

/// Last state a stylist rendered, to skip redundant widget rebuilds.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct StylistCache {
    pub(crate) rendered: bool,
    hovered: bool,
    pressed: bool,
    disabled: bool,
    pub(crate) checked: bool,
    focused: bool,
    over: Option<Style>,
    value_bits: u64,
}

impl StylistCache {
    pub(crate) fn style(&self, theme: &UiTheme) -> Style {
        let base = theme.resolve(self.hovered, self.pressed, self.disabled, self.focused);
        match self.over {
            Some(over) => base.patch(over),
            None => base,
        }
    }

    // For widgets with no interaction state to resolve; the theme's normal
    // style is what `style` returns when every flag is false.
    pub(crate) fn styled(over: Option<&UiStyle>) -> Self {
        Self {
            rendered: true,
            over: over.map(|style| style.0),
            ..Self::default()
        }
    }

    pub(crate) fn focus_only(focused: bool, over: Option<&UiStyle>) -> Self {
        Self {
            focused,
            ..Self::styled(over)
        }
    }
}

pub(crate) type StateQuery<'a> = (
    Entity,
    &'a Hovered,
    Has<Pressed>,
    Has<InteractionDisabled>,
    Has<Checked>,
    Option<&'a UiStyle>,
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
    (With<M>, Without<StylistDisabled>),
>;

// Decorations wrap the label rather than replace it, so the label's own
// spans, line style, and alignment all have to survive the splice - a
// dropped line style silently loses row striping.
pub(crate) fn decorate(
    prefix: &'static str,
    label: &Line<'static>,
    suffix: &'static str,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(label.spans.len() + 2);
    spans.push(Span::raw(prefix));
    spans.extend(label.spans.iter().cloned());
    if !suffix.is_empty() {
        spans.push(Span::raw(suffix));
    }
    Line {
        style: label.style,
        alignment: label.alignment,
        spans,
    }
}

// Every stylist funnels widget-specific state through this one hash.
pub(crate) fn hashed_bits(state: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn observed(
    (entity, hovered, pressed, disabled, checked, over): (
        Entity,
        &Hovered,
        bool,
        bool,
        bool,
        Option<&UiStyle>,
    ),
    focus: &InputFocus,
    value_bits: u64,
) -> StylistCache {
    StylistCache {
        hovered: hovered.0,
        pressed,
        disabled,
        checked,
        focused: focus.get() == Some(entity),
        value_bits,
        ..StylistCache::styled(over)
    }
}

pub(crate) fn restyle<M: Component>(
    theme: &Res<UiTheme>,
    focus: &InputFocus,
    widgets: &mut LabeledQuery<M>,
    render: impl Fn(&StylistCache, &Line<'static>, Style) -> UiWidget,
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
