//! The two row decorations a list draws that its rows cannot: the marker
//! gutter's second channel, and the trailing slot it right-aligns.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::composed_frame;
use plurimus_ui::{Checked, UiArea};
use plurimus_widgets::{
    ListBoxSelectionMarker, ListItemTrailing, Marked, WidgetsPlugin, list_item, listbox,
    listbox_self_update,
};

const WIDTH: u16 = 14;

fn app(width: u16) -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(width, 3));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_list(app: &mut App, width: u16, marker: bool) -> Entity {
    let mut list = app
        .world_mut()
        .spawn((listbox(), UiArea::Fixed(Rect::new(0, 0, width, 3))));
    if marker {
        list.insert(ListBoxSelectionMarker);
    }
    list.id()
}

fn frame(app: &mut App) -> String {
    app.update();
    composed_frame(app)
}

// `Checked` means the user selected it; a row can also be marked for a
// reason of the app's own, and saying that with `Checked` lets any app-wide
// selection updater redefine it.
#[test]
fn a_marked_row_lights_the_gutter_without_being_checked() {
    let mut app = app(WIDTH);
    let list = spawn_list(&mut app, WIDTH, true);
    app.world_mut().spawn((list_item("plain"), ChildOf(list)));
    let marked = app
        .world_mut()
        .spawn((list_item("lit"), Marked, ChildOf(list)))
        .id();

    let drawn = frame(&mut app);

    assert!(
        drawn.contains("▪ lit"),
        "marked rows light the gutter:\n{drawn}"
    );
    assert!(drawn.contains("  plain"), "unmarked rows do not:\n{drawn}");
    assert!(
        app.world().get::<Checked>(marked).is_none(),
        "and marking never made it selected"
    );
}

#[test]
fn a_checked_row_still_lights_the_gutter() {
    let mut app = app(WIDTH);
    let list = spawn_list(&mut app, WIDTH, true);
    app.world_mut()
        .spawn((list_item("sel"), Checked, ChildOf(list)));

    let drawn = frame(&mut app);

    assert!(drawn.contains("▪ sel"), "{drawn}");
}

// The stock selection updater owns `Checked` alone, which is the whole point
// of the second channel.
#[test]
fn the_selection_updater_leaves_marked_rows_alone() {
    let mut app = app(WIDTH);
    app.add_observer(listbox_self_update);
    let list = spawn_list(&mut app, WIDTH, true);
    let first = app
        .world_mut()
        .spawn((list_item("one"), Marked, ChildOf(list)))
        .id();
    let second = app
        .world_mut()
        .spawn((list_item("two"), ChildOf(list)))
        .id();
    app.update();

    app.world_mut()
        .trigger(plurimus_ui::ValueChange::new(list, second, true));
    app.update();

    assert!(
        app.world().get::<Marked>(first).is_some(),
        "selecting another row does not unmark this one"
    );
    assert!(app.world().get::<Checked>(second).is_some());
}

#[test]
fn a_trailing_slot_is_pushed_to_the_right_edge() {
    let mut app = app(WIDTH);
    let list = spawn_list(&mut app, WIDTH, false);
    app.world_mut().spawn((
        list_item("open"),
        ListItemTrailing("^O".into()),
        ChildOf(list),
    ));

    let drawn = frame(&mut app);

    let row = drawn.lines().next().unwrap_or_default().to_owned();
    assert!(row.starts_with("open"), "the label leads:\n{drawn}");
    assert!(
        row.trim_end().ends_with("^O"),
        "and the trailing slot sits at the far edge:\n{drawn}"
    );
    assert_eq!(row.trim_end().len(), usize::from(WIDTH), "{drawn}");
}

// The width is the list's own, so it is what a resize changes - and no row
// changed, which is why the width joins the redraw comparison.
#[test]
fn a_narrower_list_re_aligns_its_trailing_slots() {
    let mut app = app(WIDTH);
    let list = spawn_list(&mut app, WIDTH, false);
    app.world_mut().spawn((
        list_item("open"),
        ListItemTrailing("^O".into()),
        ChildOf(list),
    ));
    let wide = frame(&mut app)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();

    app.world_mut()
        .entity_mut(list)
        .insert(UiArea::Fixed(Rect::new(0, 0, 10, 3)));
    let narrow = frame(&mut app)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();

    assert_eq!(wide.trim_end().len(), usize::from(WIDTH));
    assert_eq!(narrow.trim_end().len(), 10, "re-aligned to the new width");
}

// Exactly wide enough for label, one space, and slot - the point at which
// right-aligning and keeping them apart are the same thing.
#[test]
fn a_list_too_narrow_for_both_keeps_them_apart() {
    let mut app = app(9);
    let list = spawn_list(&mut app, 9, false);
    app.world_mut().spawn((
        list_item("opened"),
        ListItemTrailing("^O".into()),
        ChildOf(list),
    ));

    let drawn = frame(&mut app);

    let row = drawn.lines().next().unwrap_or_default().to_owned();
    assert_eq!(row.trim_end(), "opened ^O", "{drawn}");
}

// A row losing a component reaches its container by no other route: no
// `Changed` filter fires for something that is gone.
#[test]
fn unmarking_a_row_puts_the_gutter_out() {
    let mut app = app(WIDTH);
    let list = spawn_list(&mut app, WIDTH, true);
    let row = app
        .world_mut()
        .spawn((list_item("lit"), Marked, ChildOf(list)))
        .id();
    assert!(frame(&mut app).contains("\u{25aa} lit"));

    app.world_mut().entity_mut(row).remove::<Marked>();

    let drawn = frame(&mut app);
    assert!(
        drawn.contains("  lit"),
        "the gutter goes dark again:\n{drawn}"
    );
}
