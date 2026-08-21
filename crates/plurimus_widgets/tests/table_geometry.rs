//! `TableGeometry::cell_rect`: where a `Table` says its cells are, and
//! whether a pointer agrees.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, In, On, ResMut};
use bevy_ecs::resource::Resource;
use plurimus_core::ratatui_core::layout::{Constraint, Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{click, composed_frame};
use plurimus_ui::{ScrollArea, ScrollOffset, UiArea};
use plurimus_widgets::{
    ActiveDescendant, TableColumns, TableGeometry, TablePosition, TableSelection, ValueChange,
    WidgetsPlugin, table, table_footer, table_header, table_row,
};

const AREA: Rect = Rect::new(0, 0, 20, 8);
const WIDTHS: [Constraint; 3] = [
    Constraint::Length(4),
    Constraint::Length(4),
    Constraint::Length(4),
];
const BODY: usize = 3;

struct Rows {
    header: Entity,
    body: Vec<Entity>,
    footer: Entity,
}

// The cursor symbol is left at its default, so a current row really does
// reserve the gutter the columns are pushed by.
fn app() -> (App, Entity, Rows) {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(AREA.width, AREA.height));
    app.insert_resource(Clicked(None));
    app.add_observer(
        |event: On<ValueChange<TablePosition>>, mut clicked: ResMut<Clicked>| {
            clicked.0 = Some(event.value);
        },
    );
    app.world_mut().spawn(TerminalCamera::default());

    let world = app.world_mut();
    let table = world
        .spawn((table(WIDTHS), TableSelection::Cell, UiArea::Fixed(AREA)))
        .id();
    let header = world
        .spawn((table_header(["ab", "cd", "ef"]), ChildOf(table)))
        .id();
    let body = (0..BODY)
        .map(|index| {
            let cells = ["r", "s", "t"].map(|cell| format!("{cell}{index}"));
            world.spawn((table_row(cells), ChildOf(table))).id()
        })
        .collect();
    let footer = world
        .spawn((table_footer(["gh", "ij", "kl"]), ChildOf(table)))
        .id();
    app.update();
    (
        app,
        table,
        Rows {
            header,
            body,
            footer,
        },
    )
}

fn cell_rect(app: &mut App, table: Entity, row: Entity, column: usize) -> Option<Rect> {
    app.world_mut()
        .run_system_cached_with(
            |In((table, row, column)): In<(Entity, Entity, usize)>,
             geometry: TableGeometry|
             -> Option<Rect> { geometry.cell_rect(table, row, column) },
            (table, row, column),
        )
        .expect("the geometry system runs")
}

// What the pointer resolves at a rect's own origin.
fn clicked(app: &mut App, at: Position) -> Option<TablePosition> {
    app.world_mut().resource_mut::<Clicked>().0 = None;
    click(app, at.x, at.y);
    app.world().resource::<Clicked>().0
}

#[derive(Resource)]
struct Clicked(Option<TablePosition>);

#[test]
fn a_cell_rect_names_the_column_it_was_drawn_in() {
    let (mut app, table, rows) = app();
    let frame = composed_frame(&app);
    let header = frame.lines().next().expect("the header band").to_owned();

    let second = cell_rect(&mut app, table, rows.header, 1).expect("the header's second cell");

    assert_eq!(
        usize::from(second.x),
        header.find("cd").expect("the second column's text"),
        "the rect starts where the column's text was drawn"
    );
    assert_eq!(
        (second.width, second.height),
        (4, 1),
        "as wide as its constraint and one row tall"
    );
}

#[test]
fn every_band_has_a_line_of_its_own() {
    let (mut app, table, rows) = app();

    let header = cell_rect(&mut app, table, rows.header, 0).expect("a header cell");
    let first = cell_rect(&mut app, table, rows.body[0], 0).expect("the first body cell");
    let last = cell_rect(&mut app, table, rows.body[BODY - 1], 0).expect("the last body cell");
    let footer = cell_rect(&mut app, table, rows.footer, 0).expect("a footer cell");

    assert_eq!(header.y, AREA.y, "the header is on the first line");
    assert_eq!(first.y, AREA.y + 1, "the body follows it");
    assert_eq!(
        last.y,
        AREA.y + u16::try_from(BODY).unwrap(),
        "in child order"
    );
    assert_eq!(
        footer.y,
        AREA.bottom() - 1,
        "and the footer is pinned to the bottom of the area, not to the end of the body"
    );
}

// The published rect and the click resolve against one column solve, so
// clicking a rect's own origin has to land in the cell it came from.
#[test]
fn clicking_a_rect_lands_in_the_cell_it_names() {
    let (mut app, table, rows) = app();
    let row = rows.body[1];
    let rect = cell_rect(&mut app, table, row, 2).expect("the third cell of the second row");

    let landed = clicked(&mut app, Position::new(rect.x, rect.y));

    assert_eq!(
        landed,
        Some(TablePosition {
            row: Some(row),
            column: Some(2),
        }),
        "the cell the rect named is the cell the pointer reached"
    );
}

