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
use plurimus_core::UiWidget;
use plurimus_ui::{
    Checked, Hovered, InteractionDisabled, InteractionState, Pressed, StylistDisabled, UiStyle,
    UiTheme,
};

// Shared, so the crate's cursor cannot differ between two widgets.
pub(crate) const CURSOR_SYMBOL: &str = "> ";

pub(crate) fn cursor_symbol(over: Option<&Line<'static>>) -> Line<'static> {
    over.cloned().unwrap_or_else(|| Line::from(CURSOR_SYMBOL))
}

/// Last state a stylist rendered, to skip redundant widget rebuilds.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct StylistCache {
    rendered: bool,
    state: InteractionState,
    pub(crate) checked: bool,
    over: Option<Style>,
    value_bits: u64,
}

impl StylistCache {
    pub(crate) fn style(&self, theme: &UiTheme) -> Style {
        let base = theme.resolve(self.state);
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

    // Pointer and focus set aside, for containers showing those on one
    // row. Disabled survives: a disabled container is disabled throughout.
    pub(crate) fn resting_style(&self, theme: &UiTheme) -> Style {
        Self {
            state: InteractionState {
                disabled: self.state.disabled,
                ..InteractionState::default()
            },
            ..*self
        }
        .style(theme)
    }

    pub(crate) fn focus_only(focused: bool, over: Option<&UiStyle>) -> Self {
        Self {
            state: InteractionState {
                focused,
                ..InteractionState::default()
            },
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

/// Marks a widget the stock stylists own: it is the marker `M` and has not
/// been exempted. Every stylist filters on this, so a new one cannot forget
/// to honor [`StylistDisabled`].
pub(crate) type Stylable<M> = (With<M>, Without<StylistDisabled>);

pub(crate) type LabeledQuery<'w, 's, 'a, M> = Query<
    'w,
    's,
    (
        StateQuery<'a>,
        &'a UiLabel,
        &'a mut StylistCache,
        &'a mut UiWidget,
    ),
    Stylable<M>,
>;

// Cloning is what carries the label's line style and alignment across;
// dropping either silently loses row striping.
pub(crate) fn decorate(
    prefix: &'static str,
    label: &Line<'static>,
    suffix: &'static str,
) -> Line<'static> {
    let mut line = label.clone();
    line.spans.insert(0, Span::raw(prefix));
    line.spans.push(Span::raw(suffix));
    line
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
        state: InteractionState {
            hovered: hovered.0,
            pressed,
            disabled,
            focused: focus.get() == Some(entity),
        },
        checked,
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
