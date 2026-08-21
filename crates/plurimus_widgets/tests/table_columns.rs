//! An unstated `TableColumns`: who divides the width, and against which one.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use plurimus_core::ratatui_core::layout::{Constraint, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{click, composed_frame};
use plurimus_ui::{ScrollArea, UiArea};
use plurimus_widgets::{
    ActiveColumn, TableLayout, TableSelection, WidgetsPlugin, table, table_header, table_row,
};

const AREA: Rect = Rect::new(0, 0, 20, 6);
// Wider, not narrower: columns too wide for their area are compressed by
// the layout, which would hide a stale division behind the same drawing.
const WIDE: Rect = Rect::new(0, 0, 40, 6);
const COLUMNS: u16 = 4;
// More rows than the area has lines, so a scrolled table really scrolls and
// its content width is the window's less the scrollbar's column.
const BODY: usize = 8;
const SCROLLED_WIDTH: u16 = AREA.width - 1;

// Spacing would make the columns overflow and the layout compress them,
// which hides the width they were divided from.
fn app_with(area: Rect, widths: Vec<Constraint>, scrolled: bool) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(WIDE.width, WIDE.height));
    app.world_mut().spawn(TerminalCamera::default());

    let world = app.world_mut();
    let table = world
        .spawn((
            table(widths),
            TableLayout::default().with_column_spacing(0),
            TableSelection::Cell,
            UiArea::Fixed(area),
        ))
        .id();
    if scrolled {
        world
            .entity_mut(table)
            .insert(ScrollArea::new(Size::new(area.width, area.height)));
    }
    world.spawn((table_header(["ab", "cd", "ef", "gh"]), ChildOf(table)));
    for index in 0..BODY {
        let cells = ["r", "s", "t", "u"].map(|cell| format!("{cell}{index}"));
        world.spawn((table_row(cells), ChildOf(table)));
    }
    app.update();
    (app, table)
}

fn frame_of(area: Rect, widths: Vec<Constraint>, scrolled: bool) -> String {
    let (app, _) = app_with(area, widths, scrolled);
    composed_frame(&app)
}

fn equal(width: u16) -> Vec<Constraint> {
    vec![Constraint::Length(width / COLUMNS); COLUMNS as usize]
}

// The rule is this crate's now, so stating it must draw what leaving it
// unstated draws.
#[test]
fn an_unstated_column_set_divides_the_table_equally() {
    assert_eq!(
        frame_of(AREA, Vec::new(), false),
        frame_of(AREA, equal(AREA.width), false),
        "four columns over twenty cells are five each, said or unsaid"
    );
}

// The width a scrolled table is drawn into is its content buffer's, not the
// window's - the two differ by the scrollbar.
#[test]
fn a_scrolled_table_divides_its_content_width() {
    assert_eq!(
        frame_of(AREA, Vec::new(), true),
        frame_of(AREA, equal(SCROLLED_WIDTH), true),
        "nineteen cells of content divide into four each, not five"
    );
    assert_ne!(
        frame_of(AREA, Vec::new(), true),
        frame_of(AREA, equal(AREA.width), true),
        "and dividing the window width instead would move every column but the first"
    );
}

// One solve feeds the drawing and the click, so a click where a column was
// drawn reports that column.
#[test]
fn a_click_lands_in_the_column_it_was_drawn_in() {
    let (mut app, table) = app_with(AREA, Vec::new(), false);
    let frame = composed_frame(&app);
    let third = frame
        .lines()
        .next()
        .and_then(|header| header.find("ef"))
        .expect("the third column's header");

    click(&mut app, u16::try_from(third).unwrap(), 1);

    assert_eq!(
        app.world()
            .get::<ActiveColumn>(table)
            .map(|column| column.0),
        Some(Some(2)),
        "the column under the drawn header text is the column a click reports"
    );
}

// The division depends on the area, so the area has to be a redraw signal:
// nothing else about the table changes when it is resized.
#[test]
fn a_resize_redivides_an_unstated_column_set() {
    let (mut app, table) = app_with(AREA, Vec::new(), false);
    let before = composed_frame(&app);

    app.world_mut()
        .entity_mut(table)
        .insert(UiArea::Fixed(WIDE));
    app.update();

    assert_ne!(
        composed_frame(&app),
        before,
        "a wider table divides into wider columns"
    );
    assert_eq!(
        composed_frame(&app),
        frame_of(WIDE, Vec::new(), false),
        "and lands where the new width divides, not where the old one did"
    );
}
