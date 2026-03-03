#![cfg(feature = "widget")]

use bevy::prelude::*;
use bevy_ratatui::RatatuiPlugins;
use plurimus::*;

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
    app.add_systems(Update, (update, update_list_state).chain());
    app.run();
}

#[derive(Component)]
struct HelloWorld;

#[derive(Component)]
struct ItemsList;

const ITEMS: [&str; 9] = [
    "Item 1", "Item 2", "Item 3", "Item 4", "Item 5", "Item 6", "Item 7", "Item 8", "Item 9",
];

fn startup(mut commands: Commands) -> Result {
    use ratatui::prelude::Style;
    use ratatui::widgets::{List, ListState, Paragraph};

    commands.spawn((
        HelloWorld,
        Widget::from_widget(Paragraph::new("! Hello, world !")),
        WidgetLayout::new(|area| center_layout(&layout(*area)[0], 16, 1)),
    ));

    commands.spawn((
        ItemsList,
        Widget::from_stateful(
            List::new(ITEMS)
                .style(Style::new().white())
                .highlight_style(Style::new().yellow().italic())
                .highlight_symbol("> ")
                .repeat_highlight_symbol(true),
            ListState::default().with_selected(Some(0)),
        ),
        WidgetLayout::new(|area| center_layout(&layout(*area)[1], 10, 8)),
    ));

    Ok(())
}

fn update(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut idx: Local<usize>,
    mut q_widget: Single<&mut Widget, With<HelloWorld>>,
) {
    use ratatui::widgets::Paragraph;

    let timer = timer.get_or_insert_with(|| Timer::from_seconds(0.25, TimerMode::Repeating));
    let bang = ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")"];

    if timer.tick(time.delta()).just_finished() {
        *idx = (*idx + 1) % bang.len();
        q_widget.set_widget(Paragraph::new(format!(
            "{} Hello, world {}",
            bang[*idx], bang[*idx]
        )));
    }
}

fn update_list_state(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut q_widget: Single<&mut Widget, With<ItemsList>>,
) {
    use ratatui::widgets::ListState;

    let timer = timer.get_or_insert_with(|| Timer::from_seconds(0.5, TimerMode::Repeating));

    if timer.tick(time.delta()).just_finished() {
        if let Ok(list_state) = q_widget.get_state_mut::<ListState>() {
            let selected = match list_state.selected() {
                Some(idx) => (idx + 1) % ITEMS.len(),
                None => 0,
            };
            list_state.select(Some(selected));
        }
    }
}

fn layout(area: ratatui::prelude::Rect) -> Vec<ratatui::prelude::Rect> {
    use ratatui::prelude::{Constraint, Direction, Layout};

    Layout::default()
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .direction(Direction::Vertical)
        .split(area)
        .to_vec()
}

fn center_layout(area: &ratatui::layout::Rect, width: u16, height: u16) -> ratatui::layout::Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    ratatui::layout::Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}
