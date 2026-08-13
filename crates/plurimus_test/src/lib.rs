//! Test support for plurimus: input injection and composed-frame
//! snapshots. Dev-dependency only; never ships in a consumer build.

mod frame;
mod input;
mod request;
mod widget;

pub use frame::{composed_frame, composed_styled_frame};
pub use input::{
    click, press_chord, press_key, press_key_with, repeat_key, send_focus, send_mouse, write_focus,
    write_key, write_mouse,
};
pub use request::clipboard_writes;
pub use widget::widget_content;
