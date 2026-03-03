use crate::ui::actions::{UiActions, UiInputBinding};
use crate::ui::state::{UiFocusable, UiFocused, UiHoverable, UiPressable};
use crate::widget::area::{LayoutFn, WidgetLayout};
use crate::widget::order::WidgetOrder;
use crate::widget::widget::Widget;
use bevy::prelude::{Commands, Component, EntityCommands};
use std::sync::Arc;

/// Builder for creating UI entities with a `Widget` and optional components for layout, focus, and pointer interaction.
/// The `UiBuilder` allows you to easily create UI entities with a `Widget` and optional components for layout, focus, and pointer interaction.
/// It also allows you to add custom hooks that can insert additional components or perform other setup on the entity before it is spawned.
/// Once the `UiBuilder` is configured, you can call `spawn` to create the entity in the world.
///
/// Example usage:
/// ```rust
/// use std::sync::Arc;
/// use bevy::prelude::*;
/// use bevy_ratatui::crossterm::event::MouseButton;
/// use plurimus::{LayoutFn, MouseBinding, UiBuilder, UiInputBinding, Widget};
/// use ratatui::widgets::Paragraph;
///
/// #[derive(Component, Clone)]
/// struct MyMarker;
///
/// fn startup(mut commands: Commands) {
///     UiBuilder::new(Widget::from_widget(Paragraph::new("Hello, world!")))
///         .with_actions([
///             UiInputBinding::mouse_binding(MouseBinding::down(MouseButton::Left), |_, _, _| {
///                 println!("Paragraph clicked!");
///                 Ok(())
///             }),
///         ])
///         .with_marker(MyMarker)
///         .with_layout((Arc::new(|area|*area), 0))
///         .interactive(0)
///         .focused()
///         .spawn(&mut commands);
/// }
/// ```
pub struct UiBuilder {
    actions: UiActions,
    hooks: Vec<Arc<dyn Fn(&mut EntityCommands) + Send + Sync + 'static>>,
}

impl UiBuilder {
    pub fn new(widget: Widget) -> Self {
        Self {
            actions: UiActions::default(),
            hooks: vec![Arc::new(move |ec: &mut EntityCommands| {
                ec.insert(widget.clone());
            })],
        }
    }

    pub fn with_hook(mut self, hook: impl Fn(&mut EntityCommands) + Send + Sync + 'static) -> Self {
        self.hooks.push(Arc::new(hook));
        self
    }

    pub fn spawn<'a>(&self, commands: &'a mut Commands) -> EntityCommands<'a> {
        let mut ec = commands.spawn_empty();

        for hook in &self.hooks {
            hook(&mut ec);
        }

        ec.insert(self.actions.clone());

        ec
    }

    pub fn with_actions(mut self, bindings: impl IntoIterator<Item = UiInputBinding>) -> Self {
        self.actions.inner.extend(bindings);
        self
    }

    pub fn with_layout(self, layout: (LayoutFn, i32)) -> Self {
        self.with_hook(move |ec| {
            ec.insert(WidgetLayout(layout.0.clone()));
            ec.insert(WidgetOrder(layout.1));
        })
    }

    pub fn with_marker<M: Component + Clone>(self, marker: M) -> Self {
        self.with_hook(move |ec| {
            ec.insert(marker.clone());
        })
    }

    pub fn focusable(self, index: i32) -> Self {
        self.with_hook(move |ec| {
            ec.insert(UiFocusable::new(index));
        })
    }

    pub fn hoverable(self) -> Self {
        self.with_hook(move |ec| {
            ec.insert(UiHoverable);
        })
    }

    pub fn pressable(self) -> Self {
        self.with_hook(move |ec| {
            ec.insert(UiPressable);
        })
    }

    pub fn interactive(self, index: i32) -> Self {
        self.focusable(index).hoverable().pressable()
    }

    pub fn focused(self) -> Self {
        self.with_hook(move |ec| {
            ec.insert(UiFocused);
        })
    }
}
