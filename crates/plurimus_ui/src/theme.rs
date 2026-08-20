//! The theming contract between an app and a widget library.
//!
//! One [`UiTheme`] resource styles every widget that honors it, so restyling
//! an app is a resource swap rather than a per-widget edit, and widgets from
//! different crates restyle together. [`UiTheme::resolve`] is the whole of
//! what a widget library has to call.
//!
//! [`UiStyle`] and [`StylistDisabled`] are the two escapes from it: patch one
//! entity's style, or take one entity's look over entirely.

use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::{Component, Resource};
use bevy_ecs::world::DeferredWorld;
use plurimus_core::ratatui_core::style::{Color, Modifier, Style};

use crate::stylist::StylistCache;

/// The interaction flags a widget is in, resolved by [`UiTheme::resolve`]
/// into one [`Style`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the four states of the documented precedence, not a config"
)]
#[non_exhaustive]
pub struct InteractionState {
    /// Cursor over the widget.
    pub hovered: bool,
    /// Pointer held on the widget.
    pub pressed: bool,
    /// The widget has [`InteractionDisabled`](crate::InteractionDisabled).
    pub disabled: bool,
    /// The widget holds input focus.
    pub focused: bool,
}

/// Per-state styles for widgets. Replace the resource to restyle;
/// `focused` is patched on top of the state style.
#[derive(Resource, Debug, Clone)]
#[non_exhaustive]
pub struct UiTheme {
    /// Idle widgets.
    pub normal: Style,
    /// Cursor over the widget.
    pub hovered: Style,
    /// Pointer held on the widget.
    pub pressed: Style,
    /// Widgets with [`InteractionDisabled`](crate::InteractionDisabled).
    pub disabled: Style,
    /// Patched over the state style while the widget has input focus.
    pub focused: Style,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractionState {
    /// Sets the cursor-over-the-widget term.
    #[must_use]
    pub const fn with_hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    /// Sets the pointer-held term.
    #[must_use]
    pub const fn with_pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// Sets the disabled term, which wins over every other.
    #[must_use]
    pub const fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the focus term, which patches over whichever other wins.
    #[must_use]
    pub const fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl UiTheme {
    /// The stock theme, and the seed the `with_*` builders patch. A
    /// `const fn` because an app's theme is often one too, and
    /// [`Default`] cannot be called from one.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            normal: Style::new(),
            hovered: Style::new().fg(Color::Cyan),
            pressed: Style::new().fg(Color::Black).bg(Color::Cyan),
            disabled: Style::new().fg(Color::DarkGray),
            focused: Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow),
        }
    }

    /// Sets the style for idle widgets.
    #[must_use]
    pub const fn with_normal(mut self, normal: Style) -> Self {
        self.normal = normal;
        self
    }

    /// Sets the style for a widget under the cursor.
    #[must_use]
    pub const fn with_hovered(mut self, hovered: Style) -> Self {
        self.hovered = hovered;
        self
    }

    /// Sets the style for a widget the pointer is held on.
    #[must_use]
    pub const fn with_pressed(mut self, pressed: Style) -> Self {
        self.pressed = pressed;
        self
    }

    /// Sets the style for a disabled widget.
    #[must_use]
    pub const fn with_disabled(mut self, disabled: Style) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets the patch applied over whichever state style wins while the
    /// widget holds focus.
    #[must_use]
    pub const fn with_focused(mut self, focused: Style) -> Self {
        self.focused = focused;
        self
    }

    /// The style for one interaction state: `disabled` wins over `pressed`
    /// over `hovered` over `normal`, and `focused` patches over whichever
    /// won.
    #[must_use]
    pub fn resolve(&self, state: InteractionState) -> Style {
        let base = if state.disabled {
            self.disabled
        } else if state.pressed {
            self.pressed
        } else if state.hovered {
            self.hovered
        } else {
            self.normal
        };
        if state.focused {
            base.patch(self.focused)
        } else {
            base
        }
    }
}

/// Exempts an entity from the stylist that would otherwise own it, leaving
/// its [`UiWidget`](crate::UiWidget) to the app.
///
/// The contract a stylist honors: do not rebuild the widget of an entity
/// carrying this. Behavior - selection, keys, scrolling, events - is not a
/// stylist's to touch and is unaffected either way.
///
/// Handing the entity back repaints it. Removing this resets the entity's
/// [`StylistCache`](crate::StylistCache) to its never-drawn value, because
/// change detection compares against a system's last run: an entity that
/// sat outside every stylist query missed the theme swaps that landed
/// meanwhile, and nothing else would mark it.
#[derive(Component, Debug, Clone, Copy)]
#[component(on_remove = invalidate_cache)]
pub struct StylistDisabled;

fn invalidate_cache(mut world: DeferredWorld, context: HookContext) {
    if let Some(mut cache) = world.get_mut::<StylistCache>(context.entity) {
        *cache = StylistCache::default();
    }
}

/// Patched over the style an entity would otherwise resolve to.
///
/// Patched rather than substituted, so an override setting only `bg` keeps
/// the theme's foreground and modifiers, and a widget carrying one still
/// shows hover and focus. A widget library decides what an entity's style
/// covers; where it draws rows from child entities, one on a row is that
/// row's.
#[derive(Component, Debug, Clone, Copy)]
pub struct UiStyle(pub Style);
