//! Readers for the outbound half of the terminal contract.

use bevy_app::App;
use bevy_ecs::message::{MessageCursor, Messages};
use bevy_ecs::prelude::{Mut, Resource};
use plurimus_term::TerminalRequest;

#[derive(Resource, Debug, Default)]
struct ClipboardCursor(MessageCursor<TerminalRequest>);

/// The clipboard contents an app has asked for since the previous call,
/// oldest first. Call it after the frame that wrote the requests has run.
///
/// Reads through a cursor of its own, so a backend or any other reader of
/// the stream still sees every request. Copies only: any other request is
/// passed over.
pub fn clipboard_writes(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world.init_resource::<ClipboardCursor>();
    world.resource_scope(|world, mut cursor: Mut<ClipboardCursor>| {
        cursor
            .0
            .read(world.resource::<Messages<TerminalRequest>>())
            .filter_map(|request| match request {
                TerminalRequest::CopyToClipboard { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect()
    })
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
        app.update();

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
