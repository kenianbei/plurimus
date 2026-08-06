//! The two text widgets and everything only they use.
//!
//! [`EditableText`] is a single-line field owning its value as a plain
//! component; [`TextEditor`] is a multi-line box whose text lives in a
//! ratatui-textarea engine behind a lock. That difference is deliberate - a
//! form field is read with `entity.get::<TextInput>()`, while an editor
//! trades that for undo, selection and wrapping - and it is why the two do
//! not share an implementation.
//!
//! `grapheme` and `word` are shared between them so a keybinding stops at
//! the same place in both, and `field` renders the single-line row. None of
//! the three is reachable from outside this module.

mod editor;
mod field;
mod grapheme;
mod input;
mod state;
mod word;

pub use editor::{TextChanged, TextEditor, text_editor};
pub use input::{EditableText, editable_text};
pub use state::TextInput;

pub(crate) use editor::{
    install_editor_views, text_editor_key, text_editor_paste, text_editor_wheel,
};
pub(crate) use input::{style_text_inputs, text_input_blur, text_input_key, text_input_paste};
