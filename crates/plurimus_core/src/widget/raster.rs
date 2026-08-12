//! Core's widget pipeline: widget entities extracted, then drawn in one
//! z-sorted pass.
//!
//! Every widget that reaches the ui pass is drawn here, sorted by
//! `(order, source entity)` so equal orders still resolve the same way from
//! one frame to the next, and rendered into its target camera's buffer. A
//! widget marked [`RasterDeferred`] opts out of this extraction because
//! another pipeline owns its drawing - that pipeline spawns its own
//! [`ExtractedWidget`], which is what keeps a deferred widget in the same
//! z-order as everything else.

use std::sync::Arc;

use bevy_ecs::prelude::{Commands, Component, Entity, Query, Res, ResMut, With, Without};
use bevy_ecs::schedule::SystemSet;

use super::placement::{UiArea, UiCamera, UiHidden, UiOrder, resolve_area};
use super::{TerminalWidget, UiWidget};
use crate::camera::{CameraBuffer, DefaultCamera, SourceCamera, camera_buffer_mut};
use crate::extract::MainWorld;

/// Marks a widget whose extraction and rasterization another system owns;
/// core's own widget pipeline skips it. The owning pipeline spawns its own
/// [`ExtractedWidget`] so the widget still renders in z-order with the
/// rest.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RasterDeferred;

/// The widget rasterizer's set within
/// [`RasterizeSystems::Ui`](crate::RasterizeSystems::Ui); pipelines that
/// draw beneath widgets order themselves before it.
#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct WidgetRasterize;

/// Render-world copy of a widget entity.
///
/// `target` is the main-world camera entity; rasterization resolves it via
/// [`SourceCamera`], because render-world cameras spawn from deferred
/// commands and do not exist yet during extraction. Deferred-raster
/// pipelines spawn their own during extraction; every `ExtractedWidget`
/// entity is despawned at the start of the next extraction.
#[derive(Component)]
#[non_exhaustive]
pub struct ExtractedWidget {
    /// What to render.
    pub widget: Arc<dyn TerminalWidget>,
    /// Camera-local placement.
    pub area: UiArea,
    /// Z-order; higher renders later, on top.
    pub order: i32,
    /// The main-world widget entity, tie-breaking equal orders.
    pub source: Entity,
    /// Main-world target camera; `None` falls back to the default camera.
    pub target: Option<Entity>,
}

impl ExtractedWidget {
    /// A widget extracted for `source`, on the default camera at order
    /// zero.
    #[must_use]
    pub fn new(widget: Arc<dyn TerminalWidget>, area: UiArea, source: Entity) -> Self {
        Self {
            widget,
            area,
            order: 0,
            source,
            target: None,
        }
    }

    /// Sets the z-order; higher renders later, on top.
    #[must_use]
    pub const fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Sets the main-world camera to render on; `None` is the default
    /// camera.
    #[must_use]
    pub const fn with_target(mut self, target: Option<Entity>) -> Self {
        self.target = target;
        self
    }
}

pub(crate) fn extract_widgets(
    mut main_world: ResMut<MainWorld>,
    existing: Query<Entity, With<ExtractedWidget>>,
    mut commands: Commands,
) {
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let mut widgets = main_world.query_filtered::<(
        Entity,
        &UiWidget,
        &UiArea,
        Option<&UiOrder>,
        Option<&UiCamera>,
    ), (Without<UiHidden>, Without<RasterDeferred>)>();
    for (source, widget, area, order, camera) in widgets.iter(&main_world) {
        commands.spawn(ExtractedWidget {
            widget: widget.content(),
            area: *area,
            order: order.map_or(0, |order| order.0),
            source,
            target: camera.map(|camera| camera.0),
        });
    }
}

pub(crate) fn rasterize_widgets(
    widgets: Query<&ExtractedWidget>,
    default_camera: Res<DefaultCamera>,
    mut cameras: Query<(&SourceCamera, &mut CameraBuffer)>,
) {
    let mut ordered: Vec<&ExtractedWidget> = widgets.iter().collect();
    ordered.sort_by_key(|widget| (widget.order, widget.source));
    for extracted in ordered {
        let Some(target) = extracted.target.or(default_camera.0) else {
            continue;
        };
        let Some(mut buffer) = camera_buffer_mut(&mut cameras, target) else {
            continue;
        };
        let area = resolve_area(extracted.area, buffer.0.area);
        if area.is_empty() {
            continue;
        }
        extracted.widget.render(area, &mut buffer.0);
    }
}
