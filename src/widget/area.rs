use crate::draw::DrawArea;
use bevy::prelude::Component;
use std::sync::Arc;

/// Component that holds a `Rect` representing the area a widget is drawn to. It's required component for a `Widget`.
/// If a `WidgetLayout` is also present, the `WidgetRect` will ve recalculated using the layout function and the terminal size.
///
/// Example usage:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::{Widget, WidgetRect};
/// use ratatui::prelude::Rect;
/// use ratatui::widgets::Paragraph;
///
/// fn startup(mut commands: Commands) {
///     commands.spawn((
///         Widget::from_widget(Paragraph::new("Hello, world!")),
///         WidgetRect(Rect::new(0, 0, 10, 5)),
///    ));
/// }
/// ```
#[derive(Clone, Component, Copy, Default)]
pub struct WidgetRect(pub ratatui::prelude::Rect);

impl DrawArea for WidgetRect {
    fn area(&self) -> ratatui::prelude::Rect {
        self.0
    }
}

/// Optional component that holds a layout function for calculating the area a widget is drawn to.
/// If present, the `WidgetRect` will be recalculated using the layout function and the terminal size.
///
/// Example usage:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::{Widget, WidgetLayout};
/// use ratatui::prelude::Rect;
/// use ratatui::widgets::Paragraph;
///
/// fn startup(mut commands: Commands) {
///     commands.spawn((
///         Widget::from_widget(Paragraph::new("Hello, world!")),
///         WidgetLayout::new(|area: &Rect| {
///             use ratatui::layout::{Constraint, Direction, Layout};
///
///             Layout::default().constraints([
///                 Constraint::Percentage(50),
///                 Constraint::Percentage(50),
///             ])
///                 .direction(Direction::Horizontal)
///                 .split(*area)[0]
///        }),
///     ));
/// }
/// ```
#[derive(Clone, Component)]
#[require(WidgetRect)]
pub struct WidgetLayout(pub LayoutFn);

impl WidgetLayout {
    pub fn new<F>(layout_fn: F) -> Self
    where
        F: Fn(&ratatui::prelude::Rect) -> ratatui::prelude::Rect + Send + Sync + 'static,
    {
        Self(Arc::new(layout_fn))
    }
}

impl Default for WidgetLayout {
    fn default() -> Self {
        Self(Arc::new(|area: &ratatui::prelude::Rect| *area))
    }
}

impl From<LayoutFn> for WidgetLayout {
    fn from(value: LayoutFn) -> Self {
        Self(value)
    }
}

/// Type alias for a layout function that takes a reference to a `Rect` and returns a `Rect`.
/// The inputted `Rect` originates from the terminal size, and the outputed `Rect` is used to update the entity's `WidgetRect`.
///
/// Example:
/// ```rust
/// use plurimus::{Widget, WidgetRect};
/// use ratatui::prelude::Rect;
/// fn my_layout(area: &Rect) -> Rect {
///     use ratatui::layout::{Constraint, Direction, Layout};
///
///     Layout::default().constraints([
///         Constraint::Percentage(50),
///         Constraint::Percentage(50),
///     ])
///         .direction(Direction::Horizontal)
///         .split(*area)[0]
/// }
/// ```
pub type LayoutFn =
    Arc<dyn Fn(&ratatui::prelude::Rect) -> ratatui::prelude::Rect + Send + Sync + 'static>;
