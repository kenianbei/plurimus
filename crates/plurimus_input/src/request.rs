//! What an app asks the terminal to do, as opposed to what the terminal
//! reports.
//!
//! One message carries every one-shot request rather than a type per
//! capability: there is exactly one reader - whichever backend is
//! installed - so separate channels would buy no filtering and would leave
//! two requests written in the same frame with no defined order on a stream
//! where order is the whole point.

use bevy_ecs::prelude::Message;

/// A one-shot terminal side effect an app asks the active backend for.
///
/// Best-effort by nature. A backend that cannot serve a variant drops it,
/// and nothing here can be confirmed - OSC 52 has no acknowledgement - so a
/// request is sent rather than done. Which variants a backend honors, and
/// what it needs enabling first, is the backend crate's documentation:
/// `plurimus_crossterm` gates copying behind `CrosstermPlugin::clipboard`.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TerminalRequest {
    /// Copy `content` to a clipboard selection.
    CopyToClipboard {
        /// Text to place on the clipboard.
        content: String,
        /// Which selection to write.
        destination: ClipboardTarget,
    },
    /// Set the terminal window's title.
    SetTitle(String),
}

impl TerminalRequest {
    /// Copies `content` to the ordinary clipboard - what a user means by
    /// copy. [`ClipboardTarget::Primary`] needs the full variant.
    #[must_use]
    pub fn copy(content: impl Into<String>) -> Self {
        Self::CopyToClipboard {
            content: content.into(),
            destination: ClipboardTarget::Clipboard,
        }
    }
}

/// Which clipboard selection a copy writes to.
///
/// Carried per request rather than configured once, so an editor can put a
/// dragged selection on [`Primary`](Self::Primary) and an explicit copy on
/// [`Clipboard`](Self::Clipboard) - the X11 idiom, which one shared setting
/// could not express.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClipboardTarget {
    /// The clipboard a paste reads from.
    #[default]
    Clipboard,
    /// The X11 and Wayland primary selection, pasted with the middle
    /// mouse button. Meaningless elsewhere, and ignored where it is.
    Primary,
}
