use bevy_app::App;
use bevy_color::Color;
use bevy_ecs::prelude::{Has, Without};
use bevy_input_focus::{FocusCause, InputFocus};
use bevy_ui::{BackgroundColor, Node};
use plurimus::core::TerminalSize;
use plurimus::input::{InputCapabilities, KeyModifiers, ModifierKey, MouseKind};
use plurimus::ui::{ComputedWidgetArea, FocusWithin};
use plurimus::widgets::{Checkbox, MenuButton, Pane, SliderValue, TextInput, UiLabel};
use plurimus_test::{click, composed_frame, press_chord, press_key, press_key_with, send_mouse};

use super::*;

const THEMED_FOCUSABLES: usize = 12;

const TEST_SIZE: TerminalSize = TerminalSize {
    cols: 100,
    rows: 32,
};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(TEST_SIZE);
    add_demo(&mut app);
    // Twice: bui publishes interaction areas from the previous frame's
    // layout, so nothing on that side is clickable after one tick.
    app.update();
    app.update();
    app
}

fn focused_label(app: &mut App) -> Option<String> {
    let focused = app.world().resource::<InputFocus>().get()?;
    app.world()
        .get::<UiLabel>(focused)
        .map(|label| label.0.to_string())
}

fn is_pane_highlighted(app: &mut App, title: &str) -> bool {
    let world = app.world_mut();
    let mut panes = world.query_filtered::<(&UiLabel, Has<FocusWithin>), With<Pane>>();
    panes
        .iter(world)
        .find(|(label, _)| label.0.to_string() == title)
        .map(|(_, focused)| focused)
        .expect("pane exists")
}

#[test]
fn wraparound_edge_loops_focus_between_ends() {
    let mut app = headless_app();
    click(&mut app, 3, 7);
    assert_eq!(focused_label(&mut app).as_deref(), Some("pop"));

    press_key(&mut app, KeyCode::Down);
    let button = single::<(With<Button>, Without<MenuButton>, Without<Node>)>(&mut app);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));

    press_key(&mut app, KeyCode::Up);
    assert_eq!(focused_label(&mut app).as_deref(), Some("pop"));
}

#[test]
fn focus_highlights_the_pane_holding_it() {
    let mut app = headless_app();
    assert!(!is_pane_highlighted(&mut app, "controls"));

    click(&mut app, 3, 2);
    assert!(is_pane_highlighted(&mut app, "controls"));
}

fn themed_slider(app: &App) -> f32 {
    app.world().resource::<DemoState>().themed.slider
}

#[test]
fn clicking_the_track_seeks_and_arrows_step_the_slider() {
    let mut app = headless_app();
    click(&mut app, 3, 3);
    let sought = themed_slider(&app);
    assert!(sought < SLIDER_START, "{sought}");

    press_key(&mut app, KeyCode::Right);
    let stepped = themed_slider(&app);
    assert!(
        (stepped - sought - SLIDER_KEY_STEP).abs() < f32::EPSILON,
        "{sought} -> {stepped}"
    );
}

#[test]
fn menu_opens_over_widgets_and_reset_item_clears_state() {
    let mut app = headless_app();
    app.world_mut().resource_mut::<DemoState>().themed.presses = 3;
    assert!(!composed_frame(&app).contains("reset"));

    click(&mut app, 26, 0);
    app.update();
    let open = composed_frame(&app);
    assert!(open.contains("reset") && open.contains("quit"), "{open}");

    click(&mut app, 27, 2);
    app.update();
    assert_eq!(app.world().resource::<DemoState>().themed.presses, 0);
    assert!(!composed_frame(&app).contains("reset"));
}

#[test]
fn arrows_move_the_listbox_selection() {
    let mut app = headless_app();
    click(&mut app, 3, 10);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);

    let choice = app.world().resource::<DemoState>().themed.choice.clone();
    assert_eq!(choice, "beta");
    assert!(composed_frame(&app).contains("▪ beta"));
}

#[test]
fn typing_edits_the_single_line_field() {
    let mut app = headless_app();
    click(&mut app, 3, 16);
    press_key(&mut app, KeyCode::Char('!'));

    let frame = composed_frame(&app);
    assert!(frame.contains(&format!("{FIELD_TEXT}!")), "{frame}");
}

#[test]
fn typing_edits_the_multi_line_editor() {
    let mut app = headless_app();
    click(&mut app, 3, 18);
    press_key(&mut app, KeyCode::Char('z'));

    let frame = composed_frame(&app);
    assert!(frame.contains("za multi-line"), "{frame}");
}

#[test]
fn the_wheel_scrolls_the_log_and_moves_its_scrollbar() {
    let mut app = headless_app();
    let before = composed_frame(&app);
    assert!(before.contains("log entry 1"), "{before}");

    for _ in 0..4 {
        send_mouse(&mut app, MouseKind::ScrollDown, 10, 24);
    }

    let after = composed_frame(&app);
    assert!(!after.contains("log entry 1 "), "{after}");
    assert!(after.contains("log entry 6"), "{after}");
}

#[test]
fn reset_menu_item_restores_widget_visuals() {
    let mut app = headless_app();
    click(&mut app, 3, 4);
    click(&mut app, 3, 5);
    click(&mut app, 3, 3);
    let dirty = composed_frame(&app);
    assert!(dirty.contains("[x]") && dirty.contains("(•)"), "{dirty}");

    click(&mut app, 26, 0);
    app.update();
    click(&mut app, 27, 2);
    app.update();

    let reset = composed_frame(&app);
    assert!(!reset.contains("[x]") && !reset.contains("(•)"), "{reset}");
    let world = app.world_mut();
    let mut sliders = world.query::<&SliderValue>();
    assert!(
        sliders
            .iter(world)
            .all(|value| (value.0 - SLIDER_START).abs() < f32::EPSILON)
    );
}

