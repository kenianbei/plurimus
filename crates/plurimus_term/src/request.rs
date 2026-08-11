//! What an app asks the terminal to do, as opposed to what the terminal
//! reports.
//!
//! One message carries every one-shot request rather than a type per
//! capability: there is exactly one reader - whichever backend is
//! installed - so separate channels would buy no filtering and would leave
//! two requests written in the same frame with no defined order on a stream
//! where order is the whole point.

use bevy_ecs::prelude::{Message, MessageReader, ResMut, Resource, SystemSet};

/// A one-shot terminal side effect an app asks the active backend for.
///
/// Best-effort by nature. A backend that cannot serve a variant drops it,
/// and nothing here can be confirmed - OSC 52 has no acknowledgement - so a
/// request is sent rather than done. Which variants a backend honors, and
/// what it needs enabling first, is the backend crate's documentation:
/// `plurimus_crossterm` gates copying behind `CrosstermPlugin::clipboard`
/// and drops a copy too large for one escape sequence.
///
/// One-way, and stays that way: there is no variant that reads the
/// clipboard back. The escape that would is widely disabled as a
/// data-exfiltration risk, and a reply would have to be parsed out of the
/// input stream, which no backend here does. Text arrives by
/// [`PasteMessage`](crate::PasteMessage) when the user pastes it.
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

/// What was last asked of the clipboard, for a widget that needs a paste
/// key.
///
/// An echo of the request stream, not a reading of the terminal. Nothing
/// here confirms the clipboard agrees: a backend may drop a copy it cannot
/// serve, `plurimus_crossterm` writes none at all until
/// `CrosstermPlugin::clipboard` is set, and another program may replace the
/// selection a moment later. What this does promise is that every widget in
/// one app pastes the same text, which is the thing no widget could arrange
/// for itself while [`TerminalRequest`] stays one-way.
///
/// Only [`ClipboardTarget::Clipboard`] lands here.
/// [`Primary`](ClipboardTarget::Primary) is the X11 middle-click selection,
/// which reaches an app as a [`PasteMessage`](crate::PasteMessage) of its
/// own; echoing it would let a drag-select quietly displace what an explicit
/// copy put here.
///
/// `None` until something is copied - distinct from `Some(String::new())`,
/// which says an empty string was. Apps may write it to seed what a paste
/// key inserts.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct LastCopied(pub Option<String>);

/// Phases for the outbound half of the terminal contract.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum RequestSystems {
    /// `Last`: fills [`LastCopied`] from the requests written this frame.
    ///
    /// It runs here, rather than beside the inbound systems in `PreUpdate`,
    /// because a backend consumes the requests by draining them during
    /// extraction, after every main-world schedule. Reading earlier would
    /// miss everything written later in the same frame, and would miss it
    /// only once a real backend was installed. An app writing a request in
    /// `Last` orders itself `before` this.
    Echo,
}

pub(crate) fn echo_clipboard_writes(
    mut requests: MessageReader<TerminalRequest>,
    mut copied: ResMut<LastCopied>,
) {
    let last = requests.read().filter_map(|request| match request {
        TerminalRequest::CopyToClipboard {
            content,
            destination: ClipboardTarget::Clipboard,
        } => Some(content),
        _ => None,
    });
    // Assigned rather than compared: the change flag is the signal a widget
    // resyncs on, so copying the same text twice has to tick it twice.
    if let Some(content) = last.last() {
        copied.0 = Some(content.clone());
    }
}

/// Which clipboard selection a copy writes to.
///
/// Carried per request rather than configured once, so an editor can put a
/// dragged selection on [`Primary`](Self::Primary) and an explicit copy on
/// [`Clipboard`](Self::Clipboard) - the X11 idiom, which one shared setting
/// could not express.
///
/// Deliberately closed to the two selections terminals actually implement.
/// OSC 52 also names cut buffers `0`-`7`, which are xterm trivia nothing
/// else honors; a variant for them would be a breaking change worth making
/// only if something asks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClipboardTarget {
    /// The clipboard a paste reads from.
    #[default]
    Clipboard,
    /// The X11 and Wayland primary selection, pasted with the middle
    /// mouse button. Meaningless elsewhere, and ignored where it is.
    Primary,
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::change_detection::DetectChanges;
    use bevy_ecs::prelude::{IntoScheduleConfigs, Res, ResMut, Resource};

    use super::{ClipboardTarget, LastCopied, TerminalRequest};
    use crate::TermPlugin;

    #[derive(Resource, Default)]
    struct Echoes(usize);

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((plurimus_core::CorePlugin, TermPlugin));
        app.init_resource::<Echoes>();
        app.add_systems(
            bevy_app::Last,
            (|copied: Res<LastCopied>, mut echoes: ResMut<Echoes>| {
                if copied.is_changed() {
                    echoes.0 += 1;
                }
            })
            .after(super::echo_clipboard_writes),
        );
        app
    }

    fn copied(app: &App) -> Option<&str> {
        app.world().resource::<LastCopied>().0.as_deref()
    }

    #[test]
    fn a_copy_fills_the_echo() {
        let mut app = app();
        app.world_mut()
            .write_message(TerminalRequest::copy("taken"));
        app.update();

        assert_eq!(copied(&app), Some("taken"));
    }

    #[test]
    fn nothing_is_copied_until_something_is() {
        let mut app = app();
        app.update();

        assert_eq!(copied(&app), None, "None, not an empty string");
    }

    #[test]
    fn a_primary_copy_is_not_echoed() {
        let mut app = app();
        app.world_mut()
            .write_message(TerminalRequest::CopyToClipboard {
                content: "dragged".to_owned(),
                destination: ClipboardTarget::Primary,
            });
        app.update();

        assert_eq!(
            copied(&app),
            None,
            "a drag-select must not displace an explicit copy"
        );
    }

    #[test]
    fn a_title_is_not_echoed() {
        let mut app = app();
        app.world_mut()
            .write_message(TerminalRequest::SetTitle("name".to_owned()));
        app.update();

        assert_eq!(copied(&app), None);
    }

    #[test]
    fn the_last_copy_of_a_frame_wins() {
        let mut app = app();
        app.world_mut()
            .write_message(TerminalRequest::copy("first"));
        app.world_mut()
            .write_message(TerminalRequest::copy("second"));
        app.update();

        assert_eq!(copied(&app), Some("second"));
    }

    // The change flag is what a widget resyncs its own buffer on, so an
    // identical second copy has to tick it again - otherwise an editor that
    // overwrote its buffer in between never hears about it.
    #[test]
    fn copying_the_same_text_twice_reports_twice() {
        let mut app = app();
        app.world_mut().write_message(TerminalRequest::copy("same"));
        app.update();
        let first = app.world().resource::<Echoes>().0;

        app.world_mut().write_message(TerminalRequest::copy("same"));
        app.update();

        assert_eq!(app.world().resource::<Echoes>().0, first + 1);
    }

    #[test]
    fn a_quiet_frame_does_not_report() {
        let mut app = app();
        app.world_mut().write_message(TerminalRequest::copy("once"));
        app.update();
        let after_copy = app.world().resource::<Echoes>().0;

        app.update();

        assert_eq!(app.world().resource::<Echoes>().0, after_copy);
    }
}
