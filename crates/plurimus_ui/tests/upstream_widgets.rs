//! `bevy_ui_widgets`' observers on a cell widget: what `UiPlugin` installs,
//! the gesture they see without a capture, and the disabled subtree written
//! down to where they look.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{click, move_to, press_at, release_at, send_focus};
use plurimus_ui::bevy_ui_widgets::Button;
use plurimus_ui::{
    Activate, ComputedDisabled, ComputedWidgetArea, Hovered, InteractionDisabled, Pressed, UiArea,
    UiPlugin,
};

const AREA: Rect = Rect::new(2, 1, 6, 1);

#[derive(Resource, Default)]
struct Activations(u32);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 4));
    app.init_resource::<Activations>();
    app.add_observer(|_: On<Activate>, mut seen: ResMut<Activations>| seen.0 += 1);
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_button(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Button,
            UiArea::Fixed(AREA),
            ComputedWidgetArea::default(),
            Hovered::default(),
        ))
        .id()
}

fn activations(app: &App) -> u32 {
    app.world().resource::<Activations>().0
}

fn is_flagged(app: &App, entity: Entity) -> bool {
    app.world().entity(entity).contains::<InteractionDisabled>()
}

#[test]
fn an_upstream_button_on_a_cell_activates_by_click() {
    let mut app = app();
    spawn_button(&mut app);
    app.update();

    click(&mut app, AREA.x + 1, AREA.y);

    assert_eq!(activations(&app), 1);
}

// Uncaptured, the release reaches what is under the pointer, so a press
// dragged off the button and let go elsewhere is no click on it.
#[test]
fn a_release_off_an_upstream_button_activates_nothing() {
    let mut app = app();
    let button = spawn_button(&mut app);
    app.update();

    press_at(&mut app, AREA.x + 1, AREA.y);
    assert!(app.world().entity(button).contains::<Pressed>());
    move_to(&mut app, 15, 3);
    release_at(&mut app, 15, 3);

    assert_eq!(activations(&app), 0);
    assert!(
        !app.world().entity(button).contains::<Pressed>(),
        "the gesture still ended"
    );
}

#[test]
fn a_disabled_container_flags_what_it_holds_until_it_is_enabled() {
    let mut app = app();
    let container = app.world_mut().spawn(InteractionDisabled).id();
    let button = spawn_button(&mut app);
    app.world_mut()
        .entity_mut(button)
        .insert(ChildOf(container));
    let own = app
        .world_mut()
        .spawn((InteractionDisabled, ChildOf(button)))
        .id();
    app.update();

    assert!(is_flagged(&app, button), "written down for upstream to see");
    click(&mut app, AREA.x + 1, AREA.y);
    assert_eq!(activations(&app), 0, "and upstream refuses the click");

    app.world_mut()
        .entity_mut(container)
        .remove::<InteractionDisabled>();
    app.update();

    assert!(!is_flagged(&app, button), "what was written is taken back");
    assert!(app.world().get::<ComputedDisabled>(button).is_none());
    assert!(is_flagged(&app, own), "a flag the app set is the app's");
    click(&mut app, AREA.x + 1, AREA.y);
    assert_eq!(activations(&app), 1);
}

// Nothing captured it, so the cancel a focus loss raises has to find the
// pressed widget another way.
#[test]
fn a_focus_loss_mid_press_releases_an_upstream_button() {
    let mut app = app();
    let button = spawn_button(&mut app);
    app.update();

    press_at(&mut app, AREA.x + 1, AREA.y);
    send_focus(&mut app, false);
    app.update();

    assert!(!app.world().entity(button).contains::<Pressed>());
}