#[test]
fn every_themed_widget_renders() {
    let app = headless_app();
    let frame = composed_frame(&app);
    let expected = [
        "controls",
        "[ press me ]",
        "[ ] enable tachyons",
        "( ) snap",
        "options",
        "alpha",
        "text",
        FIELD_TEXT,
        "a multi-line editor:",
        "log",
        "log entry 1",
    ];
    for fragment in expected {
        assert!(frame.contains(fragment), "{fragment} missing:\n{frame}");
    }
}

fn single<D: bevy_ecs::query::QueryFilter>(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, D>().single(world).unwrap()
}

fn click_center(app: &mut App, entity: Entity) {
    let area = app
        .world()
        .get::<ComputedWidgetArea>(entity)
        .expect("widget has a computed area")
        .0;
    click(app, area.x + area.width / 2, area.y + area.height / 2);
}

#[test]
fn clicking_a_bui_node_activates_the_widget_behind_it() {
    let mut app = headless_app();
    let button = single::<(With<Button>, With<Node>)>(&mut app);
    click_center(&mut app, button);

    assert_eq!(app.world().resource::<DemoState>().bui.presses, 1);
}

#[test]
fn typing_edits_the_bui_side_field() {
    let mut app = headless_app();
    let field = single::<(With<EditableText>, With<Node>)>(&mut app);
    click_center(&mut app, field);
    press_key(&mut app, KeyCode::Char('?'));

    let value = app
        .world()
        .get::<TextInput>(field)
        .unwrap()
        .value()
        .to_owned();
    assert_eq!(value, format!("{FIELD_TEXT}?"));
}

#[test]
fn tab_walks_from_the_themed_side_to_the_bui_side() {
    let mut app = headless_app();
    let themed_button = single::<(With<Button>, Without<MenuButton>, Without<Node>)>(&mut app);
    let bui_button = single::<(With<Button>, With<Node>)>(&mut app);
    click_center(&mut app, themed_button);

    for _ in 0..THEMED_FOCUSABLES {
        press_key(&mut app, KeyCode::Tab);
        if app.world().resource::<InputFocus>().get() == Some(bui_button) {
            return;
        }
    }
    panic!("tab never crossed to the bui side");
}

#[test]
fn both_sides_render_their_widgets() {
    let app = headless_app();
    let frame = composed_frame(&app);
    assert_eq!(frame.matches("enable tachyons").count(), 2, "{frame}");
    assert_eq!(frame.matches("press me").count(), 2, "{frame}");
    assert_eq!(frame.matches(FIELD_TEXT).count(), 2, "{frame}");
}

#[test]
fn typing_d_in_the_editor_leaves_widgets_enabled() {
    let mut app = headless_app();
    let button = single::<(With<Button>, Without<MenuButton>, Without<Node>)>(&mut app);
    click(&mut app, 3, 18);
    press_key(&mut app, KeyCode::Char('d'));

    assert!(app.world().get::<InteractionDisabled>(button).is_none());
    assert!(composed_frame(&app).contains("da multi-line"));
}

#[test]
fn the_menu_item_toggles_every_widget_disabled() {
    let mut app = headless_app();
    let button = single::<(With<Button>, Without<MenuButton>, Without<Node>)>(&mut app);
    let bui_button = single::<(With<Button>, With<Node>)>(&mut app);

    click(&mut app, 26, 0);
    app.update();
    assert!(composed_frame(&app).contains(themed::DISABLE_ITEM));
    click(&mut app, 27, 3);
    app.update();
    assert!(app.world().get::<InteractionDisabled>(button).is_some());
    assert!(app.world().get::<InteractionDisabled>(bui_button).is_some());

    click(&mut app, 26, 0);
    app.update();
    click(&mut app, 27, 3);
    app.update();
    assert!(app.world().get::<InteractionDisabled>(button).is_none());
    assert!(app.world().get::<InteractionDisabled>(bui_button).is_none());
}

#[test]
fn focus_lifts_the_background_of_bui_widgets() {
    let mut app = headless_app();
    let checkbox = single::<(With<Checkbox>, With<Node>)>(&mut app);
    assert_eq!(
        app.world().get::<BackgroundColor>(checkbox).unwrap().0,
        Color::NONE
    );

    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(checkbox, FocusCause::Navigated);
    app.update();

    let filled = app.world().get::<BackgroundColor>(checkbox).unwrap().0;
    assert_ne!(filled, Color::NONE, "focused bui widget has no fill");
}

// Two tiers, two encodings: a kitty terminal sends a real shift key
// before the tab, a legacy one folds shift into the message's modifiers
// (which is what crossterm's BackTab becomes).
#[test]
fn shift_tab_walks_focus_backwards_on_the_kitty_tier() {
    let mut app = headless_app();
    let button = single::<(With<Button>, Without<MenuButton>, Without<Node>)>(&mut app);
    click_center(&mut app, button);

    press_key(&mut app, KeyCode::Tab);
    let forward = app.world().resource::<InputFocus>().get();
    assert_ne!(forward, Some(button));

    press_chord(&mut app, ModifierKey::ShiftLeft, KeyCode::Tab);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
}

#[test]
fn shift_tab_walks_focus_backwards_on_the_legacy_tier() {
    let mut app = headless_app();
    app.insert_resource(InputCapabilities::none());
    let button = single::<(With<Button>, Without<MenuButton>, Without<Node>)>(&mut app);
    click_center(&mut app, button);

    press_key(&mut app, KeyCode::Tab);
    assert_ne!(app.world().resource::<InputFocus>().get(), Some(button));

    press_key_with(
        &mut app,
        KeyCode::Tab,
        KeyModifiers::default().with_shift(true),
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
}
