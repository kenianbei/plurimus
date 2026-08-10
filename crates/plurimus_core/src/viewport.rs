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
use ratatui_core::layout::Rect;

use crate::camera::TerminalCamera;
use crate::size::TerminalSize;

/// Where a camera's viewport sits, resolved against the terminal each
/// frame. Docked strips carve from a shared remaining region in
/// `(order, entity)` sequence; fills take what is left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// Main-world camera maintenance sets, chained in `PreUpdate`; consumers
/// of [`ResolvedViewport`] order themselves after
/// [`CameraSystems::ResolveViewports`].
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum CameraSystems {
    /// Where [`TerminalSize`](crate::TerminalSize) settles for the frame.
    /// Core writes nothing here itself; a crate reporting size changes -
    /// `plurimus_term` from a real terminal, or an app resizing its own
    /// target - applies them in this set, and backends order their event
    /// pump before it.
    SyncSize,
    /// Resolves every camera's [`ResolvedViewport`].
    ResolveViewports,
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
    use ratatui_core::layout::Rect;

    use super::{Edge, ResolvedViewport, Viewport};
    use crate::{CorePlugin, TerminalCamera, TerminalSize};

    fn resolved(app: &App, entity: bevy_ecs::entity::Entity) -> Rect {
        app.world().get::<ResolvedViewport>(entity).unwrap().0
    }

    #[test]
    fn docked_strips_carve_in_order_and_fill_takes_the_rest() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize { cols: 10, rows: 6 });
        let main = app
            .world_mut()
            .spawn(TerminalCamera {
                viewport: Viewport::Fill,
                ..TerminalCamera::default()
            })
            .id();
        let sidebar = app
            .world_mut()
            .spawn(TerminalCamera {
                order: 1,
                viewport: Viewport::Docked {
                    edge: Edge::Left,
                    cells: 3,
                },
                ..TerminalCamera::default()
            })
            .id();
        let status = app
            .world_mut()
            .spawn(TerminalCamera {
                order: 2,
                viewport: Viewport::Docked {
                    edge: Edge::Bottom,
                    cells: 1,
                },
                ..TerminalCamera::default()
            })
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
        app.insert_resource(TerminalSize { cols: 4, rows: 2 });
        let greedy = app
            .world_mut()
            .spawn(TerminalCamera {
                viewport: Viewport::Docked {
                    edge: Edge::Right,
                    cells: 10,
                },
                ..TerminalCamera::default()
            })
            .id();
        let leftover = app
            .world_mut()
            .spawn(TerminalCamera {
                order: 1,
                viewport: Viewport::Fill,
                ..TerminalCamera::default()
            })
            .id();
        let dormant = app
            .world_mut()
            .spawn(TerminalCamera {
                active: false,
                ..TerminalCamera::default()
            })
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
        app.insert_resource(TerminalSize { cols: 10, rows: 4 });
        let overlay = app
            .world_mut()
            .spawn(TerminalCamera {
                order: 5,
                ..TerminalCamera::default()
            })
            .id();
        let pinned = app
            .world_mut()
            .spawn(TerminalCamera {
                viewport: Viewport::Fixed(Rect::new(8, 3, 10, 10)),
                ..TerminalCamera::default()
            })
            .id();
        app.world_mut().spawn(TerminalCamera {
            order: 1,
            viewport: Viewport::Docked {
                edge: Edge::Top,
                cells: 2,
            },
            ..TerminalCamera::default()
        });

        app.update();

        assert_eq!(resolved(&app, overlay), Rect::new(0, 0, 10, 4));
        assert_eq!(resolved(&app, pinned), Rect::new(8, 3, 2, 1));
    }
}
