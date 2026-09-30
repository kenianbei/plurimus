//! The two text widgets and everything only they use.
//!
//! [`EditableText`] is a single-line field owning its value as a plain
//! component; [`TextEditor`] is a multi-line box whose text lives in a
//! ratatui-textarea engine behind a lock. That difference is deliberate - a
//! form field is read with `entity.get::<TextInput>()`, while an editor
//! trades that for undo, lines and wrapping - and it is why the two do not
//! share an implementation.
//!
//! `grapheme` and `word` are shared between them so a keybinding stops at
//! the same place in both, and `field` renders the single-line row; none of
//! the three is reachable from outside this module. `edit` is the exception:
//! it holds [`TextInput`]'s own key and paste entry points, which are public
//! so a host that routes its own keys drives a field without focusing it.

mod clipboard;
mod edit;
mod editor;
mod editor_keys;
mod field;
mod grapheme;
mod input;
mod keys;
mod pointer;
mod state;
mod word;

pub use editor::{TextChanged, TextEditor, text_editor};
pub use editor_keys::{TextEditorAction, TextEditorKeys};
pub use input::{EditableText, Submit, TextMask, editable_text};
pub use keys::{TextInputAction, TextInputKeys};
pub use state::TextInput;

pub(crate) use editor::{
    install_editor_views, text_editor_key, text_editor_paste, text_editor_scrolled,
};
pub(crate) use input::{
    release_text_input_caret, style_text_inputs, text_input_blur, text_input_key, text_input_paste,
};
pub(crate) use pointer::{text_input_drag, text_input_press};
