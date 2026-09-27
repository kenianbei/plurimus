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

#[allow(dead_code, reason = "wired up by the browser runtime")]
mod fit;
#[allow(dead_code, reason = "wired up by the browser runtime")]
mod keys;
#[allow(dead_code, reason = "wired up by the browser runtime")]
mod pointer;

pub use fit::GridFit;
