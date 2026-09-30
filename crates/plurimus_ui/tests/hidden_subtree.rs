//! `UiHidden` hides the subtree beneath it: drawing, scrolled content and
//! hit-testing all read `ComputedHidden`.

use bevy_app::{App, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{Commands, Res, Resource};
use plurimus_core::ratatui_core::layout::{Rect, Size};
use plurimus_core::{ComputedHidden, CorePlugin, TerminalCamera, TerminalSize, UiHidden};
use plurimus_test::composed_frame;
use plurimus_ui::tui_scrollview::ScrollbarVisibility;
use plurimus_ui::{ComputedWidgetArea, ScrollArea, UiArea, UiPlugin, UiWidget};
use ratatui_widgets::paragraph::Paragraph;

const LEFT: Rect = Rect::new(0, 0, 2, 1);
const RIGHT: Rect = Rect::new(2, 0, 2, 1);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(4, 1));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// A plain parent - no widget of its own - over a drawn child and a
/// scrolled one.
fn spawn_family(app: &mut App) -> (Entity, Entity) {
    let parent = app.world_mut().spawn_empty().id();
    let child = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("ab")),
            UiArea::Fixed(LEFT),
            ChildOf(parent),
        ))
        .id();
    let scrolled = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("cd")),
            UiArea::Fixed(RIGHT),
            ScrollArea::new(Size::new(2, 1)).with_scrollbars(ScrollbarVisibility::Never),
            ChildOf(child),
        ))
        .id();
    (parent, scrolled)
}

fn area_of(app: &App, entity: Entity) -> Rect {
    app.world().get::<ComputedWidgetArea>(entity).unwrap().0
}

#[test]
fn a_hidden_parent_hides_its_whole_subtree_until_shown() {
    let mut app = app();
    let (parent, scrolled) = spawn_family(&mut app);
    app.world_mut().entity_mut(parent).insert(UiHidden);

    app.update();

    assert_eq!(composed_frame(&app), "    ");
    assert_eq!(area_of(&app, scrolled), Rect::ZERO);
    assert!(app.world().get::<ComputedHidden>(scrolled).is_some());

    app.world_mut().entity_mut(parent).remove::<UiHidden>();
    app.update();

    assert_eq!(composed_frame(&app), "abcd");
    assert_eq!(area_of(&app, scrolled), RIGHT);
    assert!(app.world().get::<ComputedHidden>(scrolled).is_none());
}

#[derive(Resource)]
struct HideInUpdate(Entity);

// Hiding happens in observers and systems long after the frame's first
// resolve; the frame that hides must not draw what it hid.
#[test]
fn a_subtree_hidden_mid_frame_is_not_drawn_by_that_frame() {
    let mut app = app();
    let (parent, _) = spawn_family(&mut app);
    app.update();
    assert_eq!(composed_frame(&app), "abcd");

    app.insert_resource(HideInUpdate(parent));
    app.add_systems(Update, |hide: Res<HideInUpdate>, mut commands: Commands| {
        commands.entity(hide.0).insert(UiHidden);
    });
    app.update();

    assert_eq!(composed_frame(&app), "    ");
}
