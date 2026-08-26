//! A `TabBar` switching panels, and a second bar choosing its look.
//!
//! The top strip is an uncontrolled bar: `tab_bar_self_update` moves the
//! active tab, and the app only swaps the panel beneath it. Left/Right,
//! `[`/`]`, Home/End and `1`-`4` all drive it, the last two through a
//! remapped `TabBarKeys`. The Settings panel holds a vertical bar listing
//! the looks the top bar can take - plain, divided, boxed, joined, padded -
//! which is a controlled bar: the app answers its `ValueChange` by writing
//! the top bar's `TabBarLook`. Tab moves focus between the two bars while
//! Settings is open; `q` or ctrl-c quits.
//!
//! Laid out for a terminal of roughly 60x14 or larger.

use std::time::Duration;

use bevy_app::{App, AppExit, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::prelude::{
    ChildOf, Commands, Component, Entity, MessageReader, MessageWriter, On, Query, ResMut, With,
    Without,
};
use bevy_ecs::system::SystemParam;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::ratatui_core::style::{Color, Style};
use plurimus::core::{CorePlugin, Edge, TerminalCamera, UiArea, UiHidden, UiWidget};
use plurimus::crossterm::CrosstermPlugin;
use plurimus::term::{KeyCode, KeyKind, KeyMessage};
use plurimus::ui::{Checked, UiLabel, ValueChange};
use plurimus::widgets::ratatui_widgets::borders::BorderType;
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus::widgets::{
    Key, TabBarAction, TabBarActiveStyle, TabBarKeys, TabBarLook, TabBarOrientation, WidgetsPlugin,
    pane, tab_bar, tab_bar_self_update, tab_item,
};

/// Each panel's tab label and the line it shows.
const PANELS: [(&str, &str); 4] = [
    (
        "Diary",
        "08:10 oats and coffee - 12:30 lentil soup - 19:00 rice, greens, egg",
    ),
    (
        "Plan",
        "mon fast - tue 1800 kcal - wed 1800 kcal - thu fast - fri free",
    ),
    (
        "Foods",
        "oats 389 - lentils 116 - rice 130 - egg 155 - greens 23 (kcal / 100 g)",
    ),
    (
        "Settings",
        "Pick the top bar's look: Up/Down or click, Tab returns to the tabs",
    ),
];
const SETTINGS: usize = 3;

const LOOKS: [&str; 5] = ["joined", "boxed", "divided", "plain", "padded"];

const TOP_BAR: Rect = Rect::new(0, 0, 60, 3);
const PANEL_PANE: Rect = Rect::new(0, 3, 60, 10);
const PANEL_LINE: Rect = Rect::new(2, 4, 56, 1);
const LOOKS_BAR: Rect = Rect::new(2, 6, 14, 5);
const STATUS: Rect = Rect::new(0, 13, 60, 1);

const ACTIVE_BG: Color = Color::Rgb(40, 72, 96);

#[derive(Component)]
struct TopBar;

#[derive(Component)]
struct LooksBar;

#[derive(Component)]
struct PanelPane;

#[derive(Component)]
struct PanelLine;

/// Which panel a top-bar item opens.
#[derive(Component)]
struct Panel(usize);

/// Which look a looks-bar item names.
#[derive(Component)]
struct Look(usize);

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(Duration::from_millis(16)),
        CorePlugin,
        CrosstermPlugin::default(),
    ));
    add_demo(&mut app);
    app.run()
}

fn add_demo(app: &mut App) {
    app.add_plugins(WidgetsPlugin);
    app.add_systems(Startup, spawn_demo);
    app.add_systems(Update, (swap_focus_on_tab, quit_on_key));
    app.add_observer(tab_bar_self_update);
    app.add_observer(switch_panel);
    app.add_observer(apply_look);
}

fn spawn_demo(mut commands: Commands) {
    commands.spawn(TerminalCamera::default());
    let top = commands
        .spawn((
            tab_bar(),
            TopBar,
            look_named(LOOKS[0]),
            TabBarActiveStyle(Style::new().bg(ACTIVE_BG)),
            panel_keys(),
            UiArea::Fixed(TOP_BAR),
        ))
        .id();
    for (index, (label, _)) in PANELS.iter().enumerate() {
        let item = commands
            .spawn((tab_item(*label), Panel(index), ChildOf(top)))
            .id();
        if index == 0 {
            commands.entity(item).insert(Checked);
        }
    }
    commands.spawn((pane(PANELS[0].0), PanelPane, UiArea::Fixed(PANEL_PANE)));
    commands.spawn((
        UiWidget::new(Paragraph::new(PANELS[0].1)),
        PanelLine,
        UiArea::Fixed(PANEL_LINE),
    ));
    let looks = commands
        .spawn((
            tab_bar(),
            LooksBar,
            TabBarLook::default().with_orientation(TabBarOrientation::Vertical),
            TabBarActiveStyle(Style::new().bg(ACTIVE_BG)),
            UiHidden,
            UiArea::Fixed(LOOKS_BAR),
        ))
        .id();
    for (index, name) in LOOKS.iter().enumerate() {
        let item = commands
            .spawn((tab_item(*name), Look(index), ChildOf(looks)))
            .id();
        if index == 0 {
            commands.entity(item).insert(Checked);
        }
    }
    commands.spawn((
        UiWidget::new(Paragraph::new(
            " \u{2190}/\u{2192} [ ] 1-4 switch tabs - Tab moves to the looks on Settings - q quits",
        )),
        UiArea::Fixed(STATUS),
    ));
    commands.insert_resource(InputFocus::from_entity(top));
}

