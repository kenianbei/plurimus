//! The theming contract between an app and a widget library.
//!
//! One [`UiTheme`] resource styles every widget that honors it, so restyling
//! an app is a resource swap rather than a per-widget edit. States are
//! resolved in a fixed precedence - disabled, then pressed, then hovered,
//! then normal - and `focused` patches on top of whichever won, which is why
//! a focused disabled widget still reads as disabled.
//!
//! [`UiStyle`] and [`StylistDisabled`] are the two escapes from it: patch one
//! entity's style, or take one entity's look over entirely.

use bevy_ecs::prelude::{Component, Resource};
use plurimus_core::ratatui_core::style::{Color, Modifier, Style};

/// Per-state styles for widgets. Replace the resource to restyle;
/// `focused` is patched on top of the state style.
#[derive(Resource, Debug, Clone)]
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
        Self {
            normal: Style::new(),
            hovered: Style::new().fg(Color::Cyan),
            pressed: Style::new().fg(Color::Black).bg(Color::Cyan),
            disabled: Style::new().fg(Color::DarkGray),
            focused: Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow),
        }
    }
}

/// The interaction flags a widget is in, resolved by [`UiTheme::resolve`]
/// into one [`Style`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the four states of the documented precedence, not a config"
)]
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

impl UiTheme {
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

/// Exempts an entity from the stock stylists, leaving its
/// [`UiWidget`](crate::UiWidget) to the app. Behavior - selection, keys,
/// scrolling, events - is untouched.
#[derive(Component, Debug, Clone, Copy)]
pub struct StylistDisabled;

/// Patched over the style an entity would otherwise resolve to.
///
/// Patched rather than substituted, so an override setting only `bg` keeps
/// the theme's foreground and modifiers, and a widget carrying one still
/// shows hover and focus. On the row child of a container drawn from rows it
/// styles that row, covering the full row width where a label's own line
/// style stops at the cursor gutter.
#[derive(Component, Debug, Clone, Copy)]
pub struct UiStyle(pub Style);
