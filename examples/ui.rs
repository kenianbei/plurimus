#![cfg(feature = "ui")]

use bevy::prelude::*;
use bevy_ratatui::RatatuiPlugins;
use plurimus::*;
use std::sync::Arc;

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

    app.ui_actions_message::<UiFocusMessage>();
    app.ui_actions_message::<Clicked>();
    app.add_systems(Startup, startup);
    app.add_systems(Update, (clicked, sync_state_with_widget).chain());

    app.run();
}

#[derive(Clone, Copy, Component, PartialEq)]
enum Marker {
    First,
    Second,
}

type WidgetState = (&'static str, bool, bool, bool, bool);

#[derive(Message, Clone, Copy)]
struct Clicked(Marker);

impl Marker {
    fn actions(self) -> Vec<UiInputBinding> {
        use bevy_ratatui::crossterm::event::{KeyCode, MouseButton};

        match self {
            Marker::First => vec![
                UiInputBinding::key_message(
                    KeyBinding::press(KeyCode::Enter),
                    Clicked(Marker::First),
                ),
                UiInputBinding::mouse_message(
                    MouseBinding::up(MouseButton::Left),
                    Clicked(Marker::First),
                ),
                UiInputBinding::key_message(
                    KeyBinding::press(KeyCode::Tab),
                    UiFocusMessage::next(),
                ),
            ],
            Marker::Second => vec![
                UiInputBinding::key_message(
                    KeyBinding::press(KeyCode::Enter),
                    Clicked(Marker::Second),
                ),
                UiInputBinding::mouse_message(
                    MouseBinding::up(MouseButton::Left),
                    Clicked(Marker::Second),
                ),
                UiInputBinding::key_message(
                    KeyBinding::press(KeyCode::Tab),
                    UiFocusMessage::next(),
                ),
            ],
        }
    }

    fn layout(self) -> (LayoutFn, i32) {
        use ratatui::prelude::{Constraint, Direction, Layout, Rect};

        let layout_fn = move |area: &Rect| -> Rect {
            let [_, bottom, _] = Layout::default()
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Length(3),
                    Constraint::Fill(1),
                ])
                .direction(Direction::Vertical)
                .margin(1)
                .areas(*area);

            let [_, first, second, _] = Layout::default()
                .constraints([
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                ])
                .direction(Direction::Horizontal)
                .spacing(1)
                .areas(bottom);

            match self {
                Marker::First => first,
                Marker::Second => second,
            }
        };

        (Arc::new(layout_fn), 0)
    }
}

fn startup(mut commands: Commands) -> Result {
    UiBuilder::new(button_widget("First"))
        .interactive(0)
        .with_actions(Marker::First.actions())
        .with_layout(Marker::First.layout())
        .with_marker(Marker::First)
        .focused()
        .spawn(&mut commands);

    UiBuilder::new(button_widget("Second"))
        .interactive(1)
        .with_actions(Marker::Second.actions())
        .with_layout(Marker::Second.layout())
        .with_marker(Marker::Second)
        .spawn(&mut commands);

    Ok(())
}

fn button_widget(label: &'static str) -> Widget {
    use ratatui::prelude::Alignment;
    use ratatui::widgets::Paragraph;

    Widget::from_render_fn_with_state(
        move |frame, area, state| {
            let (label, disabled, focused, hovered, pressed) = *state;
            use ratatui::widgets::Widget;
            let block = interactive_block(disabled, focused, hovered, pressed);

            Paragraph::new(label)
                .alignment(Alignment::Center)
                .block(block)
                .render(area, frame.buffer_mut());

            Ok(())
        },
        (label, false, false, false, false),
    )
}

fn interactive_block<'a>(
    disabled: bool,
    focused: bool,
    hovered: bool,
    pressed: bool,
) -> ratatui::widgets::Block<'a> {
    use ratatui::prelude::Alignment;
    use ratatui::symbols::border::{DOUBLE, PLAIN};
    use ratatui::widgets::{Block, Borders};

    let mut b = Block::default()
        .borders(Borders::ALL)
        .title_alignment(Alignment::Center);

    if disabled {
        b = b.border_set(PLAIN);
    } else if pressed {
        b = b.border_set(PLAIN);
    } else if focused {
        b = b.border_set(DOUBLE);
    } else if hovered {
        b = b.border_set(DOUBLE);
    } else {
        b = b.border_set(PLAIN);
    }

    b
}

fn clicked(
    mut mr_clicked: MessageReader<Clicked>,
    mut q_widgets: Query<(&mut Widget, &Marker)>,
) -> Result {
    for Clicked(element) in mr_clicked.read() {
        for (mut w, e) in q_widgets.iter_mut() {
            if *e == *element {
                let state = w.get_state_mut::<WidgetState>()?;
                state.0 = match e {
                    Marker::First => "Clicked First!",
                    Marker::Second => "Clicked Second!",
                };
                state.2 = true;
            }
        }
    }
    Ok(())
}

fn sync_state_with_widget(
    mut q_widgets: Query<
        (
            Option<&UiDisabled>,
            Option<&UiFocused>,
            Option<&UiHovered>,
            Option<&UiPressed>,
            &mut Widget,
        ),
        With<Marker>,
    >,
) -> Result {
    for (disabled, focused, hovered, pressed, mut widget) in q_widgets.iter_mut() {
        let state = widget.get_state_mut::<WidgetState>()?;
        state.1 = disabled.is_some();
        state.2 = focused.is_some();
        state.3 = hovered.is_some();
        state.4 = pressed.is_some();
    }
    Ok(())
}
