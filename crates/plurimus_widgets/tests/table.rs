//! What a `Table` draws, and what makes it redraw.

use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use plurimus_core::ratatui_core::layout::{Constraint, Rect};
use plurimus_core::ratatui_core::style::{Color, Style};
use plurimus_core::ratatui_core::text::Line;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiWidget};
use plurimus_test::{composed_frame, composed_styled_frame, widget_content};
use plurimus_ui::{Checked, InteractionDisabled, UiArea};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{
    ActiveColumn, ActiveDescendant, StylistDisabled, Table, TableCheckedStyle, TableColumns,
    TableCursor, TableFooter, TableHeader, TableLayout, TableRow, TableSelection, TableStripe,
    UiStyle, UiTheme, WidgetsPlugin, table, table_footer, table_header, table_row,
};

const TINT: Color = Color::Indexed(236);
const AREA: Rect = Rect::new(0, 0, 20, 6);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 20, rows: 6 });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

const fn widths() -> [Constraint; 2] {
    [Constraint::Length(6), Constraint::Length(6)]
}

fn spawn_table(app: &mut App) -> (Entity, [Entity; 3]) {
    let world = app.world_mut();
    let table = world.spawn((table(widths()), UiArea::Fixed(AREA))).id();
    world.spawn((table_header(["name", "date"]), ChildOf(table)));
    let rows = [
        world
            .spawn((table_row(["ann", "may"]), ChildOf(table)))
            .id(),
        world.spawn((table_row(["bo", "jun"]), ChildOf(table))).id(),
        world.spawn((table_row(["cy", "jul"]), ChildOf(table))).id(),
    ];
    world.spawn((table_footer(["total", "3"]), ChildOf(table)));
    (table, rows)
}

