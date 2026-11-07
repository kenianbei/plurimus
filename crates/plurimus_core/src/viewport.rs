//! Camera placements, resolved to concrete rects each frame.
//!
//! A camera declares where it wants to sit - the whole terminal, a fixed
//! rect, a strip docked to an edge, or whatever is left over - and core
//! resolves that against the current [`TerminalSize`] into a
//! [`ResolvedViewport`]. Order decides the outcome: cameras resolve in
//! `(order, entity)` sequence and every [`TerminalCamera::Docked`] strip
//! carves from one shared remaining region, so a later dock sees a smaller
//! screen and [`TerminalCamera::Fill`] takes only what survives. Everything
//! downstream reads the resolved rect, never the declaration. The same pass
//! mirrors the rect into the bevy [`Camera`] in [`CellSize`] pixels for a
//! camera carrying no [`Projection`] - the terminal is its whole target - and
//! leaves a projected camera to bevy's render stack, whose `camera_system`
//! keys on that same component and sizes its viewport from its image.

use bevy_camera::{Camera, Projection, RenderTargetInfo, Viewport};
use bevy_ecs::prelude::{Commands, Component, DetectChangesMut, Entity, Has, Query, Res};
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::SystemParam;
use bevy_math::{UVec2, Vec2};
use ratatui_core::layout::Rect;

use crate::camera::{DefaultCamera, TerminalCamera};
use crate::size::{CellSize, TerminalSize};

/// A terminal edge for [`TerminalCamera::Docked`].
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
    /// Propagates [`UiCamera`](crate::UiCamera) and [`Hidden`](crate::Hidden)
    /// down the hierarchy and resolves every widget's
    /// [`ComputedUiCamera`](crate::ComputedUiCamera) against the default
    /// camera. Runs between the two above.
    PropagateCameras,
}

type CameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TerminalCamera,
        &'static mut Camera,
        Has<Projection>,
        Option<&'static mut ResolvedViewport>,
    ),
>;

pub(crate) fn resolve_camera_viewports(
    size: Res<TerminalSize>,
    cell: Res<CellSize>,
    mut cameras: CameraQuery,
    mut commands: Commands,
) {
    let screen = size.rect();
    let cell = cell.vec();
    let target = scale(size.cols, size.rows, cell);
    let mut ordered: Vec<_> = cameras.iter_mut().collect();
    ordered.sort_by_key(|(entity, _, camera, _, _)| (camera.order, *entity));
    let mut remaining = screen;
    let strips: Vec<Rect> = ordered
        .iter()
        .map(|(_, placement, camera, _, _)| match **placement {
            TerminalCamera::Docked { edge, cells } if camera.is_active => {
                carve(&mut remaining, edge, cells)
            }
            _ => Rect::ZERO,
        })
        .collect();
    for ((entity, placement, mut camera, has_projection, resolved), strip) in
        ordered.into_iter().zip(strips)
    {
        let rect = if camera.is_active {
            resolve_one(*placement, screen, remaining, strip)
        } else {
            Rect::ZERO
        };
        match resolved {
            Some(mut current) => {
                current.set_if_neq(ResolvedViewport(rect));
            }
            None => {
                commands.entity(entity).insert(ResolvedViewport(rect));
            }
        }
        // Bypassed so an idle frame leaves the camera's change tick alone;
        // `Mut`'s own `DerefMut` would mark it before the comparison ran.
        if !has_projection
            && mirror_into_camera(camera.bypass_change_detection(), rect, target, cell)
        {
            camera.set_changed();
        }
    }
}

fn scale(x: u16, y: u16, cell: Vec2) -> UVec2 {
    (Vec2::new(f32::from(x), f32::from(y)) * cell).as_uvec2()
}

fn resolve_one(placement: TerminalCamera, screen: Rect, remaining: Rect, strip: Rect) -> Rect {
    match placement {
        TerminalCamera::Full => screen,
        TerminalCamera::Fixed(rect) => rect.intersection(screen),
        TerminalCamera::Docked { .. } => strip,
        TerminalCamera::Fill => remaining,
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

/// Returns whether anything was written. Neither bevy type is `PartialEq`,
/// so the comparison is by field.
fn mirror_into_camera(camera: &mut Camera, rect: Rect, target: UVec2, cell: Vec2) -> bool {
    let position = scale(rect.x, rect.y, cell);
    let extent = scale(rect.width, rect.height, cell);
    let viewport_differs = camera.viewport.as_ref().is_none_or(|viewport| {
        viewport.physical_position != position || viewport.physical_size != extent
    });
    if viewport_differs {
        camera.viewport = Some(Viewport {
            physical_position: position,
            physical_size: extent,
            depth: 0.0..1.0,
        });
    }
    let target_differs = camera
        .computed
        .target_info
        .as_ref()
        .is_none_or(|current| current.physical_size != target);
    if target_differs {
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: target,
            scale_factor: 1.0,
        });
    }
    viewport_differs || target_differs
}

