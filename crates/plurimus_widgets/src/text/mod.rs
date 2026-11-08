//! The two text widgets and everything only they use.
//!
//! [`TextField`] is a single-line field owning its value as a plain
//! component; [`TextEditor`] is a multi-line box whose text lives in a
//! ratatui-textarea engine behind a lock. That difference is deliberate - a
//! form field is read with `entity.get::<TextInput>()`, while an editor
//! trades that for undo, lines and wrapping - and it is why the two do not
//! share an implementation.
//!
//! The field's engine, [`TextInput`], is `plurimus_ui`'s, shared with every
//! other host of a single-line field; this module is the cell widget around
//! it: `input` its keys, paste, focus and stylist, `pointer` its press and
//! drag.

mod editor;
mod editor_keys;
mod input;
mod pointer;

pub use editor::{TextChanged, TextEditor, text_editor};
pub use editor_keys::{TextEditorAction, TextEditorKeys};
pub use input::{Submit, TextField, TextMask, text_field};
pub use plurimus_ui::{TextInput, TextInputAction, TextInputKeys};

pub(crate) use editor::{
    install_editor_views, text_editor_key, text_editor_paste, text_editor_scrolled,
};
pub(crate) use input::{
    release_text_input_caret, style_text_inputs, text_input_blur, text_input_key, text_input_paste,
};
pub(crate) use pointer::{text_input_drag, text_input_press};
