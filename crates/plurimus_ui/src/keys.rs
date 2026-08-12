//! The shared shape of a widget's key bindings.
//!
//! A widget that takes movement keys holds them as data rather than a
//! closed match, so an app remaps it without reimplementing the movement
//! beside it. The bindings live on the widget's own component - the
//! action type differs per widget - and this is the scan they share.

use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};

/// The action bound to `input`'s key, or `None` if nothing is.
///
/// Scanned in order so the first match wins, which is what lets an app
/// shadow one binding by putting it ahead of the rest. A release binds to
/// nothing; a repeat binds as the press it is, so a held key keeps
/// acting. A widget that must act once per physical press checks
/// [`KeyboardInput::repeat`] itself.
#[must_use]
pub fn first_bound<A: Copy>(bindings: &[(Key, A)], input: &KeyboardInput) -> Option<A> {
    if input.state != ButtonState::Pressed {
        return None;
    }
    bindings
        .iter()
        .find(|(key, _)| *key == input.logical_key)
        .map(|(_, action)| *action)
}
