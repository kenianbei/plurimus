//! Declarative camera viewports, resolved to concrete rects each frame.
//!
//! A camera declares where it wants to sit - the whole terminal, a fixed
//! rect, a strip docked to an edge, or whatever is left over - and core
//! resolves that against the current [`TerminalSize`] into a
//! [`ResolvedViewport`]. Order decides the outcome: cameras resolve in
//! `(order, entity)` sequence and every [`Viewport::Docked`] strip carves from
//! one shared remaining region, so a later dock sees a smaller screen and
//! [`Viewport::Fill`] takes only what survives. Everything downstream reads
//! the resolved rect, never the declaration.

use bevy_ecs::prelude::{Commands, Component, DetectChangesMut, Entity, Query, Res};
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::SystemParam;
use ratatui_core::layout::Rect;

use crate::camera::{DefaultCamera, TerminalCamera};
use crate::size::TerminalSize;

/// Where a camera's viewport sits, resolved against the terminal each
/// frame. Docked strips carve from a shared remaining region in
/// `(order, entity)` sequence; fills take what is left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Viewport {
    /// The whole terminal.
    #[default]
    Full,
    /// A fixed cell rect, clamped to the screen.
    Fixed(Rect),
    /// A strip carved off one edge of the remaining region.
    Docked {
        /// Edge the strip is docked to.
        edge: Edge,
        /// Strip thickness in cells, clamped to what remains.
        cells: u16,
    },
    /// Whatever remains after all docked strips.
    Fill,
}

/// A terminal edge for [`Viewport::Docked`].
///
/// Closed: a terminal has four edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// Left edge.
    Left,
    /// Right edge.
    Right,
    /// Top edge.
    Top,
    /// Bottom edge.
    Bottom,
}

/// Screen region resolved for a camera this frame; `Rect::ZERO` while
/// the camera is inactive. Maintained by core in
/// [`CameraSystems::ResolveViewports`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedViewport(pub Rect);

/// Looks up a camera's [`ResolvedViewport`], falling back to the default
/// camera, so the fallback is one rule rather than one per system.
///
/// A system placing its own widgets pairs this with
/// [`ComputedUiCamera`](crate::ComputedUiCamera), which is where the camera
/// to pass comes from. Order such a system after
/// [`CameraSystems::ResolveViewports`], or the rect is the previous frame's.
#[derive(SystemParam)]
pub struct CameraViewports<'w, 's> {
    default_camera: Res<'w, DefaultCamera>,
    viewports: Query<'w, 's, &'static ResolvedViewport>,
}

impl CameraViewports<'_, '_> {
    /// The viewport of `camera`, or of the default camera when `None`.
    ///
    /// `None` when no camera answers - none is active, or the one named has
    /// no viewport resolved yet, which is the frame an entity spawns on.
    ///
    /// A caller reading [`ComputedUiCamera`](crate::ComputedUiCamera) has
    /// the default folded in already and never reaches the fallback; it is
    /// here for a caller holding a camera an app chose, which is the shape
    /// of every system that places widgets against a camera of its own.
    #[must_use]
    pub fn of(&self, camera: Option<Entity>) -> Option<Rect> {
        let target = camera.or(self.default_camera.0)?;
        self.viewports.get(target).ok().map(|resolved| resolved.0)
    }
}

/// Main-world camera maintenance sets, chained in `PreUpdate` in the order
/// `SyncSize`, `PropagateCameras`, `ResolveViewports`; consumers of
/// [`ResolvedViewport`] order themselves after
/// [`CameraSystems::ResolveViewports`], which is last and so after them all.
///
/// Declaration order is not run order: a set is appended here as it is
/// added, keeping every variant's discriminant where it was, and the chain
/// that `CorePlugin` configures is what sequences them.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum CameraSystems {
    /// Where [`TerminalSize`](crate::TerminalSize) settles for the frame.
    /// Core writes nothing here itself; a crate reporting size changes -
    /// `plurimus_term` from a real terminal, or an app resizing its own
    /// target - applies them in this set, and backends order their event
    /// pump before it.
    SyncSize,
    /// Resolves every camera's [`ResolvedViewport`]. Runs last.
    ResolveViewports,
    /// Resolves every widget's
    /// [`ComputedUiCamera`](crate::ComputedUiCamera) against the hierarchy
    /// and the default camera. Runs between the two above.
    PropagateCameras,
}

pub(crate) fn resolve_camera_viewports(
    size: Res<TerminalSize>,
    cameras: Query<(Entity, &TerminalCamera)>,
    mut resolved: Query<&mut ResolvedViewport>,
    mut commands: Commands,
) {
    let screen = size.rect();
    let mut ordered: Vec<_> = cameras.iter().collect();
    ordered.sort_by_key(|(entity, camera)| (camera.order, *entity));
    let mut remaining = screen;
    let strips: Vec<Rect> = ordered
        .iter()
        .map(|(_, camera)| match camera.viewport {
            Viewport::Docked { edge, cells } if camera.active => carve(&mut remaining, edge, cells),
            _ => Rect::ZERO,
        })
        .collect();
    for ((entity, camera), strip) in ordered.into_iter().zip(strips) {
        let rect = resolve_one(camera, screen, remaining, strip);
        match resolved.get_mut(entity) {
            Ok(mut current) => {
                current.set_if_neq(ResolvedViewport(rect));
            }
            Err(_) => {
                commands.entity(entity).insert(ResolvedViewport(rect));
            }
        }
    }
}

