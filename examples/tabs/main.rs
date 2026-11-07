//! A tab bar switching panels, and a second bar choosing its look.
//!
//! The top strip is an uncontrolled bar: `tablist_self_update` moves the
//! selection, and the app only swaps the panel beneath it. It selects as
//! focus moves, so Left/Right and Home/End switch panels directly; `[`/`]`
//! and `1`-`4` are the app's own, sending the request upstream's keys would.
//! The Settings panel holds a vertical bar listing the looks the top bar can
//! take - plain, divided, boxed, joined, padded - which is a controlled bar:
//! the app answers its `ValueChange` by writing the top bar's `TabBarLook`.
//! Tab moves focus between the two bars while Settings is open; `q` or
//! ctrl-c quits.
//!
//! Laid out for a terminal of roughly 60x14 or larger.

use std::time::Duration;

use bevy_app::{App, AppExit, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::prelude::{
    ChildOf, Children, Commands, Component, Entity, MessageReader, MessageWriter, On, Query,
    ResMut, With, Without,
};
use bevy_ecs::system::SystemParam;
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::ratatui_core::style::{Color, Style};
use plurimus::core::{CorePlugin, Edge, Hidden, Propagate, TerminalCamera, UiArea, UiWidget};
use plurimus::crossterm::CrosstermPlugin;
use plurimus::term::bevy_input::ButtonState;
use plurimus::term::bevy_input::keyboard::{Key, KeyboardInput};
use plurimus::ui::bevy_input_focus::{FocusCause, InputFocus};
use plurimus::ui::{UiLabel, ValueChange};
use plurimus::widgets::ratatui_widgets::borders::BorderType;
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus::widgets::{
    ControlOrientation, SelectedTab, TabActivation, TabBarActiveStyle, TabBarLook, TabList,
    WidgetsPlugin, pane, tab_bar, tab_item, tablist_self_update,
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

/// The looks the top bar can take, by name.
const LOOKS: [(&str, fn() -> TabBarLook); 5] = [
    ("joined", || {
        TabBarLook::default()
            .with_border(Some(BorderType::Rounded))
            .with_joined(Some(Edge::Bottom))
    }),
    ("boxed", || {
        TabBarLook::default().with_border(Some(BorderType::Plain))
    }),
    ("divided", || {
        TabBarLook::default().with_divider(Some("\u{2502}".into()))
    }),
    ("plain", TabBarLook::default),
    ("padded", || TabBarLook::default().with_padding(3)),
];

const TOP_BAR: Rect = Rect::new(0, 0, 60, 3);
const PANEL_PANE: Rect = Rect::new(0, 3, 60, 10);
const PANEL_LINE: Rect = Rect::new(2, 4, 56, 1);
const LOOKS_BAR: Rect = Rect::new(2, 6, 14, 5);
const STATUS: Rect = Rect::new(0, 13, 60, 1);

const ACTIVE_FG: Color = Color::Rgb(122, 194, 255);

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
    app.add_systems(Update, (panel_shortcuts, swap_focus_on_tab, quit_on_key));
    app.add_observer(tablist_self_update);
    app.add_observer(switch_panel);
    app.add_observer(apply_look);
}

// Both bars select as focus moves, so stepping is choosing.
const AUTOMATIC: TabList = TabList {
    orientation: ControlOrientation::Horizontal,
    activation: TabActivation::Automatic,
};

fn spawn_demo(mut commands: Commands) {
    commands.spawn(TerminalCamera::default());
    let top = commands
        .spawn((
            tab_bar(),
            TopBar,
            AUTOMATIC,
            LOOKS[0].1(),
            TabBarActiveStyle(Style::new().fg(ACTIVE_FG)),
            UiArea::Fixed(TOP_BAR),
        ))
        .id();
    let first = spawn_tabs(
        &mut commands,
        top,
        PANELS
            .iter()
            .enumerate()
            .map(|(index, (label, _))| (*label, Panel(index))),
    );
    commands.spawn((pane(PANELS[0].0), PanelPane, UiArea::Fixed(PANEL_PANE)));
    commands.spawn((
        UiWidget::new(Paragraph::new(PANELS[0].1)),
        PanelLine,
        UiArea::Fixed(PANEL_LINE),
    ));
    spawn_looks_bar(&mut commands);
    commands.spawn((
        UiWidget::new(Paragraph::new(
            " \u{2190}/\u{2192} [ ] 1-4 switch tabs - Tab moves to the looks on Settings - q quits",
        )),
        UiArea::Fixed(STATUS),
    ));
    if let Some(first) = first {
        commands.insert_resource(InputFocus::from_entity(first));
    }
}

// Hidden until Settings opens, and vertical, so Up and Down pick a look.
fn spawn_looks_bar(commands: &mut Commands) {
    let looks = commands
        .spawn((
            tab_bar(),
            LooksBar,
            TabList {
                orientation: ControlOrientation::Vertical,
                ..AUTOMATIC
            },
            TabBarActiveStyle(Style::new().fg(ACTIVE_FG)),
            Propagate(Hidden),
            UiArea::Fixed(LOOKS_BAR),
        ))
        .id();
    spawn_tabs(
        commands,
        looks,
        LOOKS
            .iter()
            .enumerate()
            .map(|(index, (name, _))| (*name, Look(index))),
    );
}

/// Spawns `bar`'s tabs, each labelled and tagged, and selects the first.
fn spawn_tabs<C: Component>(
    commands: &mut Commands,
    bar: Entity,
    tabs: impl Iterator<Item = (&'static str, C)>,
) -> Option<Entity> {
    let spawned: Vec<Entity> = tabs
        .map(|(label, tag)| commands.spawn((tab_item(label), tag, ChildOf(bar))).id())
        .collect();
    let first = spawned.first().copied();
    commands.entity(bar).insert(SelectedTab(first));
    first
}

/// The top bar's tabs in order, and the one selected.
type TopTabs<'w, 's> =
    Query<'w, 's, (Entity, &'static Children, &'static SelectedTab), With<TopBar>>;

// Upstream's keys are fixed, so keys of the app's own send the request
// they would: a selection change on the bar, and focus to the tab.
fn panel_shortcuts(
    mut keys: MessageReader<KeyboardInput>,
    parents: Query<&ChildOf>,
    top: TopTabs,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let Ok((bar, tabs, selected)) = top.single() else {
        return;
    };
    for key in keys.read() {
        if key.state != ButtonState::Pressed || key.repeat || !is_on_tab_of(&focus, &parents, bar) {
            continue;
        }
        let current = tabs.iter().position(|tab| Some(*tab) == selected.0);
        let Some(index) = shortcut_index(&key.logical_key, current, tabs.len()) else {
            continue;
        };
        let tab = tabs[index];
        focus.set(tab, FocusCause::Navigated);
        commands.trigger(ValueChange {
            source: bar,
            value: Some(tab),
            is_final: true,
        });
    }
}

// Focus sits on a tab, so a bar holds it while one of its tabs does.
fn is_on_tab_of(focus: &InputFocus, parents: &Query<&ChildOf>, bar: Entity) -> bool {
    focus
        .get()
        .and_then(|tab| parents.get(tab).ok())
        .is_some_and(|parent| parent.parent() == bar)
}

fn shortcut_index(key: &Key, current: Option<usize>, count: usize) -> Option<usize> {
    let Key::Character(text) = key else {
        return None;
    };
    match text.as_str() {
        "[" => current?.checked_sub(1),
        "]" => current
            .map(|index| index + 1)
            .filter(|&index| index < count),
        digit => digit
            .parse::<usize>()
            .ok()
            .and_then(|digit| digit.checked_sub(1))
            .filter(|&index| index < count),
    }
}

#[derive(SystemParam)]
struct Panels<'w, 's> {
    top: Query<'w, 's, (), With<TopBar>>,
    items: Query<'w, 's, &'static Panel>,
    pane: Query<'w, 's, &'static mut UiLabel, With<PanelPane>>,
    line: Query<'w, 's, &'static mut UiWidget, With<PanelLine>>,
    looks: Query<'w, 's, (Entity, &'static SelectedTab), With<LooksBar>>,
    focus: ResMut<'w, InputFocus>,
}

// Only the panel beneath the bar is the app's to swap: which tab is
// selected, `tablist_self_update` already moved.
fn switch_panel(
    change: On<ValueChange<Option<Entity>>>,
    mut panels: Panels,
    mut commands: Commands,
) {
    if !panels.top.contains(change.source) {
        return;
    }
    let Some(Ok(panel)) = change.value.map(|tab| panels.items.get(tab)) else {
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
    for (looks, chosen) in &panels.looks {
        if settings {
            commands.entity(looks).remove::<Propagate<Hidden>>();
            if let Some(chosen) = chosen.0 {
                panels.focus.set(chosen, FocusCause::Navigated);
            }
        } else {
            commands.entity(looks).insert(Propagate(Hidden));
        }
    }
}

// The looks bar is controlled: the app applies the choice to the top bar
// and lets the self-update mark the row.
fn apply_look(
    change: On<ValueChange<Option<Entity>>>,
    bars: Query<(), With<LooksBar>>,
    looks: Query<&Look>,
    mut top: Query<&mut TabBarLook, With<TopBar>>,
) {
    if !bars.contains(change.source) {
        return;
    }
    let Some(Ok(look)) = change.value.map(|tab| looks.get(tab)) else {
        return;
    };
    for mut current in &mut top {
        *current = LOOKS[look.0].1();
    }
}

// Focus is the app's to move between two bars that are not in one
// hierarchy; Tab alternates their selected tabs while both are shown.
fn swap_focus_on_tab(
    mut keys: MessageReader<KeyboardInput>,
    parents: Query<&ChildOf>,
    top: Query<(Entity, &SelectedTab), With<TopBar>>,
    looks: Query<&SelectedTab, (With<LooksBar>, Without<Hidden>)>,
    mut focus: ResMut<InputFocus>,
) {
    for key in keys.read() {
        if key.state != ButtonState::Pressed || key.repeat || key.logical_key != Key::Tab {
            continue;
        }
        let (Ok((top, top_tab)), Ok(looks_tab)) = (top.single(), looks.single()) else {
            return;
        };
        let next = if is_on_tab_of(&focus, &parents, top) {
            looks_tab.0
        } else {
            top_tab.0
        };
        if let Some(next) = next {
            focus.set(next, FocusCause::Navigated);
        }
    }
}

fn quit_on_key(mut keys: MessageReader<KeyboardInput>, mut exit: MessageWriter<AppExit>) {
    for key in keys.read() {
        if key.state == ButtonState::Pressed
            && !key.repeat
            && key.logical_key == Key::Character("q".into())
        {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests;
