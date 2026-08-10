//! The theme consumed by the stock stylist systems.
//!
//! One [`UiTheme`] resource styles every stock widget, so restyling an app is
//! a resource swap rather than a per-widget edit. States are resolved in a
//! fixed precedence - disabled, then pressed, then hovered, then normal - and
//! `focused` patches on top of whichever won, which is why a focused disabled
//! widget still reads as disabled.

use bevy_ecs::prelude::Resource;
use plurimus_core::ratatui_core::style::{Color, Modifier, Style};

/// Per-state styles for the standard widgets. Replace the resource to
/// restyle; `focused` is patched on top of the state style.
#[derive(Resource, Debug, Clone)]
pub struct UiTheme {
    /// Idle widgets.
    pub normal: Style,
    /// Cursor over the widget.
    pub hovered: Style,
    /// Pointer held on the widget.
    pub pressed: Style,
    /// Widgets with `InteractionDisabled`.
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

/// The interaction flags a widget is in, resolved by
/// [`UiTheme::resolve`] into one [`Style`].
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
    /// The widget has `InteractionDisabled`.
    pub disabled: bool,
    /// The widget holds input focus.
    pub focused: bool,
}

impl UiTheme {
    pub(crate) fn resolve(&self, state: InteractionState) -> Style {
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