fn resolve_one(camera: &TerminalCamera, screen: Rect, remaining: Rect, strip: Rect) -> Rect {
    if !camera.active {
        return Rect::ZERO;
    }
    match camera.viewport {
        Viewport::Full => screen,
        Viewport::Fixed(rect) => rect.intersection(screen),
        Viewport::Docked { .. } => strip,
        Viewport::Fill => remaining,
    }
}

fn carve(remaining: &mut Rect, edge: Edge, cells: u16) -> Rect {
    let mut strip = *remaining;
    match edge {
        Edge::Left => {
            strip.width = cells.min(remaining.width);
            remaining.x += strip.width;
            remaining.width -= strip.width;
        }
        Edge::Right => {
            strip.width = cells.min(remaining.width);
            remaining.width -= strip.width;
            strip.x = remaining.x + remaining.width;
        }
        Edge::Top => {
            strip.height = cells.min(remaining.height);
            remaining.y += strip.height;
            remaining.height -= strip.height;
        }
        Edge::Bottom => {
            strip.height = cells.min(remaining.height);
            remaining.height -= strip.height;
            strip.y = remaining.y + remaining.height;
        }
    }
    strip
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_ecs::system::SystemState;
    use ratatui_core::layout::Rect;

    use super::{CameraViewports, Edge, ResolvedViewport, Viewport};
    use crate::{CorePlugin, TerminalCamera, TerminalSize};

    fn resolved(app: &App, entity: bevy_ecs::entity::Entity) -> Rect {
        app.world().get::<ResolvedViewport>(entity).unwrap().0
    }

    #[test]
    fn a_lookup_falls_back_to_the_default_camera() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 6));
        let main = app
            .world_mut()
            .spawn(TerminalCamera::default().with_viewport(Viewport::Fixed(Rect::new(0, 0, 4, 2))))
            .id();
        let side = app
            .world_mut()
            .spawn(
                TerminalCamera::default()
                    .with_order(1)
                    .with_viewport(Viewport::Fixed(Rect::new(4, 0, 6, 2))),
            )
            .id();
        let stranger = app.world_mut().spawn_empty().id();
        app.update();

        let mut lookup = SystemState::<CameraViewports>::new(app.world_mut());
        let cameras = lookup.get(app.world()).expect("the lookup borrows cleanly");

        assert_eq!(cameras.of(Some(side)), Some(Rect::new(4, 0, 6, 2)));
        assert_eq!(
            cameras.of(None),
            Some(Rect::new(0, 0, 4, 2)),
            "no camera named means the default one, which is `main` by order"
        );
        assert_eq!(cameras.of(Some(main)), cameras.of(None));
        assert_eq!(
            cameras.of(Some(stranger)),
            None,
            "an entity that is not a camera answers nothing rather than the default"
        );
    }

    #[test]
    fn a_lookup_with_no_camera_at_all_answers_nothing() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 6));
        app.update();

        let mut lookup = SystemState::<CameraViewports>::new(app.world_mut());

        let cameras = lookup.get(app.world()).expect("the lookup borrows cleanly");

        assert_eq!(cameras.of(None), None);
    }

    #[test]
    fn docked_strips_carve_in_order_and_fill_takes_the_rest() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 6));
        let main = app
            .world_mut()
            .spawn(TerminalCamera::default().with_viewport(Viewport::Fill))
            .id();
        let sidebar = app
            .world_mut()
            .spawn(
                TerminalCamera::default()
                    .with_order(1)
                    .with_viewport(Viewport::Docked {
                        edge: Edge::Left,
                        cells: 3,
                    }),
            )
            .id();
        let status = app
            .world_mut()
            .spawn(
                TerminalCamera::default()
                    .with_order(2)
                    .with_viewport(Viewport::Docked {
                        edge: Edge::Bottom,
                        cells: 1,
                    }),
            )
            .id();

        app.update();

        assert_eq!(resolved(&app, sidebar), Rect::new(0, 0, 3, 6));
        assert_eq!(resolved(&app, status), Rect::new(3, 5, 7, 1));
        assert_eq!(resolved(&app, main), Rect::new(3, 0, 7, 5));
    }

    #[test]
    fn oversized_docks_clamp_and_inactive_cameras_resolve_empty() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(4, 2));
        let greedy = app
            .world_mut()
            .spawn(TerminalCamera::default().with_viewport(Viewport::Docked {
                edge: Edge::Right,
                cells: 10,
            }))
            .id();
        let leftover = app
            .world_mut()
            .spawn(
                TerminalCamera::default()
                    .with_order(1)
                    .with_viewport(Viewport::Fill),
            )
            .id();
        let dormant = app
            .world_mut()
            .spawn(TerminalCamera::default().with_active(false))
            .id();

        app.update();

        assert_eq!(resolved(&app, greedy), Rect::new(0, 0, 4, 2));
        assert!(resolved(&app, leftover).is_empty());
        assert_eq!(resolved(&app, dormant), Rect::ZERO);
    }

    #[test]
    fn full_and_fixed_viewports_ignore_docking() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 4));
        let overlay = app
            .world_mut()
            .spawn(TerminalCamera::default().with_order(5))
            .id();
        let pinned = app
            .world_mut()
            .spawn(
                TerminalCamera::default().with_viewport(Viewport::Fixed(Rect::new(8, 3, 10, 10))),
            )
            .id();
        app.world_mut()
            .spawn(
                TerminalCamera::default()
                    .with_order(1)
                    .with_viewport(Viewport::Docked {
                        edge: Edge::Top,
                        cells: 2,
                    }),
            );

        app.update();

        assert_eq!(resolved(&app, overlay), Rect::new(0, 0, 10, 4));
        assert_eq!(resolved(&app, pinned), Rect::new(8, 3, 2, 1));
    }
}
