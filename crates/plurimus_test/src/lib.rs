//! Test support for plurimus: input injection and composed-frame
//! snapshots. Dev-dependency only; never ships in a consumer build.

mod frame;
mod input;
mod request;
mod widget;

pub use frame::{composed_frame, composed_styled_frame};
pub use input::{
    character, click, click_button, move_to, press_at, press_button_at, press_key, press_key_with,
    release_at, release_button_at, repeat_key, scroll_at, scroll_sideways, send_focus, send_paste,
    send_press_at, set_focus, write_focus, write_key, write_move_to, write_paste, write_press_at,
    write_press_at_with, write_release_at, write_repeat, write_scroll_at,
};
pub use request::clipboard_writes;
pub use widget::widget_content;