// The style grid, one letter per cell, one line per row.
fn style_grid(app: &App) -> Vec<String> {
    let frame = composed_styled_frame(app);
    let grid = frame
        .split("\n--\n")
        .nth(1)
        .expect("a style grid")
        .to_owned();
    grid.lines()
        .take_while(|line| !line.contains(": fg:"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_table_draws_its_bands_and_columns() {
    let mut app = app();
    spawn_table(&mut app);
    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

#[test]
fn columns_land_where_their_constraints_put_them() {
    let mut app = app();
    spawn_table(&mut app);
    app.update();

    let frame = composed_frame(&app);
    let header = frame.lines().next().expect("the header band");
    assert!(header.starts_with("name"), "the first column starts flush");
    assert_eq!(
        header.find("date"),
        Some(7),
        "the second starts past six cells of column and one of spacing"
    );
}

#[test]
fn a_wider_column_spacing_moves_the_second_column() {
    let mut app = app();
    let (table, _) = spawn_table(&mut app);
    app.world_mut().entity_mut(table).insert(TableLayout {
        column_spacing: 3,
        ..TableLayout::default()
    });
    app.update();

    let frame = composed_frame(&app);
    let header = frame.lines().next().expect("the header band");
    assert_eq!(
        header.find("date"),
        Some(9),
        "three cells of spacing rather than one"
    );
}

#[test]
fn striping_bands_the_body_and_leaves_the_header_and_footer() {
    let mut app = app();
    let (table, _) = spawn_table(&mut app);
    app.world_mut()
        .entity_mut(table)
        .insert(TableStripe(Style::new().bg(TINT)));
    app.update();

    let grid = style_grid(&app);
    assert_eq!(grid[1], grid[3], "the first and third body rows match");
    assert_ne!(grid[1], grid[2], "and the second is banded differently");
    assert_eq!(
        grid[0], grid[1],
        "the header is outside the striping, like the first body row"
    );
    assert_eq!(grid[0], grid[5], "and so is the footer");
}

#[test]
fn a_row_style_patches_over_the_stripe() {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app);
    let world = app.world_mut();
    world
        .entity_mut(table)
        .insert(TableStripe(Style::new().bg(TINT)));
    world
        .entity_mut(rows[1])
        .insert(UiStyle(Style::new().fg(Color::Green)));
    app.update();

    let frame = composed_styled_frame(&app);
    assert!(
        frame.contains("Indexed(236)"),
        "the banded row keeps its background"
    );
    assert!(frame.contains("Green"), "under the color the app chose");
}

#[test]
fn an_empty_column_set_divides_the_width_equally() {
    let mut app = app();
    let world = app.world_mut();
    let table = world
        .spawn((table(Vec::<Constraint>::new()), UiArea::Fixed(AREA)))
        .id();
    world.spawn((table_row(["ann", "may"]), ChildOf(table)));
    app.update();

    let frame = composed_frame(&app);
    let row = frame.lines().next().expect("a rendered row");
    assert_eq!(
        row.find("may"),
        Some(11),
        "ratatui's fallback gives each of two columns half the width, plus the spacing"
    );
}

#[test]
fn a_table_without_columns_keeps_the_widget_it_carries() {
    let mut app = app();
    let world = app.world_mut();
    let table = world
        .spawn((
            Table,
            UiWidget::new(Paragraph::new("untouched")),
            UiArea::Fixed(AREA),
        ))
        .id();
    world.spawn((table_row(["ann", "may"]), ChildOf(table)));
    app.update();

    assert!(
        composed_frame(&app).contains("untouched"),
        "the stylist skips a table that has not said how wide its columns are"
    );
}

#[test]
fn stylist_disabled_leaves_the_widget_to_the_app() {
    let mut app = app();
    let (table, _) = spawn_table(&mut app);
    app.world_mut().entity_mut(table).insert(StylistDisabled);
    app.update();

    assert!(
        composed_frame(&app).trim().is_empty(),
        "the placeholder stands until the app replaces it"
    );
}

// Whether the stylist rebuilt the widget in response to `change`, with the
// table already carrying whatever `setup` left on it.
fn rebuilds_after_setup(
    setup: impl FnOnce(&mut App, Entity, [Entity; 3]),
    change: impl FnOnce(&mut App, Entity, [Entity; 3]),
) -> bool {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app);
    setup(&mut app, table, rows);
    app.update();
    let before = widget_content(&app, table);

    change(&mut app, table, rows);
    app.update();
    !Arc::ptr_eq(&before, &widget_content(&app, table))
}

// Whether the stylist rebuilt the widget in response to `change`.
fn rebuilds_after(change: impl FnOnce(&mut App, Entity, [Entity; 3])) -> bool {
    rebuilds_after_setup(|_, _, _| {}, change)
}

#[test]
fn a_row_edit_reaches_the_stylist() {
    assert!(
        rebuilds_after(|app, _, rows| {
            app.world_mut()
                .entity_mut(rows[0])
                .insert(TableRow(vec![Line::from("edited")]));
        }),
        "a row's cells"
    );
    assert!(
        rebuilds_after(|app, _, rows| {
            app.world_mut()
                .entity_mut(rows[0])
                .insert(UiStyle(Style::new().fg(Color::Green)));
        }),
        "a row's style override"
    );
    assert!(
        rebuilds_after(|app, _, rows| {
            app.world_mut().entity_mut(rows[0]).insert(TableHeader);
        }),
        "a row becoming the header band"
    );
    assert!(
        rebuilds_after(|app, _, rows| {
            app.world_mut().entity_mut(rows[0]).insert(TableFooter);
        }),
        "a row becoming the footer band"
    );
    assert!(
        rebuilds_after(|app, _, rows| {
            app.world_mut().entity_mut(rows[0]).insert(Checked);
        }),
        "a row being checked"
    );
}

// `Changed` never fires for a component that goes, so a removal reaches the
// stylist only through the forwarder's `RemovedComponents` readers.
#[test]
fn a_removed_row_component_reaches_the_stylist() {
    assert!(
        rebuilds_after_setup(
            |app, _, rows| {
                app.world_mut().entity_mut(rows[0]).insert(Checked);
            },
            |app, _, rows| {
                app.world_mut().entity_mut(rows[0]).remove::<Checked>();
            },
        ),
        "a row unchecked in place, the cursor never moving"
    );
    assert!(
        rebuilds_after_setup(
            |app, _, rows| {
                app.world_mut()
                    .entity_mut(rows[0])
                    .insert(UiStyle(Style::new().fg(Color::Green)));
            },
            |app, _, rows| {
                app.world_mut().entity_mut(rows[0]).remove::<UiStyle>();
            },
        ),
        "a row's style override cleared by removing it"
    );
}

#[test]
fn a_table_edit_reaches_the_stylist() {
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .spawn((table_row(["dee", "aug"]), ChildOf(table)));
        }),
        "a row appearing"
    );
    // The one case only the table's own `Children` can report: the row that
    // would have reported it is gone.
    assert!(
        rebuilds_after(|app, _, rows| {
            app.world_mut().entity_mut(rows[0]).despawn();
        }),
        "a row disappearing"
    );
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .entity_mut(table)
                .insert(TableColumns(vec![Constraint::Length(3)]));
        }),
        "the column constraints"
    );
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .entity_mut(table)
                .insert(TableStripe(Style::new().bg(TINT)));
        }),
        "the stripe"
    );
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut().entity_mut(table).insert(TableLayout {
                column_spacing: 3,
                ..TableLayout::default()
            });
        }),
        "the column spacing"
    );
}

