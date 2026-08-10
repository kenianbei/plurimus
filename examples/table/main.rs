//! A sortable, scrollable `Table`: a striped body under a bold header, a
//! footer of totals, and columns the app sorts by clicking their header.
//!
//! Up/Down or `j`/`k` move the cursor, Enter selects the row, the wheel
//! scrolls, and clicking a header cell sorts by that column - descending
//! the second time. `q` or ctrl-c quits.
//!
//! Sorting lives here rather than in the widget on purpose. The crate
//! reports which column was clicked; how a column compares - text here,
//! numbers there - is something only the app knows, and reordering the row
//! entities is all it takes to redraw.
//!
//! A scroll area windows a widget whole, so the header and the footer are
//! scrolled content like any row: the totals sit below the fold until the
//! body has been scrolled past, and the header leaves the top as they
//! arrive. A table that must keep its header in view is two tables, an
//! unscrolled header above the scrolled rows, sharing one set of widths.
//!
//! Laid out for a terminal of roughly 60x16 or larger.

use std::time::Duration;

use bevy_app::{App, AppExit, ScheduleRunnerPlugin, Startup, Update};
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{
    ChildOf, Children, Commands, Component, Entity, MessageReader, MessageWriter, On, Query, Res,
    ResMut, Resource, With,
};
use bevy_input_focus::InputFocus;
use plurimus::core::ratatui_core::layout::{Constraint, Rect, Size};
use plurimus::core::ratatui_core::style::{Color, Modifier, Style};
use plurimus::core::ratatui_core::text::Line;
use plurimus::core::{CorePlugin, TerminalCamera, UiArea, UiWidget};
use plurimus::crossterm::CrosstermPlugin;
use plurimus::term::{KeyCode, KeyKind, KeyMessage};
use plurimus::ui::{ScrollArea, UiStyle, ValueChange};
use plurimus::widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus::widgets::{
    Key, TableAction, TableCheckedStyle, TableHeaderClick, TableKeys, TablePosition, TableRow,
    TableSelection, TableStripe, WidgetsPlugin, pane, table, table_footer, table_header, table_row,
    table_self_update,
};

const COLUMNS: [&str; 4] = ["process", "pid", "cpu%", "mem"];
const WIDTHS: [Constraint; 4] = [
    Constraint::Fill(1),
    Constraint::Length(6),
    Constraint::Length(6),
    Constraint::Length(7),
];

/// pid, cpu percent, memory in MiB.
const PROCESSES: [(&str, u32, f32, u32); 10] = [
    ("systemd", 1, 0.1, 12),
    ("wayland", 412, 3.4, 210),
    ("kitty", 981, 1.2, 96),
    ("cargo", 2044, 88.6, 1420),
    ("rustc", 2051, 74.3, 2810),
    ("helix", 1330, 0.4, 64),
    ("pipewire", 620, 0.9, 38),
    ("firefox", 1702, 12.7, 3180),
    ("sshd", 744, 0.0, 8),
    ("nushell", 1988, 0.2, 22),
];

const PANE: Rect = Rect::new(0, 0, 56, 12);
const TABLE: Rect = Rect::new(1, 1, 54, 10);
const STATUS: Rect = Rect::new(0, 12, 56, 1);

const STRIPE: Color = Color::Rgb(32, 32, 40);
const CHECKED: Color = Color::Rgb(24, 56, 72);

#[derive(Component)]
struct ProcessTable;

#[derive(Component)]
struct StatusLine;

/// One row's data, kept beside its cells so sorting compares values rather
/// than the text they were formatted into.
#[derive(Component)]
struct Process {
    name: &'static str,
    pid: u32,
    cpu: f32,
    mem: u32,
}

#[derive(Resource, Default)]
struct Sorted {
    column: Option<usize>,
    descending: bool,
}

#[derive(Resource, Default)]
struct Selection(String);

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
    app.init_resource::<Sorted>();
    app.init_resource::<Selection>();
    app.add_systems(Startup, spawn_demo);
    app.add_systems(Update, (update_status, quit_on_key));
    app.add_observer(table_self_update);
    app.add_observer(remember_selection);
    app.add_observer(sort_by_column);
}

