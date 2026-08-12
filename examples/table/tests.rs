use bevy_input_focus::InputFocus;
use plurimus::core::TerminalSize;
use plurimus::term::MouseKind;
use plurimus::widgets::ActiveDescendant;
use plurimus_test::{click, composed_frame, press_key, send_mouse};

use super::*;

const TEST_SIZE: TerminalSize = TerminalSize::new(60, 14);

/// The header band: the pane's border takes the row above it.
const HEADER_Y: u16 = TABLE.y;
/// Inside the third column, past the filling name column and the pid one.
const CPU_COLUMN_X: u16 = 44;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(TEST_SIZE);
    add_demo(&mut app);
    app.update();
    app
}

fn table_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut tables = world.query_filtered::<Entity, With<ProcessTable>>();
    tables.single(world).expect("one process table")
}

// The body rows in the order they are drawn.
fn row_names(app: &mut App) -> Vec<String> {
    let table = table_entity(app);
    let world = app.world_mut();
    let children: Vec<Entity> = world
        .get::<Children>(table)
        .expect("children")
        .iter()
        .copied()
        .collect();
    children
        .into_iter()
        .filter(|&child| world.get::<Process>(child).is_some())
        .filter_map(|child| world.get::<TableRow>(child).map(name_of))
        .collect()
}

fn cursor_name(app: &mut App) -> Option<String> {
    let table = table_entity(app);
    let row = app.world().get::<ActiveDescendant>(table)?.0?;
    app.world().get::<TableRow>(row).map(name_of)
}

#[test]
fn the_demo_draws_its_header_body_and_footer() {
    let app = headless_app();

    let frame = composed_frame(&app);
    let mut lines = frame.lines();
    assert!(
        lines.next().is_some_and(|line| line.contains("processes")),
        "the pane border and title"
    );
    assert!(
        lines
            .next()
            .is_some_and(|line| line.contains("process") && line.contains("cpu%")),
        "the header band, one line inside the pane"
    );
    assert!(
        lines.next().is_some_and(|line| line.contains("systemd")),
        "then the first body row: {frame}"
    );
}

// The bands are part of the scrolled content, so the totals are below the
// fold until the body has been scrolled past.
#[test]
fn the_footer_arrives_at_the_bottom_of_the_scroll() {
    let mut app = headless_app();
    assert!(
        !composed_frame(&app).contains("10 processes"),
        "the footer starts out of view"
    );

    for _ in 0..4 {
        send_mouse(&mut app, MouseKind::ScrollDown, 20, 5);
    }

    let frame = composed_frame(&app);
    assert!(
        frame.contains("10 processes"),
        "and scrolls into it: {frame}"
    );
    assert!(
        !frame.contains("cpu%"),
        "taking the header out the other side, as a windowed widget does"
    );
}

#[test]
fn the_table_takes_focus_on_startup() {
    let mut app = headless_app();
    let table = table_entity(&mut app);

    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(table),
        "the only focusable widget starts focused"
    );
}

#[test]
fn clicking_a_header_sorts_by_that_column() {
    let mut app = headless_app();
    assert_eq!(
        row_names(&mut app).first().map(String::as_str),
        Some("systemd"),
        "the rows start in the order they were spawned"
    );

    // The cpu column of the header band, which is the pane's first inner row.
    click(&mut app, CPU_COLUMN_X, HEADER_Y);

    assert_eq!(
        row_names(&mut app).first().map(String::as_str),
        Some("sshd"),
        "ascending by cpu puts the idle process first"
    );

    click(&mut app, CPU_COLUMN_X, HEADER_Y);
    assert_eq!(
        row_names(&mut app).first().map(String::as_str),
        Some("cargo"),
        "and clicking the same column again reverses it"
    );
}

#[test]
fn sorting_reorders_the_body_and_leaves_the_bands_alone() {
    let mut app = headless_app();
    click(&mut app, CPU_COLUMN_X, HEADER_Y);

    let frame = composed_frame(&app);
    let mut lines = frame.lines();
    assert!(
        lines.next().is_some_and(|line| line.contains("processes")),
        "the pane title is still on top"
    );
    assert!(
        lines
            .next()
            .is_some_and(|line| line.contains("process") && line.contains("cpu%")),
        "the header band is still a header, not a sorted row: {frame}"
    );
    assert!(
        lines.next().is_some_and(|line| line.contains("sshd")),
        "and the body below it is in its new order"
    );
}

#[test]
fn the_vim_keys_walk_the_rows_beside_the_arrows() {
    let mut app = headless_app();

    press_key(&mut app, KeyCode::Char('j'));
    press_key(&mut app, KeyCode::Char('j'));
    assert_eq!(
        cursor_name(&mut app).as_deref(),
        Some("wayland"),
        "j moves down the body"
    );

    press_key(&mut app, KeyCode::Char('k'));
    assert_eq!(
        cursor_name(&mut app).as_deref(),
        Some("systemd"),
        "and k moves back up"
    );

    press_key(&mut app, KeyCode::Down);
    assert_eq!(
        cursor_name(&mut app).as_deref(),
        Some("wayland"),
        "the stock arrows still work alongside them"
    );
}

#[test]
fn selecting_a_row_reports_it() {
    let mut app = headless_app();

    press_key(&mut app, KeyCode::Char('j'));
    press_key(&mut app, KeyCode::Enter);
    app.update();

    assert_eq!(
        app.world().resource::<Selection>().0,
        "systemd",
        "the selection observer saw the row"
    );
    assert!(
        composed_frame(&app).contains("selected systemd"),
        "and the status line says so"
    );
}

#[test]
fn the_wheel_scrolls_the_body_under_the_header() {
    let mut app = headless_app();

    send_mouse(&mut app, MouseKind::ScrollDown, 20, 5);
    send_mouse(&mut app, MouseKind::ScrollDown, 20, 5);

    let frame = composed_frame(&app);
    assert!(
        !frame.contains("systemd"),
        "the first rows scrolled out of view: {frame}"
    );
}
