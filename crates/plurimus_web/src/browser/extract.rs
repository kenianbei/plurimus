//! Render-world systems that reach back into the main world or the page.

use bevy_ecs::message::{MessageCursor, Messages};
use bevy_ecs::prelude::{Local, Res, ResMut};
use plurimus_core::ratatui_core::backend::Backend;
use plurimus_core::{MainWorld, TerminalContext, TerminalSize};
use plurimus_term::{ClipboardTarget, TerminalRequest, TerminalResized};
use wasm_bindgen_futures::JsFuture;

use super::backend::WebBackend;
use super::{document, warn, window};

/// Follows the canvas's size into the grid and the grid into
/// `TerminalSize`.
///
/// The renderer only notices a new canvas size when it flushes, and the
/// presenter skips flushing a frame where no cell changed, so the resize is
/// applied here, every frame, instead.
pub(crate) fn sync_size(
    mut main_world: ResMut<MainWorld>,
    mut context: ResMut<TerminalContext<WebBackend>>,
    mut last_css: Local<(i32, i32)>,
) {
    let backend = &mut context.backend;
    let css = (
        backend.canvas.client_width(),
        backend.canvas.client_height(),
    );
    if css != *last_css {
        *last_css = css;
        if let Err(error) = backend.inner.resize_canvas() {
            warn(&format!(
                "plurimus_web: resizing the canvas failed: {error}"
            ));
        }
    }
    let Ok(size) = backend.size() else {
        return;
    };
    let current = *main_world.resource::<TerminalSize>();
    if (current.cols, current.rows) != (size.width, size.height) {
        main_world.write_message(TerminalResized::new(size.width, size.height));
    }
}

/// Serves what an app asks of the page. Reads through a cursor rather than
/// draining, so any other reader of the stream still sees every request.
pub(crate) fn serve_requests(
    main_world: Res<MainWorld>,
    mut cursor: Local<MessageCursor<TerminalRequest>>,
) {
    let requests = main_world.resource::<Messages<TerminalRequest>>();
    for request in cursor.read(requests) {
        match request {
            TerminalRequest::CopyToClipboard {
                content,
                destination: ClipboardTarget::Clipboard,
            } => copy(content),
            TerminalRequest::SetTitle(title) => document().set_title(title),
            _ => {}
        }
    }
}

/// Browsers reject a clipboard write outside a secure context or without a
/// recent user gesture; that is reported, not fatal.
fn copy(content: &str) {
    let written = window().navigator().clipboard().write_text(content);
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = JsFuture::from(written).await {
            warn(&format!(
                "plurimus_web: the clipboard refused a copy: {error:?}"
            ));
        }
    });
}
