//! ratzilla's WebGL2 backend, made presentable.

use std::io;

use plurimus_core::ratatui_core::backend::{Backend, ClearType, WindowSize};
use plurimus_core::ratatui_core::buffer::Cell;
use plurimus_core::ratatui_core::layout::{Position, Size};
use ratzilla::WebGl2Backend;
use web_sys::HtmlCanvasElement;

/// The backend the presenter draws through, with the canvas it draws on.
pub(crate) struct WebBackend {
    pub(crate) inner: WebGl2Backend,
    pub(crate) canvas: HtmlCanvasElement,
}

// SAFETY: `PresenterPlugin` asks for `Send + Sync` so that a backend may be
// used from another thread. This crate refuses to compile with wasm atomics
// (`lib.rs`), and without them `wasm32-unknown-unknown` runs exactly one
// thread, so this value is never moved to or shared with another.
unsafe impl Send for WebBackend {}
// SAFETY: as for `Send` above.
unsafe impl Sync for WebBackend {}

impl Backend for WebBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
