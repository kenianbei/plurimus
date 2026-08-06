#![doc = include_str!("../README.md")]

pub use plurimus_core as core;
#[cfg(feature = "crossterm")]
pub use plurimus_crossterm as crossterm;
#[cfg(feature = "input")]
pub use plurimus_input as input;

#[cfg(feature = "bevy-ui")]
pub use plurimus_bui as bui;
#[cfg(feature = "ui")]
pub use plurimus_ui as ui;
#[cfg(feature = "widgets")]
pub use plurimus_widgets as widgets;

#[cfg(feature = "2d")]
pub use plurimus_2d as render2d;

#[cfg(feature = "3d")]
pub use plurimus_3d as render3d;
