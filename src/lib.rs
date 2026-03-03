#![doc = include_str!("../README.md")]

mod draw;
mod plugin;

#[cfg(feature = "tachyonfx")]
mod effects;

#[cfg(feature = "ui")]
mod ui;

#[cfg(feature = "widget")]
mod widget;

pub use crate::draw::{DrawArea, DrawFn, DrawOrder};
pub use crate::plugin::{PlurimusFixedSet, PlurimusPlugin};

#[cfg(feature = "tachyonfx")]
pub use crate::effects::{TachyonEffect, TachyonRegistry, add_fx, enable_fx};

#[cfg(feature = "ui")]
pub use crate::ui::{
    actions::{
        KeyBinding, MouseBinding, UiActionDisabled, UiActions, UiActionsAppExt, UiEvent,
        UiInputBinding,
    },
    builder::UiBuilder,
    focus::UiFocusMessage,
    state::{UiDisabled, UiFocusable, UiFocused, UiHoverable, UiHovered, UiPressable, UiPressed},
};

#[cfg(feature = "widget")]
pub use crate::widget::{
    area::{LayoutFn, WidgetLayout, WidgetRect},
    order::WidgetOrder,
    widget::Widget,
};
