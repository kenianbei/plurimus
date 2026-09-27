//! Rat-Man on the 2d pipeline: a ratatui rat eats cheese through a maze
//! while four bevy birds hunt it, all drawn as halfblock pixel art.
//!
//! Arrows or WASD steer, `r` starts over, `q` or ctrl-c quits. The maze
//! wants a terminal of at least 280 by 76 cells. It also builds for the
//! browser; the README's Examples section has the steps.

mod actor;
mod chase;
mod game;
mod ghosts;
mod hud;
mod input;
mod maze;
mod sprites;
#[cfg(test)]
mod tests;
mod walls;

use bevy_app::{App, AppExit};
use plurimus::core::CorePlugin;
use plurimus::render2d::Plugin2d;
use plurimus::widgets::WidgetsPlugin;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    add_backend(&mut app);
    app.add_plugins((WidgetsPlugin, Plugin2d));
    game::add_game(&mut app);
    app.run()
}

#[cfg(not(target_arch = "wasm32"))]
fn add_backend(app: &mut App) {
    use std::time::Duration;

    use bevy_app::ScheduleRunnerPlugin;
    use plurimus::crossterm::CrosstermPlugin;

    const FRAME_INTERVAL: Duration = Duration::from_millis(16);

    app.add_plugins((
        ScheduleRunnerPlugin::run_loop(FRAME_INTERVAL),
        CrosstermPlugin::default(),
    ));
}

#[cfg(target_arch = "wasm32")]
fn add_backend(app: &mut App) {
    use plurimus::web::{GridFit, WebPlugin};

    app.add_plugins(WebPlugin::new().fit(GridFit::Cells(maze::REQUIRED_COLS, maze::REQUIRED_ROWS)));
}
