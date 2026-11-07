//! Directional navigation tests: map building and arrow-key focus
//! movement, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Bundle, ChildOf};
use bevy_input::keyboard::Key;
use bevy_input_focus::InputFocus;
use bevy_input_focus::directional_navigation::DirectionalNavigationMap;
use bevy_input_focus::tab_navigation::{TabGroup, TabIndex};
use bevy_math::CompassOctant;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{press_key, set_focus};
use plurimus_ui::{InteractionDisabled, UiArea, UiWidget};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{SelectedTab, WidgetsPlugin, slider, tab_bar, tab_item};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(40, 12));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_focusable(app: &mut App, rect: Rect, extra: impl Bundle) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("w")),
            UiArea::Fixed(rect),
            extra,
        ))
        .id()
}

fn spawn_grouped_pair(app: &mut App) -> (Entity, Entity) {
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    let left = spawn_focusable(app, Rect::new(2, 1, 6, 1), (TabIndex(0), ChildOf(root)));
    let right = spawn_focusable(app, Rect::new(12, 1, 6, 1), (TabIndex(1), ChildOf(root)));
    (left, right)
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

fn neighbor(app: &App, from: Entity, octant: CompassOctant) -> Option<Entity> {
    app.world()
        .resource::<DirectionalNavigationMap>()
        .get_neighbor(from, octant)
        .get()
}

#[test]
fn grid_widgets_connect_along_rows_and_columns() {
    let mut app = app();
    let top_left = spawn_focusable(&mut app, Rect::new(2, 1, 6, 1), TabIndex(0));
    let top_right = spawn_focusable(&mut app, Rect::new(12, 1, 6, 1), TabIndex(0));
    let bottom_left = spawn_focusable(&mut app, Rect::new(2, 4, 6, 1), TabIndex(0));
    let bottom_right = spawn_focusable(&mut app, Rect::new(12, 4, 6, 1), TabIndex(0));
    app.update();

    assert_eq!(
        neighbor(&app, top_left, CompassOctant::East),
        Some(top_right)
    );
    assert_eq!(
        neighbor(&app, top_left, CompassOctant::South),
        Some(bottom_left)
    );
    assert_eq!(
        neighbor(&app, top_right, CompassOctant::West),
        Some(top_left)
    );
    assert_eq!(
        neighbor(&app, bottom_right, CompassOctant::North),
        Some(top_right)
    );
    assert_eq!(neighbor(&app, top_left, CompassOctant::West), None);
}

#[test]
fn enclosing_pane_drops_out_of_the_map() {
    let mut app = app();
    let pane = spawn_focusable(&mut app, Rect::new(0, 0, 30, 10), TabIndex(0));
    let upper = spawn_focusable(&mut app, Rect::new(2, 1, 6, 1), TabIndex(0));
    let lower = spawn_focusable(&mut app, Rect::new(2, 4, 6, 1), TabIndex(0));
    app.update();

    let map = app.world().resource::<DirectionalNavigationMap>();
    assert!(map.get_neighbors(pane).is_none());
    assert_eq!(neighbor(&app, upper, CompassOctant::South), Some(lower));
}

#[test]
fn disabled_widget_is_not_a_navigation_target() {
    let mut app = app();
    let left = spawn_focusable(&mut app, Rect::new(2, 1, 6, 1), TabIndex(0));
    let middle = spawn_focusable(&mut app, Rect::new(12, 1, 6, 1), TabIndex(0));
    let right = spawn_focusable(&mut app, Rect::new(22, 1, 6, 1), TabIndex(0));
    app.world_mut()
        .entity_mut(middle)
        .insert(InteractionDisabled);
    app.update();

    assert_eq!(neighbor(&app, left, CompassOctant::East), Some(right));
    assert!(
        app.world()
            .resource::<DirectionalNavigationMap>()
            .get_neighbors(middle)
            .is_none()
    );
}

#[test]
fn manual_edges_survive_geometry_rebuilds() {
    let mut app = app();
    let left = spawn_focusable(&mut app, Rect::new(2, 1, 6, 1), TabIndex(0));
    let right = spawn_focusable(&mut app, Rect::new(12, 1, 6, 1), TabIndex(0));
    let far = spawn_focusable(&mut app, Rect::new(22, 1, 6, 1), TabIndex(0));
    app.update();
    assert_eq!(neighbor(&app, left, CompassOctant::East), Some(right));
    assert_eq!(neighbor(&app, right, CompassOctant::East), Some(far));

    app.world_mut()
        .resource_mut::<DirectionalNavigationMap>()
        .add_edge(left, far, CompassOctant::East);
    app.update();
    assert_eq!(neighbor(&app, left, CompassOctant::East), Some(far));

    app.world_mut()
        .entity_mut(far)
        .insert(UiArea::Fixed(Rect::new(22, 4, 6, 1)));
    app.update();
    assert_eq!(
        neighbor(&app, left, CompassOctant::East),
        Some(far),
        "hand-made edge survives the rebuild"
    );
    assert_eq!(
        neighbor(&app, right, CompassOctant::East),
        None,
        "stale auto edge to the moved widget is reset"
    );
}

#[test]
fn blocked_manual_edge_survives_and_gates_navigation() {
    let mut app = app();
    let (left, right) = spawn_grouped_pair(&mut app);
    app.world_mut()
        .resource_mut::<DirectionalNavigationMap>()
        .block_edge(left, CompassOctant::East);
    app.update();

    set_focus(&mut app, left);
    press_key(&mut app, Key::ArrowRight);
    assert_eq!(focused(&app), Some(left), "blocked edge gates navigation");
    assert_eq!(neighbor(&app, right, CompassOctant::West), Some(left));
}

#[test]
fn arrows_move_focus_between_widgets() {
    let mut app = app();
    let (left, right) = spawn_grouped_pair(&mut app);
    app.update();

    press_key(&mut app, Key::ArrowRight);
    assert_eq!(focused(&app), Some(left));

    press_key(&mut app, Key::ArrowRight);
    assert_eq!(focused(&app), Some(right));

    press_key(&mut app, Key::ArrowLeft);
    assert_eq!(focused(&app), Some(left));

    press_key(&mut app, Key::ArrowDown);
    assert_eq!(focused(&app), Some(left));
}

#[test]
fn first_arrow_focuses_lowest_tab_index() {
    let mut app = app();
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    spawn_focusable(
        &mut app,
        Rect::new(2, 1, 6, 1),
        (TabIndex(1), ChildOf(root)),
    );
    let lowest = spawn_focusable(
        &mut app,
        Rect::new(12, 1, 6, 1),
        (TabIndex(0), ChildOf(root)),
    );
    app.update();

    press_key(&mut app, Key::ArrowUp);
    assert_eq!(focused(&app), Some(lowest));
}

#[test]
fn focused_slider_consumes_horizontal_arrows() {
    let mut app = app();
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    let track = app
        .world_mut()
        .spawn((
            slider(0.0, 1.0, 0.5),
            UiArea::Fixed(Rect::new(2, 1, 8, 1)),
            ChildOf(root),
        ))
        .id();
    spawn_focusable(
        &mut app,
        Rect::new(14, 1, 6, 1),
        (TabIndex(1), ChildOf(root)),
    );
    app.update();

    set_focus(&mut app, track);
    press_key(&mut app, Key::ArrowRight);
    assert_eq!(focused(&app), Some(track));
}

// Upstream's slider consumes a side arrow even at its bound, and binds
// nothing on the other axis.
#[test]
fn a_slider_keeps_left_at_its_minimum_and_passes_down_on() {
    let mut app = app();
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    let track = app
        .world_mut()
        .spawn((
            slider(0.0, 1.0, 0.0),
            UiArea::Fixed(Rect::new(2, 1, 8, 1)),
            ChildOf(root),
        ))
        .id();
    let below = spawn_focusable(
        &mut app,
        Rect::new(2, 3, 8, 1),
        (TabIndex(1), ChildOf(root)),
    );
    app.update();

    set_focus(&mut app, track);
    press_key(&mut app, Key::ArrowLeft);
    assert_eq!(focused(&app), Some(track));
    press_key(&mut app, Key::ArrowDown);
    assert_eq!(focused(&app), Some(below));
}

#[test]
fn modal_group_traps_arrows() {
    let mut app = app();
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    let outside = spawn_focusable(
        &mut app,
        Rect::new(2, 1, 6, 1),
        (TabIndex(0), ChildOf(root)),
    );
    let modal = app.world_mut().spawn(TabGroup::modal()).id();
    let inside = spawn_focusable(
        &mut app,
        Rect::new(12, 1, 6, 1),
        (TabIndex(0), ChildOf(modal)),
    );
    let inside_east = spawn_focusable(
        &mut app,
        Rect::new(22, 1, 6, 1),
        (TabIndex(1), ChildOf(modal)),
    );
    app.update();

    set_focus(&mut app, inside);
    press_key(&mut app, Key::ArrowLeft);
    assert_eq!(focused(&app), Some(inside));

    press_key(&mut app, Key::ArrowRight);
    assert_eq!(focused(&app), Some(inside_east));

    set_focus(&mut app, outside);
    press_key(&mut app, Key::ArrowRight);
    assert_eq!(focused(&app), Some(inside));
}

// A roving strip is one stop: the tabs it is not resting on carry a
// negative index, which the map leaves out the way Tab does, so Up reaches
// the selected tab and never the one beside it.
#[test]
fn a_tab_strip_is_one_arrow_target_at_its_roving_tab() {
    let mut app = app();
    let root = app.world_mut().spawn(TabGroup::new(0)).id();
    let bar = app
        .world_mut()
        .spawn((
            tab_bar(),
            UiArea::Fixed(Rect::new(0, 1, 20, 1)),
            ChildOf(root),
        ))
        .id();
    let resting = app.world_mut().spawn((tab_item("a"), ChildOf(bar))).id();
    let selected = app.world_mut().spawn((tab_item("b"), ChildOf(bar))).id();
    app.world_mut()
        .entity_mut(bar)
        .insert(SelectedTab(Some(selected)));
    let under_resting = spawn_focusable(
        &mut app,
        Rect::new(0, 4, 3, 1),
        (TabIndex(0), ChildOf(root)),
    );
    let under_selected = spawn_focusable(
        &mut app,
        Rect::new(3, 4, 3, 1),
        (TabIndex(1), ChildOf(root)),
    );
    app.update();
    app.update();

    set_focus(&mut app, under_resting);
    press_key(&mut app, Key::ArrowUp);
    assert_ne!(focused(&app), Some(resting));

    set_focus(&mut app, under_selected);
    press_key(&mut app, Key::ArrowUp);
    assert_eq!(focused(&app), Some(selected));
}
