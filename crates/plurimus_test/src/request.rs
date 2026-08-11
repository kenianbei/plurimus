//! Readers for the outbound half of the terminal contract.

use bevy_app::App;
use bevy_ecs::message::Messages;
use plurimus_term::TerminalRequest;

/// Takes the clipboard contents an app has asked for, oldest first.
///
/// Destructive, as a backend's own consumption is: what it returns, nothing
/// else will see. Call it after the frame that wrote the requests - and
/// after that frame has run, since `LastCopied` is filled in `Last`.
///
/// Copies only. Any other request in the same stream is taken and
/// discarded, so a test asserting on copies is not made to skip past them.
pub fn clipboard_writes(app: &mut App) -> Vec<String> {
    app.world_mut()
        .resource_mut::<Messages<TerminalRequest>>()
        .drain()
        .filter_map(|request| match request {
            TerminalRequest::CopyToClipboard { content, .. } => Some(content),
            _ => None,
        })
        .collect()
}
