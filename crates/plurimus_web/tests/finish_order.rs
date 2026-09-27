//! `WebPlugin` builds its backend only once the font has loaded, which is
//! after every plugin's `build`, so it adds the presenter from its `finish`.
//! That rests on bevy accepting and building a plugin added there; this pins
//! it, so a bevy upgrade that stops doing so fails here rather than in a
//! browser.

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::Resource;

#[derive(Resource)]
struct Built;

struct AddedLate;

impl Plugin for AddedLate {
    fn build(&self, app: &mut App) {
        app.insert_resource(Built);
    }
}

struct AddsFromFinish;

impl Plugin for AddsFromFinish {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        app.add_plugins(AddedLate);
    }
}

#[test]
fn a_plugin_added_from_finish_is_built() {
    let mut app = App::new();
    app.add_plugins(AddsFromFinish);
    app.finish();
    app.cleanup();

    assert!(app.world().contains_resource::<Built>());
}