// The cursor gutter shifts every column right while a row is current, and
// the rect has to move with it.
#[test]
fn the_cursor_gutter_moves_the_cells_it_pushes() {
    let (mut app, table, rows) = app();
    let row = rows.body[0];
    let before = cell_rect(&mut app, table, row, 0).expect("a cell with no cursor");

    app.world_mut()
        .entity_mut(table)
        .insert(ActiveDescendant(Some(row)));
    app.update();
    let after = cell_rect(&mut app, table, row, 0).expect("a cell with the cursor set");

    assert_eq!(
        after.x,
        before.x + 2,
        "the two-cell cursor symbol pushes the first column right"
    );
    assert_eq!(
        clicked(&mut app, Position::new(after.x, after.y)).and_then(|position| position.column),
        Some(0),
        "and a click on the shifted rect still lands in the first column"
    );
}

#[test]
fn a_scrolled_cell_moves_with_the_offset_and_clips_at_the_edge() {
    let (mut app, table, rows) = app();
    app.world_mut().entity_mut(table).insert((
        ScrollArea::new(Size::new(AREA.width, AREA.height)),
        ScrollOffset(Position::new(0, 2)),
    ));
    app.update();
    let row = rows.body[BODY - 1];

    let scrolled = cell_rect(&mut app, table, row, 0).expect("a row still in view");

    assert_eq!(
        scrolled.y,
        AREA.y + u16::try_from(BODY).unwrap() - 2,
        "two lines of scroll move the row two lines up"
    );
    assert_eq!(
        cell_rect(&mut app, table, rows.header, 0),
        None,
        "and the header has scrolled out of view rather than clamping to the top"
    );
}

// Content wider than the window it shows through: the row sync clamps that
// back on the next frame, but a rect answered in the frame that has it
// still may not name cells outside the table.
#[test]
fn a_cell_running_past_the_window_is_clipped_to_it() {
    let (mut app, table, rows) = app();
    app.world_mut().entity_mut(table).insert((
        TableColumns(vec![Constraint::Length(8); WIDTHS.len()]),
        ScrollArea::new(Size::new(40, AREA.height)),
    ));

    let clipped = cell_rect(&mut app, table, rows.body[0], 2).expect("the third cell");

    assert_eq!(
        clipped.x, 18,
        "the third column starts two cells from the edge"
    );
    assert_eq!(
        clipped.right(),
        AREA.right(),
        "and stops at it rather than naming the eight cells it was solved"
    );
}

#[test]
fn a_row_of_another_table_and_a_column_past_the_end_are_both_none() {
    let (mut app, table, rows) = app();
    let stranger = app.world_mut().spawn(table_row(["x"])).id();
    // A band row of some other table is the case a body row does not cover:
    // its line is its band's rather than a position among these children.
    let stray_header = app.world_mut().spawn(table_header(["x"])).id();

    assert_eq!(
        cell_rect(&mut app, table, stranger, 0),
        None,
        "a row that is not this table's is not somewhere in it"
    );
    assert_eq!(
        cell_rect(&mut app, table, stray_header, 0),
        None,
        "and neither is another table's header, which has a line of its own"
    );
    assert_eq!(
        cell_rect(&mut app, table, rows.body[0], WIDTHS.len()),
        None,
        "and neither is a column past the ones it has"
    );
    assert_eq!(
        cell_rect(&mut app, stranger, rows.body[0], 0),
        None,
        "nor is an entity that is not a table"
    );
}

// The body band is the area less its two bands, and the row after it would
// otherwise be named at the line the footer is drawn on - inside the area,
// so nothing else would refuse it.
#[test]
fn a_body_row_past_the_band_is_none() {
    let (mut app, table, _) = app();
    let band = usize::from(AREA.height) - 2;
    let crowded: Vec<Entity> = (BODY..=band)
        .map(|index| {
            let cells = ["x", "y", "z"].map(|cell| format!("{cell}{index}"));
            app.world_mut()
                .spawn((table_row(cells), ChildOf(table)))
                .id()
        })
        .collect();
    app.update();

    let last_fitting = crowded[crowded.len() - 2];
    let first_past = crowded[crowded.len() - 1];

    assert_eq!(
        cell_rect(&mut app, table, last_fitting, 0).map(|rect| rect.y),
        Some(AREA.bottom() - 2),
        "the last row the band has room for sits above the footer"
    );
    assert_eq!(
        cell_rect(&mut app, table, first_past, 0),
        None,
        "and the next one is not named at the footer's line"
    );
}

#[test]
fn a_click_agrees_with_every_cell_rect() {
    let (mut app, table, rows) = app();

    for (index, &row) in rows.body.iter().enumerate() {
        for column in 0..WIDTHS.len() {
            let rect = cell_rect(&mut app, table, row, column)
                .unwrap_or_else(|| panic!("row {index} column {column}"));
            assert_eq!(
                clicked(&mut app, Position::new(rect.x, rect.y)),
                Some(TablePosition {
                    row: Some(row),
                    column: Some(column),
                }),
                "row {index} column {column} at {rect:?}"
            );
        }
    }
}
