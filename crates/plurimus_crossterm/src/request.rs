//! Serving [`TerminalRequest`]s through the terminal writer.
//!
//! Requests are drained from the main world during extraction and written in
//! the present phase, which is the only place the backend - and so the
//! writer - is reachable. The write flushes itself rather than leaning on
//! the presenter's: the presenter skips both draw and flush when no cell
//! differs, and a copy usually changes nothing on screen, so a shared flush
//! would strand the escape until some later redraw.
//!
//! Ordering against the presenter is immaterial, which is why none is
//! stated. Both systems take the backend mutably, so they never overlap, and
//! each writes a self-contained sequence and flushes it - the presenter's
//! draw and flush are one system body, so nothing can land between them.

use std::io::{self, Write};

use bevy_ecs::error::Result as BevyResult;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Res, ResMut, Resource};
use bevy_log::warn;
use crossterm::clipboard::{ClipboardSelection, ClipboardType, CopyToClipboard};
use crossterm::cursor::SetCursorStyle;
use crossterm::queue;
use crossterm::terminal::SetTitle;
use plurimus_core::{MainWorld, TerminalContext, TerminalCursor, TerminalCursorStyle};
use plurimus_input::{ClipboardTarget, TerminalRequest};
use ratatui_crossterm::CrosstermBackend;

/// Longest base64 payload tmux forwards; past it the sequence is truncated
/// rather than refused, so plurimus refuses instead - a clipboard holding
/// most of what was copied is worse than one holding none of it.
const OSC52_MAX_BASE64: usize = 74_994;

/// Requests drained from the main world, awaiting the writer.
#[derive(Resource, Debug, Default)]
pub(crate) struct PendingRequests(Vec<TerminalRequest>);

/// Whether [`CrosstermPlugin::clipboard`](crate::CrosstermPlugin::clipboard)
/// allowed copying.
#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct ClipboardEnabled(pub(crate) bool);

pub(crate) fn extract_requests(
    mut main_world: ResMut<MainWorld>,
    mut pending: ResMut<PendingRequests>,
) {
    let drained: Vec<TerminalRequest> = main_world
        .resource_mut::<Messages<TerminalRequest>>()
        .drain()
        .collect();
    pending.0.extend(drained);
}

pub(crate) fn write_requests<W: Write + Send + Sync + 'static>(
    mut context: ResMut<TerminalContext<CrosstermBackend<W>>>,
    mut pending: ResMut<PendingRequests>,
    clipboard: Res<ClipboardEnabled>,
) -> BevyResult {
    if pending.0.is_empty() {
        return Ok(());
    }
    let requests = std::mem::take(&mut pending.0);
    // The backend is itself a `Write` forwarding to the terminal writer,
    // which is the stable way in; `writer_mut` is behind ratatui's unstable
    // `backend-writer` feature.
    serve(&mut context.backend, &requests, clipboard.0)?;
    Ok(())
}

/// The cursor shape last asked of the terminal, so an unchanged one costs
/// no escape sequence.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub(crate) struct PreviousCursorStyle(Option<TerminalCursorStyle>);

/// Sets the cursor shape, which no `Backend` method reaches.
///
/// Position and visibility are the presenter's; only the shape needs a
/// crossterm-specific sequence, and a terminal is free to ignore it.
pub(crate) fn write_cursor_style<W: Write + Send + Sync + 'static>(
    mut context: ResMut<TerminalContext<CrosstermBackend<W>>>,
    cursor: Res<TerminalCursor>,
    mut previous: ResMut<PreviousCursorStyle>,
) -> BevyResult {
    if previous.0 == Some(cursor.style) {
        return Ok(());
    }
    previous.0 = Some(cursor.style);
    let Some(style) = cursor_style(cursor.style) else {
        return Ok(());
    };
    let writer = &mut context.backend;
    queue!(writer, style)?;
    writer.flush()?;
    Ok(())
}

