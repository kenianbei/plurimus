use plurimus::core::TerminalSize;
use plurimus_test::{click, composed_frame, press_key};

use super::*;

const TEST_SIZE: TerminalSize = TerminalSize::new(60, 14);

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(TEST_SIZE);
    add_demo(&mut app);
    app.update();
    app
}

fn focused_is<M: Component>(app: &mut App) -> bool {
    let focused = app.world().resource::<InputFocus>().get();
    let world = app.world_mut();
    let mut bars = world.query_filtered::<Entity, With<M>>();
    bars.single(world).ok() == focused
}

fn line(frame: &str, row: usize) -> String {
    frame.lines().nth(row).unwrap_or_default().to_owned()
}

#[test]
fn the_demo_opens_on_the_diary_under_a_joined_bar() {
    let app = headless_app();

    let frame = composed_frame(&app);
    assert!(line(&frame, 0).starts_with("╭───────╮╭──────╮╭───────╮"));
    assert!(line(&frame, 2).starts_with("╯       ╰┴──────┴┴───────┴"));
    assert!(line(&frame, 3).contains("Diary"));
    assert!(line(&frame, 4).contains("oats and coffee"));
}

#[test]
fn brackets_and_digits_switch_panels() {
    let mut app = headless_app();

    press_key(&mut app, KeyCode::Char(']'));
    app.update();
    assert!(line(&composed_frame(&app), 4).contains("mon fast"));

    press_key(&mut app, KeyCode::Char('3'));
    app.update();
    assert!(line(&composed_frame(&app), 4).contains("lentils 116"));

    press_key(&mut app, KeyCode::Home);
    app.update();
    assert!(line(&composed_frame(&app), 4).contains("oats and coffee"));
}

#[test]
fn clicking_a_tab_switches_its_panel() {
    let mut app = headless_app();

    click(&mut app, 20, 1);
    app.update();

    assert!(line(&composed_frame(&app), 4).contains("lentils 116"));
}

#[test]
fn settings_shows_the_looks_bar_and_picking_one_restyles_the_top_bar() {
    let mut app = headless_app();
    assert!(!composed_frame(&app).contains("joined"));

    press_key(&mut app, KeyCode::Char('4'));
    app.update();
    let frame = composed_frame(&app);
    assert!(line(&frame, 6).contains("joined"));
    assert!(line(&frame, 7).contains("boxed"));
    assert!(focused_is::<LooksBar>(&mut app));

    press_key(&mut app, KeyCode::Down);
    app.update();
    let frame = composed_frame(&app);
    assert!(line(&frame, 0).starts_with("┌───────┐┌──────┐"));
    assert!(line(&frame, 2).starts_with("└───────┘└──────┘"));

    press_key(&mut app, KeyCode::Tab);
    assert!(focused_is::<TopBar>(&mut app));
    press_key(&mut app, KeyCode::Char('1'));
    app.update();
    let frame = composed_frame(&app);
    assert!(!frame.contains("joined"));
    assert!(focused_is::<TopBar>(&mut app));
}