#[test]
fn a_cursor_change_reaches_the_stylist() {
    assert!(
        rebuilds_after(|app, table, rows| {
            app.world_mut()
                .entity_mut(table)
                .insert(ActiveDescendant(Some(rows[1])));
        }),
        "the cursor row"
    );
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .entity_mut(table)
                .insert(ActiveColumn(Some(1)));
        }),
        "the cursor column"
    );
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .entity_mut(table)
                .insert(TableCursor(Line::from("* ")));
        }),
        "the cursor symbol"
    );
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .entity_mut(table)
                .insert(TableCheckedStyle(Style::new().bg(TINT)));
        }),
        "the checked style"
    );
}

// Hover is deliberately absent: a table with no cursor row resolves its
// style through `resting_style`, which sets hover, press and focus aside,
// so hovering one changes nothing it draws.
#[test]
fn a_state_change_reaches_the_stylist() {
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .entity_mut(table)
                .insert(InteractionDisabled);
        }),
        "the table being disabled"
    );
    assert!(
        rebuilds_after(|app, table, _| {
            app.world_mut()
                .entity_mut(table)
                .insert(UiStyle(Style::new().fg(Color::Green)));
        }),
        "the table's own style override"
    );
    assert!(
        rebuilds_after(|app, _, _| {
            app.world_mut().resource_mut::<UiTheme>().normal = Style::new().fg(Color::Red);
        }),
        "the theme"
    );
}

// Adding `TableSelection` brings its required components with it, so only a
// mode that changes on a table already carrying one tests the mode itself.
#[test]
fn a_changed_selection_mode_reaches_the_stylist() {
    let mut app = app();
    let (table, _) = spawn_table(&mut app);
    app.world_mut()
        .entity_mut(table)
        .insert(TableSelection::Row);
    app.update();
    let before = widget_content(&app, table);

    app.world_mut()
        .entity_mut(table)
        .insert(TableSelection::Cell);
    app.update();

    assert!(
        !Arc::ptr_eq(&before, &widget_content(&app, table)),
        "switching from row to cell selection repaints the highlight"
    );
}

#[test]
fn an_idle_frame_rebuilds_nothing() {
    assert!(
        !rebuilds_after(|app, _, _| app.update()),
        "a settled table is left alone frame after frame"
    );
}