/// `None` for the terminal's own shape, which is asked for by not asking.
const fn cursor_style(style: TerminalCursorStyle) -> Option<SetCursorStyle> {
    match style {
        TerminalCursorStyle::Default => None,
        TerminalCursorStyle::BlinkingBlock => Some(SetCursorStyle::BlinkingBlock),
        TerminalCursorStyle::SteadyBlock => Some(SetCursorStyle::SteadyBlock),
        TerminalCursorStyle::BlinkingUnderline => Some(SetCursorStyle::BlinkingUnderScore),
        TerminalCursorStyle::SteadyUnderline => Some(SetCursorStyle::SteadyUnderScore),
        TerminalCursorStyle::BlinkingBar => Some(SetCursorStyle::BlinkingBar),
        TerminalCursorStyle::SteadyBar => Some(SetCursorStyle::SteadyBar),
    }
}

/// Queues every request and flushes once if any of them wrote.
fn serve(writer: &mut impl Write, requests: &[TerminalRequest], clipboard: bool) -> io::Result<()> {
    let mut wrote = false;
    for request in requests {
        wrote |= write_request(writer, request, clipboard)?;
    }
    if wrote {
        writer.flush()?;
    }
    Ok(())
}

/// Queues one request, reporting whether anything reached the writer.
///
/// A request the terminal cannot be asked for - copying with the capability
/// switched off, an oversized payload, a variant this backend does not
/// know - is dropped rather than failing the frame.
fn write_request(
    writer: &mut impl Write,
    request: &TerminalRequest,
    clipboard: bool,
) -> io::Result<bool> {
    match request {
        TerminalRequest::CopyToClipboard {
            content,
            destination,
        } => write_copy(writer, content, *destination, clipboard),
        TerminalRequest::SetTitle(title) => {
            queue!(writer, SetTitle(title))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn write_copy(
    writer: &mut impl Write,
    content: &str,
    destination: ClipboardTarget,
    enabled: bool,
) -> io::Result<bool> {
    if !enabled {
        return Ok(false);
    }
    let encoded = base64_len(content.len());
    if encoded > OSC52_MAX_BASE64 {
        warn!(
            "dropped a {encoded}-byte clipboard write; OSC 52 carries at most \
             {OSC52_MAX_BASE64} base64 bytes before it is truncated in transit"
        );
        return Ok(false);
    }
    queue!(
        writer,
        CopyToClipboard {
            content,
            destination: ClipboardSelection(vec![clipboard_type(destination)]),
        }
    )?;
    Ok(true)
}

const fn clipboard_type(target: ClipboardTarget) -> ClipboardType {
    match target {
        ClipboardTarget::Clipboard => ClipboardType::Clipboard,
        ClipboardTarget::Primary => ClipboardType::Primary,
    }
}

/// Base64 encodes three bytes into four, padded to a multiple of four, so
/// the length is known without paying for the encoding.
const fn base64_len(bytes: usize) -> usize {
    bytes.div_ceil(3) * 4
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bevy_app::App;
    use plurimus_core::{
        CorePlugin, PresenterPlugin, TerminalRenderApp, TerminalRenderAppExt,
        TerminalRenderSystems, TerminalSize,
    };

    use super::*;

    /// A buffered writer the test keeps a handle on, so bytes can be read
    /// back without ratatui's unstable `backend-writer` accessors.
    ///
    /// Buffered on purpose: the real tty writer is a `BufWriter`, so a
    /// sequence that is written but never flushed reaches nobody, and a
    /// write-through double would hide exactly that.
    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Buffered>>);

    #[derive(Default)]
    struct Buffered {
        pending: Vec<u8>,
        flushed: Vec<u8>,
    }

    impl SharedWriter {
        /// Only what actually left the buffer.
        fn written(&self) -> String {
            let buffered = self.0.lock().expect("writer lock");
            String::from_utf8(buffered.flushed.clone()).expect("escape sequences are utf-8")
        }
    }

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("writer lock")
                .pending
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            let mut buffered = self.0.lock().expect("writer lock");
            let pending = std::mem::take(&mut buffered.pending);
            buffered.flushed.extend(pending);
            Ok(())
        }
    }

    fn app_serving_requests(clipboard: bool) -> (App, SharedWriter) {
        let writer = SharedWriter::default();
        let mut app = App::new();
        app.add_plugins((CorePlugin, plurimus_input::InputPlugin));
        app.insert_resource(TerminalSize { cols: 10, rows: 2 });
        app.add_plugins(PresenterPlugin::new(CrosstermBackend::new(writer.clone())));
        app.add_extract_systems(extract_requests);
        app.add_terminal_systems(
            TerminalRenderSystems::Present,
            write_requests::<SharedWriter>,
        );
        app.sub_app_mut(TerminalRenderApp)
            .insert_resource(PendingRequests::default())
            .insert_resource(ClipboardEnabled(clipboard));
        (app, writer)
    }

    fn served(requests: &[TerminalRequest], clipboard: bool) -> String {
        let mut writer = Vec::new();
        serve(&mut writer, requests, clipboard).expect("serve into a Vec");
        String::from_utf8(writer).expect("escape sequences are utf-8")
    }

    #[test]
    fn a_copy_becomes_osc52_with_the_base64_of_its_content() {
        // "hi" base64-encodes to "aGk=".
        let written = served(&[TerminalRequest::copy("hi")], true);

        assert!(written.contains("\x1b]52;c;aGk="), "{written:?}");
    }

    #[test]
    fn the_destination_picks_the_selection_character() {
        let primary = TerminalRequest::CopyToClipboard {
            content: "hi".into(),
            destination: ClipboardTarget::Primary,
        };

        assert!(served(&[primary], true).contains("]52;p;"));
        assert!(served(&[TerminalRequest::copy("hi")], true).contains("]52;c;"));
    }

    // The flag is the only place a refusal can live, so it has to refuse.
    #[test]
    fn a_copy_writes_nothing_while_the_capability_is_off() {
        assert_eq!(served(&[TerminalRequest::copy("hi")], false), "");
    }

    // A truncated clipboard is worse than an empty one: the user pastes
    // something that looks whole.
    #[test]
    fn an_oversized_copy_is_dropped_rather_than_truncated() {
        let huge = "x".repeat(OSC52_MAX_BASE64);

        assert_eq!(served(&[TerminalRequest::copy(huge)], true), "");
    }

    #[test]
    fn a_title_needs_no_capability_flag() {
        assert!(
            served(&[TerminalRequest::SetTitle("plurimus".into())], false).contains("plurimus")
        );
    }

    #[test]
    fn requests_are_written_in_the_order_they_were_sent() {
        let written = served(
            &[
                TerminalRequest::SetTitle("first".into()),
                TerminalRequest::copy("hi"),
            ],
            true,
        );

        let title = written.find("first").expect("the title");
        let copy = written.find("]52;").expect("the copy");
        assert!(title < copy, "one channel keeps their order: {written:?}");
    }

    // Against a table rather than the same arithmetic, which would only
    // prove the expression equals itself.
    #[test]
    fn base64_length_matches_what_encoding_would_produce() {
        for (bytes, expected) in [(0, 0), (1, 4), (2, 4), (3, 4), (4, 8), (6, 8), (7, 12)] {
            assert_eq!(base64_len(bytes), expected, "{bytes} bytes");
        }
    }

    // The presenter skips draw and flush entirely when no cell differs, and
    // a copy changes nothing on screen - so a request that rode its flush
    // would sit in the writer until some later redraw, or forever.
    #[test]
    fn a_request_reaches_the_terminal_on_a_frame_that_draws_nothing() {
        let (mut app, writer) = app_serving_requests(true);
        app.update();
        let before = writer.written();

        app.world_mut().write_message(TerminalRequest::copy("hi"));
        app.update();

        let after = writer.written();
        assert!(
            after.len() > before.len() && after.contains("]52;c;aGk="),
            "nothing on screen moved, so only the request itself explains new bytes: {after:?}"
        );
    }

    #[test]
    fn a_request_is_served_once_and_not_replayed() {
        let (mut app, writer) = app_serving_requests(true);
        app.world_mut().write_message(TerminalRequest::copy("hi"));
        app.update();
        let after_first = writer.written();

        app.update();

        assert_eq!(writer.written(), after_first, "a second frame repeats it");
    }
}
