//! What a press on a disabled widget does: absorbs, unless the widget
//! opts out of hit-testing entirely.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiOrder, UiWidget};
use plurimus_test::click;
use plurimus_ui::{Hovered, InteractionDisabled, PointerPress, PressPassThrough, UiArea};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{MenuOpen, WidgetsPlugin, menu_button, menu_item, menu_popup};

const AREA: Rect = Rect::new(0, 2, 10, 3);

#[derive(Resource, Default)]
struct Presses(Vec<Entity>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Presses>();
    app.add_observer(|press: On<PointerPress>, mut log: ResMut<Presses>| log.0.push(press.entity));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_widget(app: &mut App, area: Rect) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("widget")),
            UiArea::Fixed(area),
            Hovered::default(),
        ))
        .id()
}

// A band above `beneath`, so the cover wins arbitration outright rather
// than by the entity tiebreak - the tests must prove absorption and
// transparency, not tie direction.
fn spawn_cover(app: &mut App, area: Rect) -> Entity {
    let cover = spawn_widget(app, area);
    app.world_mut().entity_mut(cover).insert(UiOrder(1));
    cover
}

fn was_pressed(app: &App, entity: Entity) -> bool {
    app.world().resource::<Presses>().0.contains(&entity)
}

// Two widgets on the same rect: the covering one disabled, the covered one
// live. Before the press knew "inert", the disabled one was invisible and
// the press went straight through it.
#[test]
fn a_press_on_a_disabled_widget_reaches_nothing_beneath() {
    let mut app = app();
    let beneath = spawn_widget(&mut app, AREA);
    let cover = spawn_cover(&mut app, AREA);
    app.world_mut()
        .entity_mut(cover)
        .insert(InteractionDisabled);
    app.update();

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert!(!was_pressed(&app, beneath), "the press was absorbed");
    assert!(
        !was_pressed(&app, cover),
        "and the disabled widget is inert"
    );
}

#[test]
fn a_press_on_a_disabled_widget_moves_no_focus() {
    let mut app = app();
    let cover = spawn_widget(&mut app, AREA);
    app.world_mut()
        .entity_mut(cover)
        .insert((InteractionDisabled, TabIndex(0)));
    let elsewhere = app.world_mut().spawn(()).id();
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(elsewhere, bevy_input_focus::FocusCause::Pressed);

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(elsewhere),
        "focus stays where it was"
    );
}

#[test]
fn pass_through_restores_the_fall_through() {
    let mut app = app();
    let beneath = spawn_widget(&mut app, AREA);
    let cover = spawn_cover(&mut app, AREA);
    app.world_mut()
        .entity_mut(cover)
        .insert((InteractionDisabled, PressPassThrough));
    app.update();

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert!(was_pressed(&app, beneath), "the press fell through");
    assert!(!was_pressed(&app, cover));
}

// The marker is not gated on being disabled: it is press transparency.
#[test]
fn an_enabled_pass_through_widget_is_invisible_to_the_press() {
    let mut app = app();
    let beneath = spawn_widget(&mut app, AREA);
    let cover = spawn_cover(&mut app, AREA);
    app.world_mut().entity_mut(cover).insert(PressPassThrough);
    app.update();

    click(&mut app, AREA.x + 1, AREA.y + 1);

    assert!(was_pressed(&app, beneath));
    assert!(!was_pressed(&app, cover));
}

// The guard's outside rule runs before target semantics: greyed chrome
// does not shield an open menu from dismissal.
#[test]
fn a_press_on_disabled_chrome_still_dismisses_the_menu() {
    let mut app = app();
    let button = app
        .world_mut()
        .spawn((menu_button("File"), UiArea::Fixed(Rect::new(11, 0, 8, 1))))
        .id();
    let popup = app
        .world_mut()
        .spawn((menu_popup(button), ChildOf(button)))
        .id();
    app.world_mut().spawn((menu_item("Open"), ChildOf(popup)));
    let chrome = spawn_widget(&mut app, Rect::new(0, 6, 6, 1));
    app.world_mut()
        .entity_mut(chrome)
        .insert(InteractionDisabled);
    app.update();
    click(&mut app, 12, 0);
    assert!(app.world().entity(popup).contains::<MenuOpen>());

    click(&mut app, 1, 6);

    assert!(
        !app.world().entity(popup).contains::<MenuOpen>(),
        "the outside press dismissed"
    );
    assert!(!was_pressed(&app, chrome), "and the chrome stayed inert");
}
