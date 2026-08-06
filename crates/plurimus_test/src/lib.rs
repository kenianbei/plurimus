//! Test support for plurimus: input injection and composed-frame
//! snapshots. Dev-dependency only; never ships in a consumer build.

mod frame;
mod input;

pub use frame::{composed_frame, composed_styled_frame};
pub use input::{
    click, press_chord, press_key, press_key_with, send_mouse, write_key, write_mouse,
};
