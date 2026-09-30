//! What a key does to a [`TextInput`], as data rather than a closed match.

use bevy_input::keyboard::Key;
use plurimus_term::KeyModifiers;
use plurimus_ui::KeyBinding;

use bevy_ecs::prelude::Component;

use super::editor_keys::{ctrl, ctrl_char};

/// One editing step a [`TextInputKeys`] binding asks a
/// [`TextInput`](super::TextInput) for.
///
/// Closed, though that makes a new action a breaking change: [`Submit`],
/// [`Copy`], [`Cut`] and [`Paste`] are actions a host routing its own keys
/// has to carry out itself, and a `_` arm would hide the next one from it.
/// Inserting a character is not here, being what an unbound key does rather
/// than something bound.
///
/// [`Submit`]: Self::Submit
/// [`Copy`]: Self::Copy
/// [`Cut`]: Self::Cut
/// [`Paste`]: Self::Paste
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
    /// Extend the selection one cluster left.
    SelectLeft,
    /// Extend the selection one cluster right.
    SelectRight,
    /// Extend the selection to the start of the word left of the cursor.
    SelectWordLeft,
    /// Extend the selection to the start of the word right of the cursor.
    SelectWordRight,
    /// Extend the selection to the start of the value.
    SelectHome,
    /// Extend the selection to the end of the value.
    SelectEnd,
    /// Select the whole value.
    SelectAll,
    /// End the selection, leaving the cursor where it is.
    CancelSelection,
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
    /// Copy the selection to the clipboard.
    ///
    /// Left untaken by [`TextInput::handle`](super::TextInput::handle), as
    /// [`Submit`](Self::Submit) is, since the clipboard is the dispatcher's:
    /// the stock observer sends
    /// [`TerminalRequest::copy`](plurimus_term::TerminalRequest::copy) of
    /// [`selected_text`](super::TextInput::selected_text), unless the field
    /// is masked.
    Copy,
    /// Copy the selection, then delete it; left untaken like
    /// [`Copy`](Self::Copy).
    Cut,
    /// Insert what was last copied; left untaken like [`Copy`](Self::Copy).
    /// The stock observer pastes [`LastCopied`](plurimus_term::LastCopied).
    Paste,
}

/// An [`EditableText`](super::EditableText)'s key bindings, scanned in order
/// so the first match wins.
///
/// Replace it to remap: two keys may share an action by appearing twice, and
/// a key bound to nothing inserts itself if it is an unchorded character and
/// propagates otherwise. Defaults to the arrows and `Home`/`End`, `Ctrl` with
/// the arrows for word motion, each selecting with `Shift`; `Alt` with
/// `Backspace`/`Delete` for word deletion; `Ctrl` with `a` to select all and
/// `c`, `x` and `v` for the clipboard - as
/// [`TextEditorKeys`](super::TextEditorKeys) binds them - and `Enter` to
/// submit. [`CancelSelection`](TextInputAction::CancelSelection) is left
/// unbound, so `Escape` still reaches whatever holds the field.
#[derive(Component, Debug, Clone)]
pub struct TextInputKeys(pub Vec<(KeyBinding, TextInputAction)>);

/// Each motion the default binds: the key, what it does plain, and what it
/// does with `Shift`.
const MOTIONS: [(KeyBinding, TextInputAction, TextInputAction); 6] = [
    (
        ctrl(Key::ArrowLeft),
        TextInputAction::WordLeft,
        TextInputAction::SelectWordLeft,
    ),
    (
        ctrl(Key::ArrowRight),
        TextInputAction::WordRight,
        TextInputAction::SelectWordRight,
    ),
    (
        KeyBinding::new(Key::ArrowLeft),
        TextInputAction::Left,
        TextInputAction::SelectLeft,
    ),
    (
        KeyBinding::new(Key::ArrowRight),
        TextInputAction::Right,
        TextInputAction::SelectRight,
    ),
    (
        KeyBinding::new(Key::Home),
        TextInputAction::Home,
        TextInputAction::SelectHome,
    ),
    (
        KeyBinding::new(Key::End),
        TextInputAction::End,
        TextInputAction::SelectEnd,
    ),
];

impl Default for TextInputKeys {
    fn default() -> Self {
        let moving = MOTIONS.into_iter().flat_map(|(binding, plain, shifted)| {
            [(binding.clone(), plain), (binding.with_shift(), shifted)]
        });
        let editing = [
            (
                KeyBinding::new(Key::Backspace).with_alt(),
                TextInputAction::WordBackspace,
            ),
            (
                KeyBinding::new(Key::Delete).with_alt(),
                TextInputAction::WordDelete,
            ),
            (Key::Backspace.into(), TextInputAction::Backspace),
            (Key::Delete.into(), TextInputAction::Delete),
            (Key::Enter.into(), TextInputAction::Submit),
            (ctrl_char("a"), TextInputAction::SelectAll),
            (ctrl_char("c"), TextInputAction::Copy),
            (ctrl_char("x"), TextInputAction::Cut),
            (ctrl_char("v"), TextInputAction::Paste),
        ];
        Self(moving.chain(editing).collect())
    }
}

/// What an unbound key types into either text widget, which is nothing
/// unless it is an unchorded character.
///
/// Shift is not a chord: the kitty protocol reports a shifted letter with
/// the bit set, so blocking it would stop capitals.
pub(super) fn unbound_text(key: &Key, held: KeyModifiers) -> Option<&str> {
    if held.ctrl || held.alt || held.super_key || held.hyper || held.meta {
        return None;
    }
    match key {
        Key::Character(characters) => Some(characters.as_str()),
        Key::Space => Some(" "),
        _ => None,
    }
}
