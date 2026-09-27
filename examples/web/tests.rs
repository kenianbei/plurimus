use bevy_app::{App, AppExit};
use bevy_ecs::message::{MessageCursor, Messages};
use plurimus::core::{CorePlugin, TerminalSize};
use plurimus::term::TerminalRequest;
use plurimus_test::{click, composed_frame};

use super::demo::{COLUMNS, DemoState, FRUITS, LIST, QUIT_BUTTON, ROWS, TITLE_BUTTON, add_demo};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(TerminalSize::new(COLUMNS, ROWS));
    add_demo(&mut app);
    app.update();
    app
}

#[test]
fn clicking_a_fruit_picks_it() {
    let mut app = headless_app();

    click(&mut app, LIST.x + 1, LIST.y + 1);
    app.update();

    assert_eq!(
        app.world().resource::<DemoState>().fruit.as_deref(),
        Some(FRUITS[1])
    );
    assert!(composed_frame(&app).contains(&format!("picked {}", FRUITS[1])));
}

#[test]
fn the_title_button_asks_for_a_title() {
    let mut app = headless_app();

    click(&mut app, TITLE_BUTTON.x, TITLE_BUTTON.y);
    app.update();

    let requests = app.world().resource::<Messages<TerminalRequest>>();
    let titles = MessageCursor::<TerminalRequest>::default()
        .read(requests)
        .filter(|request| matches!(request, TerminalRequest::SetTitle(_)))
        .count();
    assert_eq!(titles, 1);
}

#[test]
fn the_quit_button_exits() {
    let mut app = headless_app();

    click(&mut app, QUIT_BUTTON.x, QUIT_BUTTON.y);
    app.update();

    assert_eq!(app.should_exit(), Some(AppExit::Success));
}