fn spawn_demo(mut commands: Commands) {
    commands.spawn(TerminalCamera::default());
    commands.spawn((pane("processes"), UiArea::Fixed(PANE)));
    commands.spawn((
        UiWidget::new(Paragraph::new("")),
        UiArea::Fixed(STATUS),
        StatusLine,
    ));

    let table = commands
        .spawn((
            table(WIDTHS),
            ProcessTable,
            TableSelection::Row,
            TableStripe(Style::new().bg(STRIPE)),
            TableCheckedStyle(Style::new().bg(CHECKED)),
            vim_keys(),
            ScrollArea::new(Size::new(TABLE.width, 0)),
            UiArea::Fixed(TABLE),
        ))
        .id();
    commands.spawn((
        table_header(COLUMNS),
        // The header is an ordinary row entity, so bolding it is the same
        // override any other row would take - no theme field of its own.
        UiStyle(Style::new().add_modifier(Modifier::BOLD)),
        ChildOf(table),
    ));
    for (name, pid, cpu, mem) in PROCESSES {
        commands.spawn((
            table_row(cells(name, pid, cpu, mem)),
            Process {
                name,
                pid,
                cpu,
                mem,
            },
            ChildOf(table),
        ));
    }
    commands.spawn((
        table_footer(totals()),
        UiStyle(Style::new().fg(Color::DarkGray)),
        ChildOf(table),
    ));
    commands.insert_resource(InputFocus::from_entity(table));
}

// The stock bindings plus `j`/`k`, which is the whole cost of remapping.
fn vim_keys() -> TableKeys {
    let mut keys = TableKeys::default();
    keys.0
        .push((Key::Character("j".into()), TableAction::RowNext));
    keys.0
        .push((Key::Character("k".into()), TableAction::RowPrev));
    keys
}

fn cells(name: &str, pid: u32, cpu: f32, mem: u32) -> Vec<Line<'static>> {
    vec![
        Line::from(name.to_owned()),
        Line::from(pid.to_string()).right_aligned(),
        Line::from(format!("{cpu:.1}")).right_aligned(),
        Line::from(format!("{mem} M")).right_aligned(),
    ]
}

fn totals() -> Vec<Line<'static>> {
    let cpu: f32 = PROCESSES.iter().map(|process| process.2).sum();
    let mem: u32 = PROCESSES.iter().map(|process| process.3).sum();
    vec![
        Line::from(format!("{} processes", PROCESSES.len())),
        Line::from(""),
        Line::from(format!("{cpu:.1}")).right_aligned(),
        Line::from(format!("{mem} M")).right_aligned(),
    ]
}

/// The app's half of the sorting contract: the widget says which column was
/// clicked, and the row entities are reordered in place.
fn sort_by_column(
    click: On<TableHeaderClick>,
    mut sorted: ResMut<Sorted>,
    tables: Query<&Children, With<ProcessTable>>,
    rows: Query<(Entity, &Process)>,
    mut commands: Commands,
) {
    let Ok(children) = tables.get(click.entity) else {
        return;
    };
    sorted.descending = sorted.column == Some(click.column) && !sorted.descending;
    sorted.column = Some(click.column);

    let mut body: Vec<(Entity, &Process)> = children
        .iter()
        .filter_map(|&child| rows.get(child).ok())
        .collect();
    body.sort_by(|left, right| order(click.column, left.1, right.1));
    if sorted.descending {
        body.reverse();
    }
    let order: Vec<Entity> = body.iter().map(|(row, _)| *row).collect();
    commands
        .entity(click.entity)
        .detach_children(&order)
        .add_children(&order);
}

// Text sorts as text and numbers as numbers, which is the judgement the
// crate cannot make for an app.
fn order(column: usize, left: &Process, right: &Process) -> core::cmp::Ordering {
    match column {
        1 => left.pid.cmp(&right.pid),
        2 => left.cpu.total_cmp(&right.cpu),
        3 => left.mem.cmp(&right.mem),
        _ => left.name.cmp(right.name),
    }
}

fn name_of(cells: &TableRow) -> String {
    cells.0.first().map(Line::to_string).unwrap_or_default()
}

fn remember_selection(
    change: On<ValueChange<TablePosition>>,
    rows: Query<&TableRow>,
    mut selection: ResMut<Selection>,
) {
    let Some(row) = change.value.row else {
        return;
    };
    if let Ok(cells) = rows.get(row) {
        selection.0 = name_of(cells);
    }
}

fn update_status(
    sorted: Res<Sorted>,
    selection: Res<Selection>,
    mut lines: Query<&mut UiWidget, With<StatusLine>>,
) {
    if !sorted.is_changed() && !selection.is_changed() {
        return;
    }
    let column = sorted
        .column
        .and_then(|column| COLUMNS.get(column))
        .unwrap_or(&"none");
    let arrow = if sorted.descending { "desc" } else { "asc" };
    let selected = if selection.0.is_empty() {
        "nothing"
    } else {
        &selection.0
    };
    for mut widget in &mut lines {
        *widget = UiWidget::new(Paragraph::new(format!(
            " sorted by {column} ({arrow}) - selected {selected} - click a header to sort"
        )));
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
