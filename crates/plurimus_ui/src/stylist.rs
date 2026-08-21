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

use bevy_ecs::change_detection::{DetectChanges, Ref};
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::{Component, Has, Query, With, Without};
use bevy_ecs::world::DeferredWorld;
use bevy_input_focus::InputFocus;
use plurimus_core::UiWidget;
use plurimus_core::ratatui_core::style::Style;
use plurimus_core::ratatui_core::text::{Line, Span};

use crate::interaction::{Checked, Hovered, InteractionDisabled, Pressed};
use crate::theme::{InteractionState, StylistDisabled, UiStyle, UiTheme};

/// A widget's text label, rendered by a widget library's stylists.
///
/// A [`Line`] rather than a `String`, so a label can carry per-span style -
/// columns in a list row, a dimmed shortcut beside a menu item. Wraps
/// anything that converts into a `Line`, so a plain label stays plain:
/// `UiLabel("save".into())`.
#[derive(Component, Debug, Clone)]
pub struct UiLabel(pub Line<'static>);

/// The state a stylist last drew, so it can tell an idle frame from a
/// changed one without rebuilding the widget to find out.
///
/// [`Default`] is the never-drawn value and equals nothing any constructor
/// produces, which is what makes a widget paint on its first frame. A
/// widget library puts it on its widgets, usually through `#[require]`; a
/// stylist finds nothing to compare against without it.
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
    pub const fn new(state: InteractionState, over: Option<&UiStyle>) -> Self {
        Self {
            rendered: true,
            state,
            checked: false,
            over: match over {
                Some(style) => Some(style.0),
                None => None,
            },
            value_bits: 0,
        }
    }

    /// Sets what the widget carries beyond interaction state, hashed
    /// through [`hashed_bits`].
    ///
    /// The seam for a stylist that resolves its own state rather than
    /// reading it through [`observed`] - a painter drawing one resting
    /// style, say, which has no [`Hovered`] or [`InputFocus`] to hand over.
    #[must_use]
    pub const fn with_value(mut self, bits: u64) -> Self {
        self.value_bits = bits;
        self
    }

    /// Sets the focus term, keeping everything else - including any
    /// [`UiStyle`] already patched in.
    ///
    /// The seam for a container that draws one part of itself as the active
    /// one while keyboard focus sits somewhere else: a list stepped by a
    /// search field beside it is still the thing being operated, so its
    /// cursor row resolves focused even though the list does not hold
    /// focus. Style with the result; keep storing the observed value, which
    /// is what a redraw compares against.
    #[must_use]
    pub const fn with_focused(mut self, focused: bool) -> Self {
        self.state.focused = focused;
        self
    }

    /// Whether the widget was checked when it was last drawn.
    #[must_use]
    pub const fn checked(&self) -> bool {
        self.checked
    }

    /// The interaction state the widget was last drawn in, for a stylist
    /// whose drawing turns on more than the style it resolves - a caret
    /// shown only while focused, say.
    #[must_use]
    pub const fn state(&self) -> InteractionState {
        self.state
    }

    /// Stores `next` and reports whether the widget has to be rebuilt.
    ///
    /// True when the drawn state moved, and whenever `dirty` is set -
    /// which is how a theme swap repaints widgets whose own state has not
    /// changed, and how a container catches an edit to a child row that
    /// never marked the container itself.
    ///
    /// Forgetting either is the failure this exists to prevent: a stylist
    /// that compares only the cache stops repainting on a theme change,
    /// and nothing about the widget says so.
    #[must_use]
    pub fn redraws(&mut self, next: Self, dirty: bool) -> bool {
        if !dirty && next == *self {
            return false;
        }
        *self = next;
        true
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
/// term. What it yields for one entity is what [`observed`] takes.
pub type StateQuery<'a> = (
    Entity,
    &'a Hovered,
    Has<Pressed>,
    Has<InteractionDisabled>,
    Has<Checked>,
    Option<&'a UiStyle>,
);

/// Filters the widgets marked `M` that have not been exempted, so a
/// stylist cannot forget to honor [`StylistDisabled`].
pub type Stylable<M> = (With<M>, Without<StylistDisabled>);

/// The query [`restyle`] drives: every stylable `M` with a label, its
/// cache, and the widget to rebuild.
///
/// The label is a [`Ref`] so [`restyle`] can repaint a widget whose text
/// changed - the cache compares interaction state, which an edited label
/// leaves untouched.
pub type LabeledQuery<'w, 's, 'a, M> = Query<
    'w,
    's,
    (
        StateQuery<'a>,
        Ref<'a, UiLabel>,
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

/// Rebuilds every stylable `M` whose drawn state or label has moved, and
/// skips the rest.
///
/// `render` is handed the state, the label, and the resolved style, and
/// returns the widget to draw. It runs only for entities that need it, or
/// for all of them when the theme itself changed.
///
/// `theme_changed` is [`DetectChanges::is_changed`] on the theme resource,
/// taken as a `bool` so a caller holding a plain `&UiTheme` can call this.
///
/// For widgets whose look turns on interaction state and the label alone:
/// the comparison passes no `value_bits`, so a widget that also draws a
/// value - a slider's position, a cursor row - would skip the frame that
/// value moved. Those call [`observed`] and [`StylistCache::redraws`]
/// themselves, hashing what they carry through [`hashed_bits`].
pub fn restyle<M: Component>(
    theme: &UiTheme,
    theme_changed: bool,
    focus: &InputFocus,
    widgets: &mut LabeledQuery<M>,
    render: impl Fn(&StylistCache, &Line<'static>, Style) -> UiWidget,
) {
    for (state, label, mut cache, mut widget) in widgets.iter_mut() {
        let next = observed(state, focus, 0);
        if !cache.redraws(next, theme_changed || label.is_changed()) {
            continue;
        }
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

/// Resets the cache of an entity leaving [`StylistDisabled`], so the
/// stylist taking it back rebuilds on the first frame - see that
/// component for why.
pub(crate) fn invalidate_cache(mut world: DeferredWorld, context: HookContext) {
    if let Some(mut cache) = world.get_mut::<StylistCache>(context.entity) {
        *cache = StylistCache::default();
    }
}

/// Hashes whatever a widget carries beyond its interaction state into the
/// `value_bits` [`observed`] takes.
#[must_use]
pub fn hashed_bits(state: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{StylistCache, hashed_bits};
    use crate::theme::InteractionState;

    fn cache(bits: u64) -> StylistCache {
        StylistCache::new(InteractionState::default(), None).with_value(bits)
    }

    #[test]
    fn a_moved_value_redraws_a_widget_whose_interaction_state_held_still() {
        let mut drawn = cache(hashed_bits("first"));

        assert!(!drawn.redraws(cache(hashed_bits("first")), false));
        assert!(drawn.redraws(cache(hashed_bits("second")), false));
    }

    #[test]
    fn with_value_is_the_only_difference_from_the_bare_constructor() {
        let bare = StylistCache::new(InteractionState::default(), None);

        assert_eq!(bare.with_value(0), bare);
        assert_ne!(bare.with_value(1), bare);
    }
}
