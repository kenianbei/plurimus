//! The single-line text engine: [`TextInput`], the value, cursor and
//! selection every single-line field edits, and what it draws.
//!
//! It lives here rather than with a widget so each host of a field shares
//! it: `plurimus_widgets`' cell `TextField` and `plurimus_bui`'s upstream
//! `TextInput` node both edit through it, and a multi-line editor steps by
//! the same clusters and types by the same [`unbound_text`] rule.

mod edit;
mod grapheme;
mod keys;
mod row;
mod state;
mod word;

pub use grapheme::{cluster_len_after, cluster_len_before};
pub use keys::{TextInputAction, TextInputKeys, unbound_text};
pub use row::{DrawnRow, TextRow};
pub use state::TextInput;
