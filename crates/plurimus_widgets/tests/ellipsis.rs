//! Text too wide for its widget, under the theme's `TextOverflow`: each
//! stock cell widget ends a cut label in the ellipsis within its rect.

use bevy_app::App;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::ChildOf;
use plurimus_core::ratatui_core::layout::{Constraint, Rect};
use plurimus_core::ratatui_core::text::{Line, Text};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::composed_frame;
use plurimus_ui::{TextOverflow, UiArea, UiTheme};
use plurimus_widgets::ratatui_widgets::borders::BorderType;
use plurimus_widgets::{
    ControlOrientation, ListItemText, ListItemTrailing, TabBarLook, TabList, WidgetsPlugin, button,
    checkbox, list_item, listbox, pane, tab_bar, tab_item, table, table_header, table_row,
};

const ELLIPSIS: TextOverflow = TextOverflow::Ellipsis("…");

fn app(cols: u16, rows: u16, overflow: TextOverflow) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(cols, rows));
    app.insert_resource(UiTheme::new().with_overflow(overflow));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_at(app: &mut App, rect: Rect, widget: impl Bundle) -> bevy_ecs::entity::Entity {
    app.world_mut().spawn((widget, UiArea::Fixed(rect))).id()
}

fn rows(app: &mut App) -> Vec<String> {
    app.update();
    app.update();
    composed_frame(app).lines().map(str::to_owned).collect()
}

#[test]
fn a_cut_button_keeps_its_brackets() {
    let mut app = app(10, 1, ELLIPSIS);
    spawn_at(&mut app, Rect::new(0, 0, 10, 1), button("漢字漢字漢字"));
    // A wide glyph's second cell reads as a space in the frame string.
    assert_eq!(rows(&mut app), [" [ 漢 字 … ]"]);
}

#[test]
fn the_default_theme_still_clips() {
    let mut app = app(10, 1, UiTheme::new().overflow);
    spawn_at(&mut app, Rect::new(0, 0, 10, 1), button("a long label"));
    assert_eq!(rows(&mut app), ["[ a long l"]);
}

#[test]
fn a_cut_checkbox_keeps_its_mark() {
    let mut app = app(10, 1, ELLIPSIS);
    spawn_at(&mut app, Rect::new(0, 0, 10, 1), checkbox("a long label"));
    assert_eq!(rows(&mut app), ["[ ] a lon…"]);
}

#[test]
fn a_list_row_cuts_its_label_and_keeps_its_trailing_text() {
    let mut app = app(12, 1, ELLIPSIS);
    let list = spawn_at(&mut app, Rect::new(0, 0, 12, 1), listbox());
    app.world_mut().spawn((
        list_item("a very long label"),
        ListItemTrailing(Line::from("42")),
        ChildOf(list),
    ));
    assert_eq!(rows(&mut app), ["a very l… 42"]);
}

#[test]
fn every_line_of_a_multi_line_row_is_cut() {
    let mut app = app(8, 2, ELLIPSIS);
    let list = spawn_at(&mut app, Rect::new(0, 0, 8, 2), listbox());
    app.world_mut().spawn((
        list_item("unused"),
        ListItemText(Text::from(vec![
            Line::from("first line"),
            Line::from("second line"),
        ])),
        ChildOf(list),
    ));
    assert_eq!(rows(&mut app), ["first l…", "second …"]);
}

#[test]
fn table_cells_are_cut_to_their_columns() {
    let mut app = app(11, 2, ELLIPSIS);
    let widths = [Constraint::Length(5), Constraint::Length(5)];
    let grid = spawn_at(&mut app, Rect::new(0, 0, 11, 2), table(widths));
    let world = app.world_mut();
    world.spawn((table_header(["Longname", "b"]), ChildOf(grid)));
    world.spawn((table_row(["abcdefgh", "xy"]), ChildOf(grid)));
    assert_eq!(rows(&mut app), ["Long… b    ", "abcd… xy   "]);
}

#[test]
fn a_pane_title_is_cut_between_its_borders() {
    let mut app = app(10, 3, ELLIPSIS);
    spawn_at(&mut app, Rect::new(0, 0, 10, 3), pane("A long pane title"));
    assert_eq!(rows(&mut app)[0], "┌A long …┐");
}

#[test]
fn a_boxed_vertical_tab_is_cut_inside_its_box() {
    let mut app = app(8, 3, ELLIPSIS);
    let list = TabList {
        orientation: ControlOrientation::Vertical,
        ..TabList::default()
    };
    let look = TabBarLook::default().with_border(Some(BorderType::Plain));
    let bar = spawn_at(&mut app, Rect::new(0, 0, 8, 3), (tab_bar(), list, look));
    app.world_mut()
        .spawn((tab_item("Planning ahead"), ChildOf(bar)));
    assert_eq!(rows(&mut app)[1], "│ Pla… │");
}

#[test]
fn an_unboxed_tab_is_cut_inside_its_padding() {
    let mut app = app(8, 1, ELLIPSIS);
    let list = TabList {
        orientation: ControlOrientation::Vertical,
        ..TabList::default()
    };
    let look = TabBarLook::default().with_padding(1);
    let bar = spawn_at(&mut app, Rect::new(0, 0, 8, 1), (tab_bar(), list, look));
    app.world_mut()
        .spawn((tab_item("Planning ahead"), ChildOf(bar)));
    assert_eq!(rows(&mut app), [" Plann… "]);
}
