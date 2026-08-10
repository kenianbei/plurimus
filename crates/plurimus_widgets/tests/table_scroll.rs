//! A scrolled `Table`: content extent, windowing, and the offset the cursor
//! and the pointer both have to account for.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Constraint, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, MouseKind};
use plurimus_test::{click, composed_frame, press_key, send_mouse};
use plurimus_ui::{ScrollArea, ScrollOffset, UiArea};
use plurimus_widgets::{
    ActiveColumn, ActiveDescendant, TableCursor, TableSelection, WidgetsPlugin, table,
    table_footer, table_header, table_row,
};

const AREA: Rect = Rect::new(0, 0, 20, 6);
const BODY: usize = 8;

fn app() -> (App, Entity, Vec<Entity>) {
    app_with(
        [Constraint::Length(6), Constraint::Length(6)],
        TableSelection::Row,
        true,
    )
}

// Header, eight body rows, footer: ten lines of content in a six-line area,
// scrollable unless a test wants the truncating unscrolled layout.
fn app_with(
    widths: [Constraint; 2],
    selection: TableSelection,
    scrollable: bool,
) -> (App, Entity, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 20, rows: 6 });
    app.world_mut().spawn(TerminalCamera::default());

    let world = app.world_mut();
    let table = world
        .spawn((
            table(widths),
            selection,
            TableCursor("".into()),
            UiArea::Fixed(AREA),
        ))
        .id();
    if scrollable {
        world
            .entity_mut(table)
            .insert(ScrollArea::new(Size::new(20, 10)));
    }
    world.spawn((table_header(["name", "date"]), ChildOf(table)));
    let rows = (0..BODY)
        .map(|index| {
            world
                .spawn((
                    table_row([format!("row{index}"), "x".into()]),
                    ChildOf(table),
                ))
                .id()
        })
        .collect();
    world.spawn((table_footer(["total", "8"]), ChildOf(table)));
    world
        .resource_mut::<InputFocus>()
        .set(table, FocusCause::Pressed);
    app.update();
    (app, table, rows)
}

fn offset(app: &App, table: Entity) -> u16 {
    app.world()
        .get::<ScrollOffset>(table)
        .expect("an offset")
        .0
        .y
}

fn cursor(app: &App, table: Entity) -> Option<Entity> {
    app.world().entity(table).get::<ActiveDescendant>()?.0
}

#[test]
fn the_content_extent_counts_every_band() {
    let (app, table, _) = app();

    let scroll = app.world().get::<ScrollArea>(table).expect("a scroll area");
    assert_eq!(
        scroll.content_size.height,
        u16::try_from(BODY).unwrap() + 2,
        "the header and footer are content too, because a scroll area windows the widget whole"
    );
    assert_eq!(
        scroll.content_size.width, 19,
        "and the width leaves a column for the scrollbar"
    );
}

#[test]
fn scrolling_moves_the_header_out_of_view() {
    let (mut app, _, _) = app();
    assert!(
        composed_frame(&app)
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("name")),
        "the header is on top before anything scrolls"
    );

    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);

    let frame = composed_frame(&app);
    let first = frame.lines().next().expect("a first line");
    assert!(
        first.starts_with("row1"),
        "two ticks down puts the second body row on top: {first}"
    );
}

#[test]
fn the_cursor_stays_in_view_as_it_walks_past_the_bottom() {
    let (mut app, table, rows) = app();

    for _ in 0..BODY {
        press_key(&mut app, KeyCode::Down);
    }

    assert_eq!(
        cursor(&app, table),
        Some(rows[BODY - 1]),
        "the cursor reached the last row"
    );
    let scrolled = offset(&app, table);
    assert!(scrolled > 0, "and dragged the window down with it");

    let frame = composed_frame(&app);
    assert!(
        frame.contains(&format!("row{}", BODY - 1)),
        "so the row it is on is on screen: {frame}"
    );
}

#[test]
fn a_click_after_scrolling_picks_the_row_under_the_pointer() {
    let (mut app, table, rows) = app();
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);
    assert_eq!(offset(&app, table), 2, "two rows are scrolled off");

    click(&mut app, 2, 0);

    assert_eq!(
        cursor(&app, table),
        Some(rows[1]),
        "the top line is now the second body row, not the header"
    );
}

#[test]
fn a_click_past_the_last_row_of_a_scrolled_table_selects_nothing() {
    let (mut app, table, _) = app();
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);

    // Content lines 4..10 are showing: rows 3..8 then the footer.
    click(&mut app, 2, 5);

    assert_eq!(
        cursor(&app, table),
        None,
        "the last visible line is the footer, which is not a row"
    );
}

fn column(app: &App, table: Entity) -> Option<usize> {
    app.world().entity(table).get::<ActiveColumn>()?.0
}

// An unscrolled table truncates its body to fit between the bands, so rows
// past the fold are not on screen to be clicked - and the footer sits at a
// line one of them would otherwise answer for.
#[test]
fn an_unscrolled_table_ignores_a_click_on_the_footer_over_a_long_body() {
    let (mut app, table, _) = app_with(
        [Constraint::Length(6), Constraint::Length(6)],
        TableSelection::Row,
        false,
    );

    click(&mut app, 2, 5);

    assert_eq!(
        cursor(&app, table),
        None,
        "the fifth body row exists, but the footer is what is drawn there"
    );
}

// The mirror of the case above: a scrolled table's footer follows its rows
// rather than the area, so a line past the visible band is still a row.
#[test]
fn a_scrolled_table_selects_a_row_past_the_unscrolled_bands() {
    let (mut app, table, rows) = app();
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);
    send_mouse(&mut app, MouseKind::ScrollDown, 5, 3);

    click(&mut app, 2, 4);

    assert_eq!(
        cursor(&app, table),
        Some(rows[5]),
        "two lines scrolled off plus four down is the sixth body row"
    );
}

// Filling columns divide whatever width they are given, so where the last
// one ends is the difference between resolving against the area and against
// the content a scrollbar has narrowed.
#[test]
fn a_scrolled_table_resolves_columns_against_its_content_width() {
    let (mut app, table, _) = app_with(
        [Constraint::Fill(1), Constraint::Fill(1)],
        TableSelection::Cell,
        true,
    );

    click(&mut app, 19, 1);
    assert_eq!(
        column(&app, table),
        None,
        "the area's last cell is past the content, and belongs to no column"
    );

    click(&mut app, 18, 1);
    assert_eq!(
        column(&app, table),
        Some(1),
        "the content's last cell is the last column's"
    );
}
