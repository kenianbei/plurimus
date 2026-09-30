//! Terminal lifecycle: raw mode, alternate screen, input modes, and
//! restoration.
//!
//! Every mode entered here has to be undone, including when the process dies
//! badly, so restoration is idempotent and installed as a panic hook rather
//! than left to a `Drop`. Color depth is detected from the environment at the
//! same time; what the keyboard reports is learned later, from what arrives.

use std::fmt;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use bevy_ecs::prelude::Resource;
use crossterm::cursor::{Hide, SetCursorStyle, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{Command, execute, queue};
use plurimus_core::{ColorDepth, TerminalSize};
use ratatui_crossterm::CrosstermBackend;

// Restore runs from panic hooks with no app state, so what it undoes only
// if it was done is recorded here. Restore takes each rather than reading
// it: a panic restores from the hook and again as the world drops, and a
// second pop would take an entry pushed by whatever ran before this process.
static KITTY_PUSHED: AtomicBool = AtomicBool::new(false);
static SHAPE_WRITTEN: AtomicBool = AtomicBool::new(false);
static TITLE_PUSHED: AtomicBool = AtomicBool::new(false);

/// XTWINOPS 22;0: saves the icon name and the window title, the pair
/// crossterm's `SetTitle` replaces with OSC 0. The escape that reads a title
/// back is widely disabled as a data-exfiltration risk, so the terminal's own
/// stack is the only reliable way to hand it back.
struct PushTitle;

impl Command for PushTitle {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[22;0t")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Ok(())
    }
}

/// XTWINOPS 23;0: restores what [`PushTitle`] saved.
struct PopTitle;

impl Command for PopTitle {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[23;0t")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Ok(())
    }
}

/// Restores the terminal when the render world is torn down.
#[derive(Resource)]
pub(crate) struct RestoreOnDrop;

impl Drop for RestoreOnDrop {
    fn drop(&mut self) {
        restore();
    }
}

pub(crate) fn mark_shape_written() {
    SHAPE_WRITTEN.store(true, Ordering::Relaxed);
}

pub(crate) fn init<W: Write + Send + Sync + 'static>(
    mut writer: W,
    mouse: bool,
    paste: bool,
) -> io::Result<(CrosstermBackend<W>, TerminalSize, ColorDepth)> {
    terminal::enable_raw_mode()?;
    let kitty = terminal::supports_keyboard_enhancement().unwrap_or(false);
    queue!(writer, EnterAlternateScreen, Hide, PushTitle)?;
    TITLE_PUSHED.store(true, Ordering::Relaxed);
    queue_input_modes(&mut writer, kitty, mouse, paste)?;
    writer.flush()?;
    let (cols, rows) = terminal::size()?;
    Ok((
        CrosstermBackend::new(writer),
        TerminalSize::new(cols, rows),
        detect_color_depth(),
    ))
}

fn detect_color_depth() -> ColorDepth {
    color_depth_from_env(
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}

fn color_depth_from_env(colorterm: Option<&str>, term: Option<&str>) -> ColorDepth {
    match (colorterm, term) {
        (Some("truecolor" | "24bit"), _) => ColorDepth::TrueColor,
        (_, Some(term)) if term.contains("256color") => ColorDepth::Ansi256,
        _ => ColorDepth::Ansi16,
    }
}

fn queue_input_modes<W: Write>(
    writer: &mut W,
    kitty: bool,
    mouse: bool,
    paste: bool,
) -> io::Result<()> {
    if kitty {
        queue!(
            writer,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;
        KITTY_PUSHED.store(true, Ordering::Relaxed);
    }
    if mouse {
        queue!(writer, EnableMouseCapture)?;
    }
    if paste {
        queue!(writer, EnableBracketedPaste)?;
    }
    // Unconditional: mode 1004 reports nothing about the user and costs
    // nothing, and `FocusMessage` is public API that never fires without it.
    queue!(writer, EnableFocusChange)?;
    Ok(())
}

#[cfg(unix)]
pub(crate) fn open_tty() -> io::Result<std::fs::File> {
    std::fs::File::options().write(true).open("/dev/tty")
}

/// Chains terminal restoration in front of the current panic hook.
///
/// [`CrosstermPlugin`](crate::CrosstermPlugin) installs this automatically; call it
/// directly only when building a custom terminal lifecycle.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore();
        previous(panic_info);
    }));
}

