//! The clipboard every widget with something to copy writes to, and the
//! text widgets paste from.

use bevy_ecs::prelude::{MessageWriter, Res};
use bevy_ecs::system::SystemParam;
use plurimus_term::{LastCopied, TerminalRequest};

/// The clipboard both ways: what a widget offers the terminal, and what a
/// text widget pastes from.
#[derive(SystemParam)]
pub(crate) struct Clipboard<'w> {
    requests: MessageWriter<'w, TerminalRequest>,
    copied: Res<'w, LastCopied>,
}

impl Clipboard<'_> {
    /// Sends `text` to the terminal, unless it is empty - an empty copy
    /// would take away whatever the user last put there. Answers whether
    /// anything was sent.
    pub(crate) fn offer(&mut self, text: &str) -> bool {
        let is_sent = !text.is_empty();
        if is_sent {
            self.requests.write(TerminalRequest::copy(text));
        }
        is_sent
    }

    /// What was last copied, by any widget, for a paste to insert.
    pub(crate) fn last_copied(&self) -> Option<&str> {
        self.copied.0.as_deref()
    }
}
