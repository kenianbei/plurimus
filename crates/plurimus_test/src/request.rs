//! Readers for the outbound half of the terminal contract.

use bevy_app::App;
use bevy_ecs::prelude::MessageReader;
use plurimus_term::TerminalRequest;

/// The clipboard contents an app has asked for since the previous call,
/// oldest first.
///
/// Reads through a cursor of its own, so a backend or any other reader of
/// the stream still sees every request. Copies only: any other request is
/// passed over.
///
/// # Panics
///
/// If the app never registered `TerminalRequest`, which `TermPlugin` does.
pub fn clipboard_writes(app: &mut App) -> Vec<String> {
    app.world_mut()
        .run_system_cached(|mut requests: MessageReader<TerminalRequest>| {
            requests
                .read()
                .filter_map(|request| match request {
                    TerminalRequest::CopyToClipboard { content, .. } => Some(content.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .expect("TerminalRequest is registered, as TermPlugin does")
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::message::{MessageCursor, Messages};
    use plurimus_term::TerminalRequest;

    use super::clipboard_writes;

    #[test]
    fn a_copy_is_returned_once_and_left_for_other_readers() {
        let mut app = App::new();
        app.add_message::<TerminalRequest>();
        app.world_mut()
            .write_message(TerminalRequest::copy("taken"));

        assert_eq!(clipboard_writes(&mut app), ["taken"]);
        assert!(
            clipboard_writes(&mut app).is_empty(),
            "a second call returns only newer copies"
        );
        let requests = app.world().resource::<Messages<TerminalRequest>>();
        let mut cursor = MessageCursor::default();
        assert_eq!(
            cursor.read(requests).count(),
            1,
            "another reader still sees the copy"
        );
    }
}
