//! The page's widgets, shared by the browser build and the headless tests.

use bevy_app::{App, AppExit, Startup, Update};
use bevy_ecs::prelude::{
    ChildOf, Commands, Component, DetectChanges, Entity, MessageWriter, On, Query, Res, ResMut,
    Resource, With,
};
use bevy_input_focus::tab_navigation::TabGroup;
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::{TerminalCamera, UiArea, UiWidget};
use plurimus::term::TerminalRequest;
use plurimus::ui::{UiLabel, ValueChange};
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus::widgets::{
    Activate, WidgetsPlugin, button, list_item, listbox, listbox_self_update, text_editor,
};

// The grid the page asks for; every rect below fits in it.
pub const COLUMNS: u16 = 80;
pub const ROWS: u16 = 24;

const STATUS: Rect = Rect::new(1, 0, 78, 1);
pub const LIST: Rect = Rect::new(1, 2, 20, 6);
pub const TITLE_BUTTON: Rect = Rect::new(1, 10, 20, 1);
pub const QUIT_BUTTON: Rect = Rect::new(1, 12, 20, 1);
const EDITOR: Rect = Rect::new(23, 2, 56, 21);

pub const FRUITS: [&str; 6] = ["apple", "banana", "cherry", "damson", "elder", "fig"];
/// Enough lines that the editor has somewhere to scroll.
const EDITOR_LINES: usize = 60;

#[derive(Resource, Default)]
pub struct DemoState {
    pub fruit: Option<String>,
    titles: u32,
}

#[derive(Component)]
struct StatusLine;

pub fn add_demo(app: &mut App) {
    app.add_plugins(WidgetsPlugin);
    app.init_resource::<DemoState>();
    app.add_systems(Startup, spawn_page);
    app.add_systems(Update, show_status);
}

/// Every widget keeps the stock tab index, so focus moves in spawn order.
fn spawn_page(mut commands: Commands) {
    commands.spawn(TerminalCamera::default());
    commands.spawn((
        UiWidget::new(Paragraph::new("")),
        UiArea::Fixed(STATUS),
        StatusLine,
    ));
    let root = commands.spawn(TabGroup::new(0)).id();
    let list = commands
        .spawn((listbox(), UiArea::Fixed(LIST), ChildOf(root)))
        .observe(listbox_self_update)
        .observe(pick_fruit)
        .id();
    for fruit in FRUITS {
        commands.spawn((list_item(fruit), ChildOf(list)));
    }
    let text = (1..=EDITOR_LINES)
        .map(|line| format!("line {line}: type, paste, select with shift-arrows and ctrl-c"))
        .collect::<Vec<_>>()
        .join("\n");
    commands.spawn((text_editor(text), UiArea::Fixed(EDITOR), ChildOf(root)));
    commands
        .spawn((
            button("set the title"),
            UiArea::Fixed(TITLE_BUTTON),
            ChildOf(root),
        ))
        .observe(set_title);
    commands
        .spawn((button("quit"), UiArea::Fixed(QUIT_BUTTON), ChildOf(root)))
        .observe(|_: On<Activate>, mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit::Success);
        });
}

fn pick_fruit(on: On<ValueChange<Entity>>, labels: Query<&UiLabel>, mut state: ResMut<DemoState>) {
    if let Ok(label) = labels.get(on.value) {
        state.fruit = Some(label.0.to_string());
    }
}

fn set_title(
    _: On<Activate>,
    mut state: ResMut<DemoState>,
    mut requests: MessageWriter<TerminalRequest>,
) {
    state.titles += 1;
    requests.write(TerminalRequest::SetTitle(format!(
        "plurimus web - titled {} times",
        state.titles
    )));
}

fn show_status(state: Res<DemoState>, mut lines: Query<&mut UiWidget, With<StatusLine>>) {
    if !state.is_changed() {
        return;
    }
    let fruit = state.fruit.as_deref().unwrap_or("none");
    let text = format!("picked {fruit}  titled {}  - tab moves focus", state.titles);
    for mut widget in &mut lines {
        *widget = UiWidget::new(Paragraph::new(text.clone()));
    }
}
