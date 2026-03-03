# Plurimus

Provides a set of components and systems to make it easier to build terminal UIs with bevy_ratatui.

## Overview

Plurimus adds a layer of abstraction on top of bevy_ratatui, allowing you to define renderable ratatui widget using components with queryable
traits. This makes it easier to manage the state and behavior of your UI components using bevy's ECS, while still leveraging the power of
ratatui for rendering.

The core functionality of Plurimus is to provide queryable traits that enable bevy entities to reflect renderable ratatui widgets. The traits
can be implemented on any bevy component, allowing you to create custom widgets and manage their state using bevy's ECS.

It's important to note that once Plurimus is enabled as a plugin, RatatuiContext draw function will be exclusively managed by Plurimus.
Attempting to use RatatuiContext's draw function directly will result in flickering renders, as Plurimus will override the draw calls. To avoid
this, you should use the provided traits and components to define your widgets and their rendering behavior.

The main traits are:

- `DrawArea`: Implement on any component to define the area of the terminal frame where the widget should be drawn.
- `DrawOrder`: Implement on any component to define the order in which widgets are drawn, allowing layering of widgets.
- `DrawFn`: Implement on any component to define a custom draw function for the widget. This widget is required in order for rendering.

## Features

Plurimus provides default Widget components that implement the above traits, but you can also create your own custom widgets by implementing the
traits on your own components. There are also optional `ui` and `tachyon` features which provide additional components and systems for managing
UI state and input events.

### Core Widget Components

- `Widget`: Represents a ratatui widget that can be drawn on the screen, either as widget, stateful widget, or a render function.
- `WidgetOrder`: Component state that determines the draw order of widgets, allowing you to control which widgets are drawn on top of others.
- `WidgetRect`: Component state that holds the position and size of a widget on the screen.
- `WidgetLayout`: Component state that recalculates `WidgetRect` based on the current terminal frame size.

### Optional Ui Components & Functions

To enable the UI features, add the `ui` feature to your Cargo.toml.

- `UiFocusable`: Marks a widget as focusable, allowing it to receive input events and be navigated to using keyboard/mouse controls.
- `UiHoverable`: Marks a widget as hoverable, allowing it to respond to mouse hover events.
- `UiPressable`: Marks a widget as pressable, allowing it to respond to mouse click events.
- `UiDisabled`: Marks a widget as disabled, preventing it from receiving input events and applying a disabled style.
- `UiFocused`: Indicates that a widget is currently focused, which can be used to apply different styles or behaviors when the widget is active.
- `UiHovered`: Indicates that a widget is currently being hovered over by the mouse, which can be used to apply different styles or behaviors
  when the widget is hovered.
- `UiPressed`: Indicates that a widget is currently being pressed (clicked) by the mouse, which can be used to apply different styles or
  behaviors when the widget is pressed.
- `UiActions`: A component that holds a list of actions that can be triggered by input events, allowing you to define input handlers.
- `UiFocusMessage`: A message that can be sent to request focus on a specific `Focusable` entity. Sets the `UiFocused` component.
- `UiBuilder`: A helper struct for building UI widgets with a fluent interface, allowing you to easily configure the various UI components.

### Optional Tachyon Components & Functions

To enable the Tachyon features, add the `tachyonfx` feature to your Cargo.toml.

Since Tachyon effects are no send/sync, effects are stored in a non-send resource. In order to enable an effect on a frame area, you can attach
a `TachyonEffect` component to an entity with a `DrawArea` and `DrawFn`. The `TachyonEffect` just marks the entity as having a tachyon effect,
and the actual effect(s) are stored in the `TachyonRegistry` resource.

To remove the effect from the registry, simply remove the `TachyonEffect` component from the entity. Plurimus will automatically clean up any
effects associated with that entity.

- `TachyonEffect`: Marks an entity as having a tachyon effect, allowing it to be registered in the `TachyonRegistry` and rendered.
- `enable_fx`: Helper function to add the `TachyonEffect` component and register the effect in the `TachyonRegistry`.
- `add_fx`: Helper function to add an effect to the `TachyonRegistry` for an entity that already has a `TachyonEffect` component.

## Usage

See the examples folder for more complete examples, but here is a simple example of how to use Plurimus a simple "Hello, world!" widget:

```rust,ignore
use bevy::MinimalPlugins;
use bevy::prelude::*;
use bevy_ratatui::RatatuiPlugins;
use plurimus::{PlurimusPlugin, Widget, WidgetLayout};

fn main() {
    let mut app = App::new();

    app.add_plugins((
        PlurimusPlugin,
        MinimalPlugins,
        RatatuiPlugins {
            enable_kitty_protocol: true,
            enable_input_forwarding: true,
            enable_mouse_capture: true,
        },
    ));

    app.add_systems(Startup, startup);
  
    app.run();
}

fn startup(mut commands: Commands) -> Result {
    use ratatui::prelude::Rect;
    use ratatui::widgets::Paragraph;

    commands.spawn((
        Widget::from_widget(Paragraph::new("! Hello, world !")),
        WidgetLayout::new(|area| {
            let width = area.width / 2;
            let height = area.height / 2;
            let x = (area.width - width) / 2;
            let y = (area.height - height) / 2;
            Rect::new(x, y, width, height)
        }),
    ));

    Ok(())
}
```
