//! What a ListBox draws: gutters, per-row styles, and where focus lands.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::style::{Color, Style};
use plurimus_core::ratatui_core::text::{Line, Span};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::KeyCode;
use plurimus_test::{composed_frame, composed_styled_frame, press_key};
use plurimus_ui::UiArea;
use plurimus_widgets::{
    ActiveDescendant, ListBoxCursor, ListBoxSelectionMarker, UiLabel, UiStyle, WidgetsPlugin,
    list_item, listbox, listbox_self_update,
};

const TINT: Color = Color::Indexed(236);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 12, rows: 4 });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_listbox(app: &mut App) -> (Entity, [Entity; 3]) {
    let world = app.world_mut();
    let container = world
        .spawn((listbox(), UiArea::Fixed(Rect::new(0, 0, 12, 4))))
        .id();
    let items = [
        world.spawn((list_item("alpha"), ChildOf(container))).id(),
        world.spawn((list_item("beta"), ChildOf(container))).id(),
        world.spawn((list_item("gamma"), ChildOf(container))).id(),
    ];
    world
        .resource_mut::<InputFocus>()
        .set(container, FocusCause::Pressed);
    (container, items)
}

// One row is all a styling test needs; returns the list box.
fn spawn_row(app: &mut App, label: impl Into<Line<'static>>) -> Entity {
    let container = app
        .world_mut()
        .spawn((listbox(), UiArea::Fixed(Rect::new(0, 0, 12, 4))))
        .id();
    app.world_mut()
        .spawn((list_item(label), ChildOf(container)));
    container
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

// The shape an app builds columns from: one row, several styled spans.
fn columned_row(name: &str, date: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(name.to_owned(), Style::new().fg(Color::Green)),
        Span::raw(" "),
        Span::styled(date.to_owned(), Style::new().fg(Color::Blue)),
    ]
}

// Cells between the row's left edge and its text, which is what the
// marker column costs.
fn row_indent(marker: bool) -> usize {
    let mut app = app();
    let container = spawn_row(&mut app, "alpha");
    if marker {
        app.world_mut()
            .entity_mut(container)
            .insert(ListBoxSelectionMarker);
    }
    app.update();

    let frame = composed_frame(&app);
    let first = frame.lines().next().expect("a rendered row");
    first.len() - first.trim_start().len()
}

#[test]
fn the_selection_marker_column_is_opt_in() {
    assert_eq!(row_indent(false), 0, "a plain list starts at column zero");
    assert_eq!(row_indent(true), 2, "the marker column costs two cells");
}

// The cursor row as rendered, symbol column included.
fn cursor_row_text(symbol: Option<&'static str>) -> String {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    if let Some(symbol) = symbol {
        app.world_mut()
            .entity_mut(container)
            .insert(ListBoxCursor(Line::from(symbol)));
    }
    app.world_mut()
        .entity_mut(container)
        .insert(ActiveDescendant(Some(items[0])));
    app.update();

    composed_frame(&app)
        .lines()
        .next()
        .expect("the cursor row")
        .to_owned()
}

#[test]
fn the_cursor_symbol_is_replaceable() {
    assert!(
        cursor_row_text(None).starts_with("> alpha"),
        "the default symbol stays when nothing asks otherwise"
    );
    assert!(
        cursor_row_text(Some("▌ ")).starts_with("▌ alpha"),
        "a custom symbol renders and shifts the row by its width"
    );
    assert!(
        cursor_row_text(Some("")).starts_with("alpha"),
        "an empty symbol frees the gutter for bar-style selection"
    );
}

#[test]
fn focus_paints_the_cursor_row_and_leaves_the_others() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    app.update();
    let unfocused = style_grid(&app);

    app.world_mut()
        .entity_mut(container)
        .insert(ActiveDescendant(Some(items[1])));
    app.update();
    let focused = style_grid(&app);

    assert_eq!(
        unfocused[0], focused[0],
        "a row that is not the cursor keeps its style while the list is focused"
    );
    assert_eq!(
        unfocused[2], focused[2],
        "and so does every other non-cursor row"
    );
    assert_ne!(
        unfocused[1], focused[1],
        "the cursor row is the one that takes the focus style"
    );
}

// The style-grid row for the cursor row, whose first two cells are the
// `> ` gutter. Uniform means the row's style reached the gutter too.
fn cursor_row_styles(on_row: bool) -> String {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    let tint = Style::new().bg(TINT);
    if on_row {
        app.world_mut().entity_mut(items[0]).insert(UiStyle(tint));
    } else {
        app.world_mut()
            .entity_mut(items[0])
            .insert(UiLabel(Line::from("alpha").style(tint)));
    }
    app.world_mut()
        .entity_mut(container)
        .insert(ActiveDescendant(Some(items[0])));
    app.update();

    style_grid(&app).swap_remove(0)
}

fn is_uniform(row: &str) -> bool {
    let mut cells = row.chars();
    let Some(first) = cells.next() else {
        return true;
    };
    cells.all(|cell| cell == first)
}

#[test]
fn a_row_style_covers_the_cursor_gutter_where_a_line_style_does_not() {
    let on_row = cursor_row_styles(true);
    assert!(
        is_uniform(&on_row),
        "UiStyle paints the whole row including the gutter: {on_row}"
    );

    let on_label = cursor_row_styles(false);
    assert!(
        !is_uniform(&on_label),
        "a label's line style stops at the gutter: {on_label}"
    );
}

#[test]
fn a_row_keeps_a_style_per_span() {
    let mut app = app();
    spawn_row(&mut app, columned_row("ann", "may"));
    app.update();

    let frame = composed_styled_frame(&app);
    assert!(frame.contains("Green"), "the name column keeps its style");
    assert!(frame.contains("Blue"), "the date column keeps its own");
}

#[test]
fn a_row_keeps_its_line_style_through_the_marker_column() {
    let mut app = app();
    let container = spawn_row(&mut app, Line::from("striped").style(Style::new().bg(TINT)));
    app.world_mut()
        .entity_mut(container)
        .insert(ListBoxSelectionMarker);
    app.update();

    assert!(
        composed_styled_frame(&app).contains("Indexed(236)"),
        "decorating a label must not drop its line style"
    );
}

#[test]
fn listbox_renders_rows_and_highlight() {
    let mut app = app();
    app.add_observer(listbox_self_update);
    let (_, _) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Down);
    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}
