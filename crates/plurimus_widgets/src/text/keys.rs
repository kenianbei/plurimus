//! What a key does to a [`TextInput`], as data rather than a closed match.

use bevy_input::keyboard::Key;
use plurimus_ui::KeyBinding;

use bevy_ecs::prelude::Component;

/// One editing step a [`TextInputKeys`] binding asks a
/// [`TextInput`](super::TextInput) for.
///
/// Closed: these are every motion and deletion the field has, and a field
/// that grew a new one would be a new field. Inserting a character is not
/// here, being what an unbound key does rather than something bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputAction {
    /// One cluster left.
    Left,
    /// One cluster right.
    Right,
    /// To the start of the word left of the cursor.
    WordLeft,
    /// To the start of the word right of the cursor.
    WordRight,
    /// To the start of the value.
    Home,
    /// To the end of the value.
    End,
    /// Delete the cluster left of the cursor.
    Backspace,
    /// Delete the cluster right of the cursor.
    Delete,
    /// Delete back to the start of the word left of the cursor.
    WordBackspace,
    /// Delete forward to the end of the word right of the cursor.
    WordDelete,
    /// Commit the value.
    ///
    /// The one action [`TextInput::handle`](super::TextInput::handle) does
    /// not apply: what committing means - emitting, closing a dialog,
    /// running a search - is the dispatcher's, so `handle` leaves it untaken
    /// and whoever routes the key acts on it. The stock observer emits a
    /// final `ValueChange` and a `Submit`.
    Submit,
}

/// An [`EditableText`](super::EditableText)'s key bindings, scanned in order
/// so the first match wins.
///
/// Replace it to remap: two keys may share an action by appearing twice, and
/// a key bound to nothing inserts itself if it is an unchorded character and
/// propagates otherwise. Defaults to the arrows and `Home`/`End`, `Ctrl` with
/// the arrows for word motion and `Alt` with `Backspace`/`Delete` for word
/// deletion - which mirror the multi-line editor's engine - and `Enter` to
/// submit.
#[derive(Component, Debug, Clone)]
pub struct TextInputKeys(pub Vec<(KeyBinding, TextInputAction)>);

impl Default for TextInputKeys {
    fn default() -> Self {
        Self(vec![
            (
                KeyBinding::new(Key::ArrowLeft).with_ctrl(),
                TextInputAction::WordLeft,
            ),
            (
                KeyBinding::new(Key::ArrowRight).with_ctrl(),
                TextInputAction::WordRight,
            ),
            (
                KeyBinding::new(Key::Backspace).with_alt(),
                TextInputAction::WordBackspace,
            ),
            (
                KeyBinding::new(Key::Delete).with_alt(),
                TextInputAction::WordDelete,
            ),
            (Key::ArrowLeft.into(), TextInputAction::Left),
            (Key::ArrowRight.into(), TextInputAction::Right),
            (Key::Backspace.into(), TextInputAction::Backspace),
            (Key::Delete.into(), TextInputAction::Delete),
            (Key::Home.into(), TextInputAction::Home),
            (Key::End.into(), TextInputAction::End),
            (Key::Enter.into(), TextInputAction::Submit),
        ])
    }
}
