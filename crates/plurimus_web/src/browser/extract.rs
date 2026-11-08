//! Render-world systems that reach back into the main world or the page.

use bevy_clipboard::Clipboard;
use bevy_ecs::message::{MessageCursor, Messages};
use bevy_ecs::prelude::{Local, ResMut};
use plurimus_core::ratatui_core::backend::Backend;
use plurimus_core::{MainWorld, TerminalContext, TerminalSize};
use plurimus_term::{ClipboardTarget, TerminalRequest, TerminalResized};

use super::backend::WebBackend;
use super::{document, warn};

/// Follows the canvas's size into the grid and the grid into
/// `TerminalSize`, during extraction so the app sees a resize before it
/// composes the next frame - the browser's counterpart to a terminal's
/// resize event.
///
/// The renderer compares the canvas against the size it last laid out only
/// when it flushes, which the presenter skips on a frame where no cell
/// changed; the same comparison is made here, against that same stored size.
pub(crate) fn sync_size(
    mut main_world: ResMut<MainWorld>,
    mut context: ResMut<TerminalContext<WebBackend>>,
) {
    let backend = &mut context.backend;
    let displayed = (
        backend.canvas.client_width(),
        backend.canvas.client_height(),
    );
    let laid_out = backend
        .window_size()
        .map(|size| (i32::from(size.pixels.width), i32::from(size.pixels.height)));
    if laid_out.is_ok_and(|laid_out| laid_out != displayed)
        && let Err(error) = backend.inner.resize_canvas()
    {
        warn(&format!(
            "plurimus_web: resizing the canvas failed: {error}"
        ));
    }
    let Ok(size) = backend.size() else {
        return;
    };
    let current = *main_world.resource::<TerminalSize>();
    if (current.cols, current.rows) != (size.width, size.height) {
        main_world.write_message(TerminalResized::new(size.width, size.height));
    }
}

/// Serves what an app asks of the page. Reads through a cursor of its own,
/// so any other reader of the stream still sees every request.
///
/// A copy goes through `bevy_clipboard`, which reports a browser refusing
/// the write - outside a secure context, without a recent user gesture - and
/// treats it as not fatal.
pub(crate) fn serve_requests(
    mut main_world: ResMut<MainWorld>,
    mut cursor: Local<MessageCursor<TerminalRequest>>,
) {
    main_world.resource_scope::<Messages<TerminalRequest>, _>(|world, requests| {
        for request in cursor.read(&requests) {
            match request {
                TerminalRequest::CopyToClipboard {
                    content,
                    destination: ClipboardTarget::Clipboard,
                } => {
                    if let Err(error) = world.resource_mut::<Clipboard>().set_text(content.as_str())
                    {
                        warn(&format!(
                            "plurimus_web: the clipboard refused a copy: {error}"
                        ));
                    }
                }
                TerminalRequest::SetTitle(title) => document().set_title(title),
                _ => {}
            }
        }
    });
}
