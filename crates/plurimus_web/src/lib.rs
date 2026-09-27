//! Browser integration for plurimus: the counterpart to `plurimus_crossterm`
//! for a page instead of a terminal.
//!
//! The browser-facing half compiles only for `wasm32-unknown-unknown`; on
//! other targets this crate holds just its portable vocabulary.

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
