//! The clipboard both text widgets copy to and paste from.

use bevy_ecs::prelude::{MessageWriter, Res};
use bevy_ecs::system::SystemParam;
use plurimus_term::{LastCopied, TerminalRequest};

/// The clipboard both ways: what a text widget offers the terminal, and
/// what it pastes from.
#[derive(SystemParam)]
pub(crate) struct Clipboard<'w> {
    requests: MessageWriter<'w, TerminalRequest>,
    copied: Res<'w, LastCopied>,
}

impl Clipboard<'_> {
    /// Sends `text` to the terminal, unless it is empty - an empty copy
    /// would take away whatever the user last put there.
    pub(super) fn offer(&mut self, text: &str) {
        if !text.is_empty() {
            self.requests.write(TerminalRequest::copy(text));
        }
    }

    /// What was last copied, by any widget, for a paste to insert.
    pub(super) fn last_copied(&self) -> Option<&str> {
        self.copied.0.as_deref()
    }
}
