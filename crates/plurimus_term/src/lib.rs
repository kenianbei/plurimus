//! Terminal contract for plurimus: the messages crossing between an app and
//! whichever backend drives its terminal, in both directions.
//!
//! Inbound, a backend reports keys, the pointer and focus through
//! [`TerminalInput`], which writes bevy's own messages for the virtual
//! primary window, and [`PasteMessage`] and [`TerminalResized`] as itself;
//! [`InputCapabilities`] records which of it the terminal can actually
//! manage, and [`HeldModifiers`] and [`PointerModifiers`] read back the
//! modifiers bevy's messages carry no field for. What no terminal reports at all - a key release below
//! the kitty tier, anything while unfocused - is synthesized against the
//! real clock. Outbound, [`TerminalRequest`] is what an app asks of the
//! terminal and [`TerminalCursorStyle`] is the shape it asks the caret to
//! take.
//!
//! What is here is what needs a terminal to mean anything. Rendering to a
//! cell grid does not: `plurimus_core` drives any ratatui `Backend`,
//! terminal or otherwise, and never depends on this crate.

mod clipboard;
mod cursor;
mod input;
mod keyboard;
mod keymap;
mod request;
mod resize;
mod synthesis;

pub use clipboard::Clipboard;
pub use cursor::TerminalCursorStyle;

pub use bevy_input;
pub use bevy_input::ButtonInput;
pub use bevy_input::mouse::MouseButton;
pub use input::{InputSystems, PointerModifiers, TerminalInput, VirtualWindow};
pub use keyboard::{HeldModifiers, KeyModifiers, held_modifiers};
pub use keymap::physical_code;
pub use request::{ClipboardTarget, LastCopied, RequestSystems, TerminalRequest};
pub use resize::TerminalResized;
pub use synthesis::ReleaseTimeout;

pub(crate) use input::PendingInput;
pub(crate) use request::echo_clipboard_writes;
pub(crate) use synthesis::{
    HeldKeys, expire_held_keys, record_held_keys, release_buttons_on_focus_loss,
    release_keys_on_focus_loss, releases_are_synthesized,
};

use bevy_app::{App, First, Last, Plugin, PreStartup, PreUpdate};
use bevy_ecs::message::Message;
use bevy_ecs::prelude::{IntoScheduleConfigs, Resource};
use bevy_input::InputPlugin;
use bevy_window::{CursorMoved, WindowEvent, WindowFocused, WindowResized};
use plurimus_core::CameraSystems;

/// Text pasted into the terminal (bracketed paste).
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct PasteMessage(pub String);

/// What the active terminal backend can report.
///
/// Every capability defaults to present, so no synthesis runs without a
/// backend saying otherwise. Backends overwrite this after detection, and
/// should build their value from [`InputCapabilities::none`] rather than
/// [`Default`]: seeding from the permissive default claims any capability
/// added later without ever probing for it. A backend that can only learn a
/// capability from what the terminal sends raises it later, so an app
/// reading this at startup may see a capability absent that arrives with the
/// first event proving it.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InputCapabilities {
    /// Real key-release events arrive (kitty keyboard protocol).
    pub key_release: bool,
    /// Modifier keys arrive as key events of their own (kitty keyboard
    /// protocol). When unset, [`TerminalInput`] synthesizes their presses
    /// and releases from the [`KeyModifiers`] bits each event carries.
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

/// Registers the terminal messages, `bevy_input`'s polled state, the
/// [`VirtualWindow`] every message names where the app opened none, and the
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
        if !app.is_plugin_added::<InputPlugin>() {
            app.add_plugins(InputPlugin);
        }
        app.init_resource::<TerminalCursorStyle>();
        app.init_resource::<InputCapabilities>();
        app.init_resource::<ReleaseTimeout>();
        app.init_resource::<HeldKeys>();
        app.init_resource::<LastCopied>();
        app.init_resource::<PointerModifiers>();
        app.init_resource::<PendingInput>();
        app.add_message::<PasteMessage>();
        app.add_message::<TerminalRequest>();
        app.add_message::<TerminalResized>();
        app.add_message::<WindowEvent>();
        app.add_message::<WindowFocused>();
        app.add_message::<WindowResized>();
        app.add_message::<CursorMoved>();
        app.add_systems(PreStartup, input::spawn_virtual_window);
        add_schedule(app);
    }
}

fn add_schedule(app: &mut App) {
    app.configure_sets(
        First,
        (InputSystems::Pump, InputSystems::Synthesize)
            .chain()
            .after(bevy_time::TimeSystems)
            .after(bevy_ecs::message::MessageUpdateSystems),
    );
    app.add_systems(
        First,
        (
            // Recording this frame's presses first, so a focus lost in
            // the same frame releases them too.
            (
                record_held_keys,
                expire_held_keys.run_if(releases_are_synthesized),
                release_keys_on_focus_loss,
            )
                .chain()
                .in_set(InputSystems::Synthesize),
        ),
    );
    app.add_systems(
        PreUpdate,
        (
            // Into core's own set, so a resize reported this frame reaches
            // the cameras that resolve against it in the same frame.
            (resize::apply_terminal_resize, resize::size_virtual_window)
                .chain()
                .in_set(CameraSystems::SyncSize),
            // After bevy applies the frame's presses, so one pumped with the
            // focus loss is released too, and inside its set, so focus
            // dispatch reads the settled state.
            release_buttons_on_focus_loss
                .in_set(bevy_input::InputSystems)
                .after(bevy_input::mouse::mouse_button_input_system),
        ),
    );
    // At the frame's end, so what a press held back lands for the whole
    // of the next frame, before anything reads the stream.
    app.add_systems(
        Last,
        (
            input::flush_pending,
            echo_clipboard_writes.in_set(RequestSystems::Echo),
        ),
    );
}
