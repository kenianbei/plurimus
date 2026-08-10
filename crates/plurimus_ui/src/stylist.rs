//! The engine a widget library's stylists share: what a widget last drew,
//! and the comparison that lets an idle frame skip redrawing it.
//!
//! Rendering every widget every frame would be wasteful, so a stylist
//! compares the state it drew last time - hover, press, focus, disabled,
//! checked, and a hash of whatever value the widget carries - against the
//! current state, and rebuilds only on a difference. [`StylistCache`] is
//! what stores that comparison.
//!
//! This is the vocabulary rather than the widgets: nothing here draws
//! anything. A library supplies the render closure, and an app that wants
//! a different look replaces the stylist and leaves the widget alone.

use std::hash::{DefaultHasher, Hash, Hasher};

use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Component, Has, Query, Res, With, Without};
use bevy_input_focus::InputFocus;
use plurimus_core::UiWidget;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::{Line, Span};

use crate::interaction::{Checked, Hovered, InteractionDisabled, Pressed};
use crate::theme::{InteractionState, StylistDisabled, UiStyle, UiTheme};

/// A widget's text label, rendered by a widget library's stylists.
///
/// A [`Line`] rather than a `String`, so a label can carry per-span style -
/// columns in a list row, a dimmed shortcut beside a menu item. Converts
/// from `String` and `&str`, so a plain label stays a plain label.
#[derive(Component, Debug, Clone)]
pub struct UiLabel(pub Line<'static>);

/// The state a stylist last drew, so it can tell an idle frame from a
/// changed one without rebuilding the widget to find out.
///
/// [`Default`] is the never-drawn value and equals nothing any constructor
/// produces, which is what makes a widget paint on its first frame.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct StylistCache {
    rendered: bool,
    state: InteractionState,
    checked: bool,
    over: Option<Style>,
    value_bits: u64,
}

impl StylistCache {
    /// A cache for `state`, with `over` patched over what the theme
    /// resolves.
    ///
    /// Records the widget as drawn, so it compares unequal to
    /// [`Default`] - see the type's own documentation.
    #[must_use]
    pub fn new(state: InteractionState, over: Option<&UiStyle>) -> Self {
        Self {
            rendered: true,
            state,
            checked: false,
            over: over.map(|style| style.0),
            value_bits: 0,
        }
    }

    /// Whether the widget was checked when it was last drawn.
    #[must_use]
    pub const fn checked(&self) -> bool {
        self.checked
    }

    /// The style this state resolves to, with any [`UiStyle`] patched over
    /// it.
    #[must_use]
    pub fn style(&self, theme: &UiTheme) -> Style {
        let base = theme.resolve(self.state);
        match self.over {
            Some(over) => base.patch(over),
            None => base,
        }
    }

    /// [`style`](Self::style) with pointer and focus set aside, for a
    /// container showing those on one row rather than throughout.
    ///
    /// Disabled survives: a disabled container is disabled everywhere.
    #[must_use]
    pub fn resting_style(&self, theme: &UiTheme) -> Style {
        Self {
            state: InteractionState {
                disabled: self.state.disabled,
                ..InteractionState::default()
            },
            ..*self
        }
        .style(theme)
    }
}

/// The interaction state a stylist reads off one widget entity, as a query
/// term. It yields [`ObservedState`], which [`observed`] takes.
pub type StateQuery<'a> = (
    Entity,
    &'a Hovered,
    Has<Pressed>,
    Has<InteractionDisabled>,
    Has<Checked>,
    Option<&'a UiStyle>,
);

/// One entity's worth of [`StateQuery`]: pressed, disabled, and checked
/// arrive as the booleans [`Has`] resolves to.
pub type ObservedState<'a> = (Entity, &'a Hovered, bool, bool, bool, Option<&'a UiStyle>);

/// Filters the widgets marked `M` that have not been exempted, so a
/// stylist cannot forget to honor [`StylistDisabled`].
pub type Stylable<M> = (With<M>, Without<StylistDisabled>);

/// The query [`restyle`] drives: every stylable `M` with a label, its
/// cache, and the widget to rebuild.
pub type LabeledQuery<'w, 's, 'a, M> = Query<
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

/// The cache a widget's current state would produce, to compare against
/// the one it last drew.
///
/// `value_bits` is whatever the widget carries beyond interaction state -
/// a slider's position, a list's cursor - hashed through [`hashed_bits`].
/// Pass `0` for a widget with none.
#[must_use]
pub fn observed(
    (entity, hovered, pressed, disabled, checked, over): ObservedState,
    focus: &InputFocus,
    value_bits: u64,
) -> StylistCache {
    StylistCache {
        checked,
        value_bits,
        ..StylistCache::new(
            InteractionState {
                hovered: hovered.0,
                pressed,
                disabled,
                focused: focus.get() == Some(entity),
            },
            over,
        )
    }
}

/// Rebuilds every stylable `M` whose drawn state has moved, and skips the
/// rest.
///
/// `render` is handed the state, the label, and the resolved style, and
/// returns the widget to draw. It runs only for entities that need it, or
/// for all of them when the theme itself changed.
pub fn restyle<M: Component>(
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

/// Wraps a label in a prefix and suffix, keeping its line style and
/// alignment - dropping either silently loses row striping.
#[must_use]
pub fn decorate(
    prefix: &'static str,
    label: &Line<'static>,
    suffix: &'static str,
) -> Line<'static> {
    let mut line = label.clone();
    line.spans.insert(0, Span::raw(prefix));
    line.spans.push(Span::raw(suffix));
    line
}

/// Hashes whatever a widget carries beyond its interaction state into the
/// `value_bits` [`observed`] takes.
#[must_use]
pub fn hashed_bits(state: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.hash(&mut hasher);
    hasher.finish()
}
