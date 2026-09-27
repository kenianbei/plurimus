//! Browser integration for plurimus: the counterpart to `plurimus_crossterm`
//! for a page instead of a terminal.
//!
//! `WebPlugin` draws the app on a WebGL2 canvas, turns the browser's keys,
//! pointer, wheel, paste and focus into `plurimus_term`'s messages, and runs
//! the app once per animation frame. Add it after `CorePlugin` where a
//! terminal app adds `CrosstermPlugin`:
//!
//! ```ignore
//! #[wasm_bindgen(start)]
//! pub fn start() {
//!     App::new()
//!         .add_plugins((
//!             CorePlugin,
//!             WebPlugin::new().font("Inconsolata").fit(GridFit::Cells(128, 32)),
//!             WidgetsPlugin,
//!         ))
//!         .run();
//! }
//! ```
//!
//! The page gives the canvas's element a size - `html, body { height: 100% }`
//! when it is the body - and supplies the font. When the app exits the canvas
//! receives `EXIT_EVENT`, its `detail` the exit code, and the page decides
//! what leaving means:
//!
//! ```js
//! document.querySelector("canvas")
//!   .addEventListener("plurimus-exit", () => location.reload());
//! ```
//!
//! The browser half compiles only for `wasm32-unknown-unknown`, and refuses
//! to with wasm threads; on other targets the crate holds just [`GridFit`].

#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
compile_error!(
    "plurimus_web runs single-threaded: its backend is shared with the presenter on the \
     promise that no second thread exists, which wasm atomics would break"
);

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only the wasm32 browser runtime calls into it")
)]
mod fit;
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only the wasm32 browser runtime calls into it")
)]
mod keys;
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "only the wasm32 browser runtime calls into it")
)]
mod pointer;

#[cfg(target_arch = "wasm32")]
pub use browser::{EXIT_EVENT, WebPlugin};
pub use fit::GridFit;
