//! Lending the main world to the render sub-app for the extract phase.
//!
//! Extract systems run inside the sub-app but need to read the main world,
//! which the sub-app cannot borrow. Rather than copy anything, the two worlds
//! are swapped: the main world moves into the render world as [`MainWorld`]
//! for the length of [`ExtractSchedule`], an empty `ScratchMainWorld` stands
//! in its place, and the swap is undone once the schedule finishes. Extract
//! systems therefore read the live main world, never a snapshot of it.

use core::mem;
use core::ops::{Deref, DerefMut};

use bevy_ecs::prelude::{Resource, World};

use crate::sub_app::ExtractSchedule;

/// The main world, readable from [`ExtractSchedule`] systems while
/// extraction runs.
#[derive(Resource)]
pub struct MainWorld(World);

impl Deref for MainWorld {
    type Target = World;

    fn deref(&self) -> &World {
        &self.0
    }
}

impl DerefMut for MainWorld {
    fn deref_mut(&mut self) -> &mut World {
        &mut self.0
    }
}

/// Placeholder that stands in for the main world while it is lent to the
/// render world.
#[derive(Resource, Default)]
pub(crate) struct ScratchMainWorld(World);

pub(crate) fn extract(main_world: &mut World, render_world: &mut World) {
    let scratch = main_world
        .remove_resource::<ScratchMainWorld>()
        .expect("scratch world must exist outside of extraction");
    let lent = mem::replace(main_world, scratch.0);
    render_world.insert_resource(MainWorld(lent));
    render_world.run_schedule(ExtractSchedule);
    let MainWorld(lent) = render_world
        .remove_resource::<MainWorld>()
        .expect("main world must exist during extraction");
    let scratch = mem::replace(main_world, lent);
    main_world.insert_resource(ScratchMainWorld(scratch));
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::prelude::{Commands, Res, Resource};

    use crate::{CorePlugin, MainWorld, TerminalRenderApp, TerminalRenderAppExt};

    #[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
    struct Marker(u32);

    fn extract_marker(main_world: Res<MainWorld>, mut commands: Commands) {
        commands.insert_resource(*main_world.resource::<Marker>());
    }

    #[test]
    fn extract_lends_and_restores_the_main_world() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(Marker(7));
        app.add_extract_systems(extract_marker);

        app.update();
        app.update();

        assert_eq!(app.world().resource::<Marker>(), &Marker(7));
        let render_world = app.sub_app(TerminalRenderApp).world();
        assert_eq!(render_world.resource::<Marker>(), &Marker(7));
    }
}
