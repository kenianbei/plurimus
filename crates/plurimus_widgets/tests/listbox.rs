//! ListBox keyboard navigation and selection flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::ratatui_core::style::{Color, Style};
use plurimus_core::ratatui_core::text::{Line, Span};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{composed_frame, composed_styled_frame, press_key, send_mouse};
use plurimus_ui::{Checked, ScrollArea, ScrollOffset, UiArea, UiOrder, ValueChange};
use plurimus_widgets::{
    ActiveDescendant, ListBoxCursor, ListBoxMultiSelect, ListBoxSelectionMarker, UiLabel, UiStyle,
    WidgetsPlugin, button, list_item, listbox, listbox_self_update,
};

#[derive(Resource, Default)]
struct Selections(Vec<(Entity, Entity)>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 12, rows: 4 });
    app.init_resource::<Selections>();
    app.add_observer(
        |change: On<ValueChange<Entity>>, mut log: ResMut<Selections>| {
            log.0.push((change.source, change.value));
        },
    );
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

fn spawn_scrolling_listbox(app: &mut App) -> (Entity, Vec<Entity>) {
    let world = app.world_mut();
    let container = world
        .spawn((
            listbox(),
            UiArea::Fixed(Rect::new(0, 0, 12, 3)),
            ScrollArea::new(Size::new(1, 1)),
        ))
        .id();
    let items = (0..6)
        .map(|index| {
            world
                .spawn((list_item(format!("item {index}")), ChildOf(container)))
                .id()
        })
        .collect();
    world
        .resource_mut::<InputFocus>()
        .set(container, FocusCause::Pressed);
    (container, items)
}

fn active(app: &App, container: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(container).unwrap().0
}

#[test]
fn keyboard_moves_active_descendant() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Down);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, KeyCode::Down);
    assert_eq!(active(&app, container), Some(items[1]));
    press_key(&mut app, KeyCode::Up);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, KeyCode::Up);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, KeyCode::End);
    assert_eq!(active(&app, container), Some(items[2]));
    press_key(&mut app, KeyCode::Down);
    assert_eq!(active(&app, container), Some(items[2]));
    press_key(&mut app, KeyCode::Home);
    assert_eq!(active(&app, container), Some(items[0]));
}

#[test]
fn enter_selects_the_active_descendant() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Enter);
    assert!(app.world().resource::<Selections>().0.is_empty());

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[1])]
    );
}

#[test]
fn single_select_self_update_moves_checked() {
    let mut app = app();
    app.add_observer(listbox_self_update);
    let (_, items) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_some());

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_none());
    assert!(app.world().get::<Checked>(items[1]).is_some());
}

#[test]
fn multi_select_self_update_toggles_checked() {
    let mut app = app();
    app.add_observer(listbox_self_update);
    let (container, items) = spawn_listbox(&mut app);
    app.world_mut()
        .entity_mut(container)
        .insert(ListBoxMultiSelect);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_some());
    assert!(app.world().get::<Checked>(items[1]).is_some());

    press_key(&mut app, KeyCode::Up);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_none());
    assert!(app.world().get::<Checked>(items[1]).is_some());
}

#[test]
fn click_selects_the_clicked_row() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    send_mouse(&mut app, MouseKind::Moved, 2, 1);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);

    assert_eq!(active(&app, container), Some(items[1]));
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[1])]
    );
}

#[test]
fn press_on_an_overlay_does_not_reach_the_listbox_beneath() {
    let mut app = app();
    let (container, _) = spawn_listbox(&mut app);
    app.world_mut().spawn((
        button("ok"),
        UiArea::Fixed(Rect::new(0, 0, 12, 4)),
        UiOrder(1),
    ));

    send_mouse(&mut app, MouseKind::Moved, 2, 1);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);

    assert_eq!(active(&app, container), None);
    assert!(app.world().resource::<Selections>().0.is_empty());
}

#[test]
fn click_resolves_rows_through_scroll_offset() {
    let mut app = app();
    let (container, items) = spawn_scrolling_listbox(&mut app);
    app.update();

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 2)
    );

    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 0);
    assert_eq!(active(&app, container), Some(items[2]));
}

#[test]
fn keyboard_keeps_the_active_row_visible() {
    let mut app = app();
    let (container, items) = spawn_scrolling_listbox(&mut app);
    app.update();

    press_key(&mut app, KeyCode::End);
    assert_eq!(active(&app, container), Some(items[5]));
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 3)
    );

    press_key(&mut app, KeyCode::Home);
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 0)
    );
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
    let mut container = app
        .world_mut()
        .spawn((listbox(), UiArea::Fixed(Rect::new(0, 0, 12, 4))));
    if marker {
        container.insert(ListBoxSelectionMarker);
    }
    let container = container.id();
    app.world_mut()
        .spawn((list_item("alpha"), ChildOf(container)));
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
    let tint = Style::new().bg(Color::Indexed(236));
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

    let frame = composed_styled_frame(&app);
    let grid = frame.split("\n--\n").nth(1).expect("a style grid");
    grid.lines().next().expect("the cursor row").to_owned()
}

#[test]
fn a_row_style_covers_the_cursor_gutter_where_a_line_style_does_not() {
    let on_row = cursor_row_styles(true);
    assert!(
        on_row
            .chars()
            .all(|cell| cell == on_row.chars().next().unwrap()),
        "UiStyle paints the whole row including the gutter: {on_row}"
    );

    let on_label = cursor_row_styles(false);
    assert!(
        !on_label
            .chars()
            .all(|cell| cell == on_label.chars().next().unwrap()),
        "a label's line style stops at the gutter: {on_label}"
    );
}

#[test]
fn a_row_keeps_a_style_per_span() {
    let mut app = app();
    let container = app
        .world_mut()
        .spawn((listbox(), UiArea::Fixed(Rect::new(0, 0, 12, 4))))
        .id();
    app.world_mut()
        .spawn((list_item(columned_row("ann", "may")), ChildOf(container)));
    app.update();

    let frame = composed_styled_frame(&app);
    assert!(frame.contains("Green"), "the name column keeps its style");
    assert!(frame.contains("Blue"), "the date column keeps its own");
}

#[test]
fn a_row_keeps_its_line_style_through_the_marker_column() {
    let mut app = app();
    let container = app
        .world_mut()
        .spawn((listbox(), UiArea::Fixed(Rect::new(0, 0, 12, 4))))
        .id();
    app.world_mut().spawn((
        list_item(Line::from("striped").style(Style::new().bg(Color::Indexed(236)))),
        ChildOf(container),
    ));
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
