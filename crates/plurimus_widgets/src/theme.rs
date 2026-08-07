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

impl UiTheme {
    #[expect(
        clippy::fn_params_excessive_bools,
        reason = "the four states of the documented precedence; a struct of them trips struct_excessive_bools instead"
    )]
    pub(crate) fn resolve(
        &self,
        hovered: bool,
        pressed: bool,
        disabled: bool,
        focused: bool,
    ) -> Style {
        let base = if disabled {
            self.disabled
        } else if pressed {
            self.pressed
        } else if hovered {
            self.hovered
        } else {
            self.normal
        };
        if focused {
            base.patch(self.focused)
        } else {
            base
        }
    }
}