#[cfg(test)]
mod tests {
    use bevy_app::{App, Last};
    use bevy_camera::{Camera, Projection};
    use bevy_ecs::prelude::{DetectChanges, Query, Ref, ResMut, Resource};
    use bevy_ecs::system::SystemState;
    use bevy_math::UVec2;
    use ratatui_core::layout::Rect;

    use super::{CameraViewports, Edge, ResolvedViewport};
    use crate::{CellSize, CorePlugin, TerminalCamera, TerminalSize};

    fn resolved(app: &App, entity: bevy_ecs::entity::Entity) -> Rect {
        app.world().get::<ResolvedViewport>(entity).unwrap().0
    }

    fn ordered(order: isize) -> Camera {
        Camera {
            order,
            ..Camera::default()
        }
    }

    #[test]
    fn a_lookup_falls_back_to_the_default_camera() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 6));
        let main = app
            .world_mut()
            .spawn(TerminalCamera::Fixed(Rect::new(0, 0, 4, 2)))
            .id();
        let side = app
            .world_mut()
            .spawn((TerminalCamera::Fixed(Rect::new(4, 0, 6, 2)), ordered(1)))
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
        let main = app.world_mut().spawn(TerminalCamera::Fill).id();
        let sidebar = app
            .world_mut()
            .spawn((
                TerminalCamera::Docked {
                    edge: Edge::Left,
                    cells: 3,
                },
                ordered(1),
            ))
            .id();
        let status = app
            .world_mut()
            .spawn((
                TerminalCamera::Docked {
                    edge: Edge::Bottom,
                    cells: 1,
                },
                ordered(2),
            ))
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
            .spawn(TerminalCamera::Docked {
                edge: Edge::Right,
                cells: 10,
            })
            .id();
        let leftover = app
            .world_mut()
            .spawn((TerminalCamera::Fill, ordered(1)))
            .id();
        let dormant = app
            .world_mut()
            .spawn((
                TerminalCamera::default(),
                Camera {
                    is_active: false,
                    ..Camera::default()
                },
            ))
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
            .spawn((TerminalCamera::Full, ordered(5)))
            .id();
        let pinned = app
            .world_mut()
            .spawn(TerminalCamera::Fixed(Rect::new(8, 3, 10, 10)))
            .id();
        app.world_mut().spawn((
            TerminalCamera::Docked {
                edge: Edge::Top,
                cells: 2,
            },
            ordered(1),
        ));

        app.update();

        assert_eq!(resolved(&app, overlay), Rect::new(0, 0, 10, 4));
        assert_eq!(resolved(&app, pinned), Rect::new(8, 3, 2, 1));
    }

    #[test]
    fn an_unprojected_camera_is_mirrored_in_cell_pixels_and_a_projected_one_is_left_alone() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize::new(10, 4));
        app.insert_resource(CellSize { x: 8.0, y: 16.0 });
        let windowed = app
            .world_mut()
            .spawn(TerminalCamera::Fixed(Rect::new(2, 1, 4, 2)))
            .id();
        let projected = app
            .world_mut()
            .spawn((
                TerminalCamera::Fixed(Rect::new(2, 1, 4, 2)),
                Projection::default(),
            ))
            .id();

        app.update();

        let camera = app.world().get::<Camera>(windowed).unwrap();
        let viewport = camera.viewport.as_ref().unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(16, 16));
        assert_eq!(viewport.physical_size, UVec2::new(32, 32));
        assert_eq!(
            camera.computed.target_info.as_ref().unwrap().physical_size,
            UVec2::new(80, 64)
        );
        let camera = app.world().get::<Camera>(projected).unwrap();
        assert!(camera.viewport.is_none());
        assert!(camera.computed.target_info.is_none());
        assert_eq!(resolved(&app, projected), Rect::new(2, 1, 4, 2));
    }

    #[derive(Resource, Default)]
    struct CameraChanged(bool);

    fn record_camera_change(cameras: Query<Ref<Camera>>, mut changed: ResMut<CameraChanged>) {
        changed.0 = cameras.iter().any(|camera| camera.is_changed());
    }

    #[test]
    fn an_idle_frame_leaves_the_mirrored_camera_unchanged() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.init_resource::<CameraChanged>();
        app.add_systems(Last, record_camera_change);
        app.insert_resource(TerminalSize::new(10, 4));
        app.world_mut().spawn(TerminalCamera::Full);

        app.update();
        assert!(app.world().resource::<CameraChanged>().0);
        app.update();
        assert!(!app.world().resource::<CameraChanged>().0);

        app.insert_resource(TerminalSize::new(12, 4));
        app.update();
        assert!(app.world().resource::<CameraChanged>().0);
    }
}
