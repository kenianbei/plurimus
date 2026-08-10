//! Terminal contract for plurimus: the messages crossing between an app and
//! whichever backend drives its terminal, in both directions.
//!
//! Inbound, a backend reports [`KeyMessage`], [`MouseMessage`],
//! [`PasteMessage`], [`FocusMessage`] and [`TerminalResized`], and
//! [`ButtonInput`] and [`CursorCell`] are the state derived from them;
//! [`InputCapabilities`] records which of it the terminal can actually
//! manage. Outbound, [`TerminalRequest`] is what an app asks of the
//! terminal and [`TerminalCursorStyle`] is the shape it asks the caret to
//! take.
//!
//! What is here is what needs a terminal to mean anything. Rendering to a
//! cell grid does not: `plurimus_core` drives any ratatui `Backend`,
//! terminal or otherwise, and never depends on this crate.

pub mod bevy_compat;
mod cursor;
mod keyboard;
mod mouse;
mod request;
mod resize;
mod state;
mod synthesis;

pub use cursor::TerminalCursorStyle;

pub use bevy_input::ButtonInput;
pub use bevy_input::mouse::MouseButton;
pub use keyboard::{KeyCode, KeyKind, KeyMessage, KeyModifiers, ModifierKey};
pub use mouse::{CursorCell, MouseKind, MouseMessage};
pub use request::{ClipboardTarget, TerminalRequest};
pub use resize::TerminalResized;
pub use state::InputSystems;
pub use synthesis::ReleaseTimeout;

pub(crate) use state::{track_cursor_cell, update_button_input};
pub(crate) use synthesis::synthesize_releases;

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::message::Message;
use bevy_ecs::prelude::{IntoScheduleConfigs, Resource};
use plurimus_core::CameraSystems;

/// Text pasted into the terminal (bracketed paste).
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct PasteMessage(pub String);

/// Terminal focus change.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusMessage {
    /// Whether the terminal gained focus.
    pub gained: bool,
}

/// What the active terminal backend can report.
///
/// Every capability defaults to present, so no synthesis runs without a
/// backend saying otherwise. Backends overwrite this after detection, and
/// should build their value from [`InputCapabilities::none`] rather than
/// [`Default`]: seeding from the permissive default claims any capability
/// added later without ever probing for it.
#[derive(Resource, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct InputCapabilities {
    /// Real key-release events arrive (kitty keyboard protocol).
    pub key_release: bool,
    /// Modifier keys arrive as [`KeyCode::Modifier`] events (kitty keyboard
    /// protocol). When unset, [`bevy_compat::forward_keyboard`] synthesizes
    /// modifier transitions from [`KeyModifiers`] instead.
    pub modifier_keys: bool,
}

impl InputCapabilities {
    /// Every capability absent: the seed a backend refines with what it
    /// detected.
    ///
    /// [`Default`] is the opposite - every capability present - which is
    /// what an app with no backend attached needs. Seeding detection from
    /// this instead means a capability added later reads as absent until a
    /// backend probes for it, rather than being claimed unasked.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            key_release: false,
            modifier_keys: false,
        }
    }

    /// Sets whether real key-release events arrive.
    #[must_use]
    pub const fn with_key_release(mut self, reported: bool) -> Self {
        self.key_release = reported;
        self
    }

    /// Sets whether modifier keys arrive as their own key events.
    #[must_use]
    pub const fn with_modifier_keys(mut self, reported: bool) -> Self {
        self.modifier_keys = reported;
        self
    }
}

impl Default for InputCapabilities {
    fn default() -> Self {
        Self {
            key_release: true,
            modifier_keys: true,
        }
    }
}

/// Registers the terminal messages, polled input state, and the
/// `PreUpdate` phases backends and consumers order against.
///
/// Requires [`plurimus_core::CorePlugin`] to be added first: applying a
/// resize writes core's `TerminalSize`.
pub struct TermPlugin;

impl Plugin for TermPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy_time::TimePlugin>() {
            app.add_plugins(bevy_time::TimePlugin);
        }
        app.init_resource::<CursorCell>();
        app.init_resource::<TerminalCursorStyle>();
        app.init_resource::<InputCapabilities>();
        app.init_resource::<ReleaseTimeout>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<ButtonInput<MouseButton>>();
        app.add_message::<KeyMessage>();
        app.add_message::<MouseMessage>();
        app.add_message::<PasteMessage>();
        app.add_message::<FocusMessage>();
        app.add_message::<TerminalRequest>();
        app.add_message::<TerminalResized>();
        app.configure_sets(
            PreUpdate,
            (InputSystems::Pump, InputSystems::Update).chain(),
        );
        app.add_systems(
            PreUpdate,
            (synthesize_releases, update_button_input, track_cursor_cell)
                .chain()
                .in_set(InputSystems::Update),
        );
        // Into core's own set, so a resize reported this frame reaches the
        // cameras that resolve against it in the same frame.
        app.add_systems(
            PreUpdate,
            resize::apply_terminal_resize.in_set(CameraSystems::SyncSize),
        );
    }
}
