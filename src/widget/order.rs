use crate::draw::DrawOrder;
use bevy::prelude::Component;

/// Component that holds an `i32` representing the order a widget is drawn in. It's a required component for a `Widget`.
/// Widgets with a lower order value are drawn before widgets with a higher order value. The default value is `0`.
///
/// Example usage:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::{Widget, WidgetOrder};
/// use ratatui::widgets::Paragraph;
/// fn startup(mut commands: Commands) {
///     commands.spawn((
///         Widget::from_widget(Paragraph::new("Hello, world!")),
///         WidgetOrder(1),
///     ));
///     commands.spawn((
///         Widget::from_widget(Paragraph::new("Goodbye, world!")),
///         WidgetOrder(0),
///     ));
/// }
/// ```
#[derive(Clone, Component, Copy, Default)]
pub struct WidgetOrder(pub i32);

impl DrawOrder for WidgetOrder {
    fn order(&self) -> i32 {
        self.0
    }
}
