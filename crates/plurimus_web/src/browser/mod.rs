//! The browser half: presenting, measuring, running, and serving requests.

mod backend;
mod extract;
mod input;
mod measure;
mod plugin;
mod runner;

pub use plugin::WebPlugin;
pub use runner::EXIT_EVENT;

fn window() -> web_sys::Window {
    web_sys::window().expect("plurimus_web: runs only in a browser window")
}

fn document() -> web_sys::Document {
    window()
        .document()
        .expect("plurimus_web: the browser window has no document")
}

fn warn(message: &str) {
    web_sys::console::warn_1(&message.into());
}
