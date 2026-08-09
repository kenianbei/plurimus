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
use std::marker::PhantomData;

use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{Component, Has, Query, RemovedComponents, Res, With, Without};
use bevy_ecs::query::QueryFilter;
use bevy_ecs::system::SystemParam;
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "the interaction flags a stylist last drew; this is the change-detection key, not a config"
)]
pub(crate) struct StylistCache {
    rendered: bool,
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

    // Pointer and focus set aside, for containers showing those on one
    // row. Disabled survives: a disabled container is disabled throughout.
    pub(crate) fn resting_style(&self, theme: &UiTheme) -> Style {
        Self {
            hovered: false,
            pressed: false,
            focused: false,
            ..*self
        }
        .style(theme)
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
        hovered: hovered.0,
        pressed,
        disabled,
        checked,
        focused: focus.get() == Some(entity),
        value_bits,
        ..StylistCache::styled(over)
    }
}

/// Marks a container whose content changed: a row added, edited, restyled,
/// or checked.
///
/// A container's rows are children, and a child's change never marks its
/// parent, so no query filter on the container can see one.
/// [`mark_dirty_content`] forwards it here, and the stylist reads this
/// beside its [`StylistCache`] rather than hashing every row's content to
/// find out.
#[derive(Component)]
pub(crate) struct ContentDirty<M: Send + Sync + 'static>(PhantomData<M>);

impl<M: Send + Sync + 'static> Default for ContentDirty<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

// A row clearing one of these reaches its container by no other route:
// `Changed` never fires for a component that goes, and the row itself keeps
// no record that it had one.
#[derive(SystemParam)]
pub(crate) struct ClearedRows<'w, 's> {
    checked: RemovedComponents<'w, 's, Checked>,
    styled: RemovedComponents<'w, 's, UiStyle>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl ClearedRows<'_, '_> {
    // A despawned row resolves to nothing, which is right: its container
    // hears about it through `Changed<Children>` instead.
    fn parents(&mut self) -> Vec<Entity> {
        self.checked
            .read()
            .chain(self.styled.read())
            .filter_map(|row| self.parents.get(row).ok())
            .map(ChildOf::parent)
            .collect()
    }
}

/// Forwards a row's change to the container that draws it.
///
/// `RowsChanged` filters the rows, `SelfChanged` the containers themselves.
/// Both need a `With<..>` term beside any `Or`, which matches an archetype
/// holding **any** one of its terms and would otherwise scan every entity
/// in the app carrying one.
pub(crate) fn mark_dirty_content<M, RowsChanged, SelfChanged>(
    rows: Query<&ChildOf, RowsChanged>,
    changed: Query<Entity, SelfChanged>,
    mut cleared: ClearedRows,
    mut content: Query<&mut ContentDirty<M>>,
) where
    M: Send + Sync + 'static,
    RowsChanged: QueryFilter + 'static,
    SelfChanged: QueryFilter + 'static,
{
    let touched = rows
        .iter()
        .map(ChildOf::parent)
        .chain(changed.iter())
        .chain(cleared.parents());
    for container in touched {
        if let Ok(mut dirty) = content.get_mut(container) {
            dirty.set_changed();
        }
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
