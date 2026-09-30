//! What a key does to a [`TextEditor`](super::TextEditor), as data rather
//! than the engine's own keymap.

use bevy_ecs::prelude::Component;
use bevy_input::keyboard::Key;
use plurimus_ui::KeyBinding;
use ratatui_textarea::{CursorMove, Scrolling};

/// One step a [`TextEditorKeys`] binding asks a
/// [`TextEditor`](super::TextEditor)'s engine for.
///
/// Motions and scrolls carry the engine's own [`CursorMove`] and
/// [`Scrolling`], so every place the engine can move to is bindable.
/// Inserting a character is not here, being what an unbound key does rather
/// than something bound.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEditorAction {
    /// End any selection and move.
    Move(CursorMove),
    /// Start a selection if there is none, and move, extending it.
    Select(CursorMove),
    /// End any selection and scroll, the cursor kept inside the viewport.
    Scroll(Scrolling),
    /// Break the line at the cursor.
    Newline,
    /// Insert a tab, or the spaces the engine's tab width asks for.
    InsertTab,
    /// Delete the selection, or the cluster left of the cursor.
    Backspace,
    /// Delete the selection, or the cluster right of the cursor.
    Delete,
    /// Delete back to the start of the word left of the cursor.
    DeleteWord,
    /// Delete forward to the end of the word right of the cursor.
    DeleteNextWord,
    /// Delete to the end of the line, into the engine's kill ring.
    DeleteToLineEnd,
    /// Delete to the start of the line, into the engine's kill ring.
    DeleteToLineHead,
    /// Undo the last edit.
    Undo,
    /// Redo the last undone edit.
    Redo,
    /// Insert what the engine's kill ring holds.
    Yank,
    /// Copy the selection, offering it to the terminal as well.
    Copy,
    /// Cut the selection, offering it to the terminal as well.
    Cut,
    /// Insert what was last copied anywhere in the app.
    Paste,
    /// Select the whole text.
    SelectAll,
    /// End the selection, leaving the cursor where it is.
    CancelSelection,
}

/// A [`TextEditor`](super::TextEditor)'s key bindings, scanned in order so
/// the first match wins.
///
/// Replace it to remap: two keys may share an action by appearing twice, a
/// key bound to nothing inserts itself if it is an unchorded character, and
/// an unbound chord propagates. Defaults to the conventional set: the arrows,
/// `Home` and `End`, and `Ctrl` with them to move by word, paragraph and to
/// either end, each selecting with `Shift`; `PageUp` and `PageDown`; `Enter`;
/// `Backspace` and `Delete`, shifted or not, and by word with `Alt` as the
/// single-line field binds them; `Ctrl` with `z` and `y` to undo and redo, `c`, `x` and `v` for
/// the clipboard, and `a` to select all; and `Escape` to end a selection.
/// `Tab` is left to focus navigation.
#[derive(Component, Debug, Clone)]
pub struct TextEditorKeys(pub Vec<(KeyBinding, TextEditorAction)>);

/// Each motion the default binds, moving plain and selecting with shift.
const MOTIONS: [(KeyBinding, CursorMove); 12] = [
    (KeyBinding::new(Key::ArrowLeft), CursorMove::Back),
    (KeyBinding::new(Key::ArrowRight), CursorMove::Forward),
    (KeyBinding::new(Key::ArrowUp), CursorMove::Up),
    (KeyBinding::new(Key::ArrowDown), CursorMove::Down),
    (KeyBinding::new(Key::Home), CursorMove::Head),
    (KeyBinding::new(Key::End), CursorMove::End),
    (ctrl(Key::ArrowLeft), CursorMove::WordBack),
    (ctrl(Key::ArrowRight), CursorMove::WordForward),
    (ctrl(Key::ArrowUp), CursorMove::ParagraphBack),
    (ctrl(Key::ArrowDown), CursorMove::ParagraphForward),
    (ctrl(Key::Home), DOCUMENT_START),
    (ctrl(Key::End), DOCUMENT_END),
];

/// The first cell of the first line; the engine's `Top` keeps the column.
const DOCUMENT_START: CursorMove = CursorMove::Jump(0, 0);

/// The last cell of the last line, the engine clamping the jump to both.
const DOCUMENT_END: CursorMove = CursorMove::Jump(u16::MAX, u16::MAX);

impl Default for TextEditorKeys {
    fn default() -> Self {
        let moving = MOTIONS.into_iter().flat_map(|(binding, motion)| {
            [
                (binding.clone(), TextEditorAction::Move(motion)),
                (binding.with_shift(), TextEditorAction::Select(motion)),
            ]
        });
        let editing = [
            (
                Key::PageUp.into(),
                TextEditorAction::Scroll(Scrolling::PageUp),
            ),
            (
                Key::PageDown.into(),
                TextEditorAction::Scroll(Scrolling::PageDown),
            ),
            (Key::Enter.into(), TextEditorAction::Newline),
            (
                KeyBinding::new(Key::Backspace).with_alt(),
                TextEditorAction::DeleteWord,
            ),
            (
                KeyBinding::new(Key::Delete).with_alt(),
                TextEditorAction::DeleteNextWord,
            ),
            (Key::Backspace.into(), TextEditorAction::Backspace),
            (Key::Delete.into(), TextEditorAction::Delete),
            (
                KeyBinding::new(Key::Backspace).with_shift(),
                TextEditorAction::Backspace,
            ),
            (
                KeyBinding::new(Key::Delete).with_shift(),
                TextEditorAction::Delete,
            ),
            (ctrl_char("z"), TextEditorAction::Undo),
            (ctrl_char("y"), TextEditorAction::Redo),
            (ctrl_char("c"), TextEditorAction::Copy),
            (ctrl_char("x"), TextEditorAction::Cut),
            (ctrl_char("v"), TextEditorAction::Paste),
            (ctrl_char("a"), TextEditorAction::SelectAll),
            (Key::Escape.into(), TextEditorAction::CancelSelection),
        ];
        Self(moving.chain(editing).collect())
    }
}

pub(super) const fn ctrl(key: Key) -> KeyBinding {
    KeyBinding::new(key).with_ctrl()
}

pub(super) fn ctrl_char(character: &str) -> KeyBinding {
    ctrl(Key::Character(character.into()))
}