/// Restores the terminal taken over by [`CrosstermPlugin`](crate::CrosstermPlugin).
///
/// Idempotent and best-effort - safe to call from `atexit` hooks or other
/// abnormal-exit paths that bypass Rust's drop order (a C library calling
/// `exit()`, `std::process::exit`). Targets the controlling terminal when
/// available (falling back to stdout), which is also where crossterm
/// applies raw mode - restoration works even when stdout is redirected.
pub fn restore() {
    let mut writer = restore_writer();
    if KITTY_PUSHED.swap(false, Ordering::Relaxed) {
        let _ = queue!(writer, PopKeyboardEnhancementFlags);
    }
    if SHAPE_WRITTEN.swap(false, Ordering::Relaxed) {
        let _ = queue!(writer, SetCursorStyle::DefaultUserShape);
    }
    if TITLE_PUSHED.swap(false, Ordering::Relaxed) {
        let _ = queue!(writer, PopTitle);
    }
    let _ = execute!(
        writer,
        DisableMouseCapture,
        DisableBracketedPaste,
        DisableFocusChange,
        LeaveAlternateScreen,
        Show
    );
    let _ = terminal::disable_raw_mode();
}

fn restore_writer() -> Box<dyn Write> {
    #[cfg(unix)]
    if let Ok(tty) = open_tty() {
        return Box::new(tty);
    }
    Box::new(io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorterm_truecolor_wins_over_term() {
        assert_eq!(
            color_depth_from_env(Some("truecolor"), Some("xterm-256color")),
            ColorDepth::TrueColor
        );
        assert_eq!(
            color_depth_from_env(Some("24bit"), None),
            ColorDepth::TrueColor
        );
    }

    #[test]
    fn term_with_256color_maps_to_ansi256() {
        assert_eq!(
            color_depth_from_env(None, Some("xterm-256color")),
            ColorDepth::Ansi256
        );
        assert_eq!(
            color_depth_from_env(Some("yes"), Some("screen-256color")),
            ColorDepth::Ansi256
        );
    }

    #[test]
    fn plain_term_maps_to_ansi16() {
        assert_eq!(
            color_depth_from_env(None, Some("xterm")),
            ColorDepth::Ansi16
        );
        assert_eq!(
            color_depth_from_env(None, Some("linux")),
            ColorDepth::Ansi16
        );
    }

    #[test]
    fn missing_or_dumb_term_floors_at_ansi16() {
        assert_eq!(color_depth_from_env(None, None), ColorDepth::Ansi16);
        assert_eq!(color_depth_from_env(None, Some("dumb")), ColorDepth::Ansi16);
    }

    fn ansi(command: &impl Command) -> String {
        let mut written = String::new();
        command
            .write_ansi(&mut written)
            .expect("write into a String");
        written
    }

    #[test]
    fn the_title_stack_saves_and_restores_icon_and_window_together() {
        assert_eq!(ansi(&PushTitle), "\x1b[22;0t");
        assert_eq!(ansi(&PopTitle), "\x1b[23;0t");
    }

    fn queued(kitty: bool, mouse: bool, paste: bool) -> String {
        let mut writer = Vec::new();
        queue_input_modes(&mut writer, kitty, mouse, paste).expect("queue into a Vec");
        String::from_utf8(writer).expect("escape sequences are utf-8")
    }

    // Mode 1004 is the only thing that makes a terminal report focus, and
    // nothing else emits it: mouse capture sends 1000/1002/1003/1015/1006.
    #[test]
    fn focus_reporting_is_enabled_whatever_else_is_asked_for() {
        assert!(
            queued(false, false, false).contains("\x1b[?1004h"),
            "PasteMessage's sibling FocusMessage never fires without it"
        );
        assert!(queued(true, true, true).contains("\x1b[?1004h"));
    }

    #[test]
    fn the_optional_modes_stay_optional() {
        let none = queued(false, false, false);
        assert!(!none.contains("?1000h"), "mouse capture is off: {none:?}");
        assert!(!none.contains("?2004h"), "bracketed paste is off: {none:?}");

        let all = queued(false, true, true);
        assert!(all.contains("?1000h") && all.contains("?2004h"));
    }
}