// The stock arrows plus the brackets and a digit per panel, which is the
// whole cost of remapping: each is one binding naming its action.
fn panel_keys() -> TabBarKeys {
    let mut keys = TabBarKeys::default();
    keys.0.extend([
        (Key::Character("[".into()).into(), TabBarAction::Previous),
        (Key::Character("]".into()).into(), TabBarAction::Next),
    ]);
    keys.0.extend((0..PANELS.len()).map(|index| {
        let digit = Key::Character((index + 1).to_string().into());
        (digit.into(), TabBarAction::Select(index))
    }));
    keys
}

fn look_named(name: &str) -> TabBarLook {
    match name {
        "boxed" => TabBarLook::default().with_border(Some(BorderType::Plain)),
        "divided" => TabBarLook::default().with_divider(Some("\u{2502}".into())),
        "plain" => TabBarLook::default(),
        "padded" => TabBarLook::default().with_padding(3),
        _ => TabBarLook::default()
            .with_border(Some(BorderType::Rounded))
            .with_joined(Some(Edge::Bottom)),
    }
}

#[derive(SystemParam)]
struct Panels<'w, 's> {
    top: Query<'w, 's, (), With<TopBar>>,
    items: Query<'w, 's, &'static Panel>,
    pane: Query<'w, 's, &'static mut UiLabel, With<PanelPane>>,
    line: Query<'w, 's, &'static mut UiWidget, With<PanelLine>>,
    looks: Query<'w, 's, Entity, With<LooksBar>>,
    focus: ResMut<'w, InputFocus>,
}

// Only the panel beneath the bar is the app's to swap: which tab is
// active, `tab_bar_self_update` already moved.
fn switch_panel(change: On<ValueChange<Entity>>, mut panels: Panels, mut commands: Commands) {
    if !panels.top.contains(change.source) {
        return;
    }
    let Ok(panel) = panels.items.get(change.value) else {
        return;
    };
    let (title, text) = PANELS[panel.0];
    for mut label in &mut panels.pane {
        label.0 = title.into();
    }
    for mut line in &mut panels.line {
        *line = UiWidget::new(Paragraph::new(text));
    }
    let settings = panel.0 == SETTINGS;
    for looks in &panels.looks {
        if settings {
            commands.entity(looks).remove::<UiHidden>();
            panels.focus.set(looks, FocusCause::Navigated);
        } else {
            commands.entity(looks).insert(UiHidden);
            panels.focus.set(change.source, FocusCause::Navigated);
        }
    }
}

// The looks bar is controlled: the app applies the choice to the top bar
// and lets the self-update mark the row.
fn apply_look(
    change: On<ValueChange<Entity>>,
    bars: Query<(), With<LooksBar>>,
    looks: Query<&Look>,
    mut top: Query<&mut TabBarLook, With<TopBar>>,
) {
    if !bars.contains(change.source) {
        return;
    }
    let Ok(look) = looks.get(change.value) else {
        return;
    };
    for mut current in &mut top {
        *current = look_named(LOOKS[look.0]);
    }
}

// Focus is the app's to move between two bars that are not in one
// hierarchy; Tab alternates them while both are shown.
fn swap_focus_on_tab(
    mut keys: MessageReader<KeyMessage>,
    top: Query<Entity, With<TopBar>>,
    looks: Query<Entity, (With<LooksBar>, Without<UiHidden>)>,
    mut focus: ResMut<InputFocus>,
) {
    let (Ok(top), Ok(looks)) = (top.single(), looks.single()) else {
        return;
    };
    for key in keys.read() {
        if key.kind != KeyKind::Press || key.code != KeyCode::Tab {
            continue;
        }
        let next = if focus.get() == Some(top) { looks } else { top };
        focus.set(next, FocusCause::Navigated);
    }
}

fn quit_on_key(mut keys: MessageReader<KeyMessage>, mut exit: MessageWriter<AppExit>) {
    for key in keys.read() {
        if key.kind == KeyKind::Press && key.code == KeyCode::Char('q') {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests;
