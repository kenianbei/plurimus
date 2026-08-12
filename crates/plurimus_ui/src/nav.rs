//! Directional focus navigation: a map of widget-geometry edges rebuilt
//! whenever the participating rects change.
//!
//! Arrow keys move focus by geometry rather than by tree order, so the map is
//! derived from where widgets actually sit on screen and regenerated when
//! that changes. An app can pin or block any direction by hand and those
//! edges survive every rebuild - only the slots the generator filled itself
//! are ever reset. Arrows reach this file only once no focused widget has
//! consumed them, and they never leave an open modal subtree.

use bevy_app::App;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, Local, On, Query, Ref, Res, ResMut, Resource, With, Without};
use bevy_ecs::system::SystemParam;
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::directional_navigation::{
    AutoNavigationConfig, DirectionalNavigationMap, DirectionalNavigationPlugin, FocusableArea,
    NavNeighbor, auto_generate_navigation_edges,
};
use bevy_input_focus::tab_navigation::{NavAction, TabGroup, TabIndex, TabNavigation};
use bevy_input_focus::{FocusCause, FocusedInput, InputFocus};
use bevy_math::{CompassOctant, Vec2};
use bevy_window::Window;
use plurimus_core::ratatui_core::layout::Rect;

use crate::interaction::{ComputedWidgetArea, InteractionDisabled};

// Terminal layouts are gridded: rows and columns must not cross-connect.
const GRID_MIN_ALIGNMENT: f32 = 0.5;

/// Controls the stock rebuild of the
/// [`DirectionalNavigationMap`](bevy_input_focus::directional_navigation::DirectionalNavigationMap).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct NavigationConfig {
    /// Rebuild the map from widget geometry whenever it changes. Hand-made
    /// edges survive rebuilds; disable to own the map entirely.
    pub auto_build: bool,
}

impl NavigationConfig {
    /// Sets whether the map is rebuilt from widget geometry.
    #[must_use]
    pub const fn with_auto_build(mut self, auto_build: bool) -> Self {
        self.auto_build = auto_build;
        self
    }
}

impl Default for NavigationConfig {
    fn default() -> Self {
        Self { auto_build: true }
    }
}

pub(crate) fn install(app: &mut App) {
    if !app.is_plugin_added::<DirectionalNavigationPlugin>() {
        app.add_plugins(DirectionalNavigationPlugin);
        app.insert_resource(AutoNavigationConfig {
            min_alignment_factor: GRID_MIN_ALIGNMENT,
            ..AutoNavigationConfig::default()
        });
    }
    app.init_resource::<NavigationConfig>();
}

type FocusableQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, Ref<'static, ComputedWidgetArea>),
    (With<TabIndex>, Without<InteractionDisabled>),
>;

const ALL_OCTANTS: [CompassOctant; 8] = [
    CompassOctant::North,
    CompassOctant::NorthEast,
    CompassOctant::East,
    CompassOctant::SouthEast,
    CompassOctant::South,
    CompassOctant::SouthWest,
    CompassOctant::West,
    CompassOctant::NorthWest,
];

type AutoEdges = Vec<(Entity, CompassOctant, Entity)>;

/// What the last rebuild left behind: the focusable count it saw, and the
/// edges it added, which are the only ones the next rebuild may reset.
#[derive(SystemParam)]
pub(crate) struct AutoEdgeState<'s> {
    previous_count: Local<'s, usize>,
    tracked: Local<'s, AutoEdges>,
}

pub(crate) fn build_navigation_map(
    config: Res<NavigationConfig>,
    auto: Res<AutoNavigationConfig>,
    focusables: FocusableQuery,
    mut map: ResMut<DirectionalNavigationMap>,
    mut state: AutoEdgeState,
) {
    if !config.auto_build {
        return;
    }
    // Adds, removals, and filter flips all change the count; a same-count
    // swap still marks the added area Changed.
    let count = focusables.iter().len();
    let is_stale = count != *state.previous_count
        || config.is_changed()
        || auto.is_changed()
        || focusables.iter().any(|(_, area)| area.is_changed());
    if !is_stale {
        return;
    }
    *state.previous_count = count;
    rebuild_auto_edges(
        &mut map,
        &focusable_areas(&focusables),
        &auto,
        &mut state.tracked,
    );
}

// Hand-made edges survive rebuilds: only slots this system added last
// time are reset - and only if the app has not overwritten them - and
// the generator skips every surviving Set/Blocked slot.
fn rebuild_auto_edges(
    map: &mut DirectionalNavigationMap,
    nodes: &[FocusableArea],
    auto: &AutoNavigationConfig,
    tracked: &mut AutoEdges,
) {
    for (entity, octant, target) in tracked.drain(..) {
        if let Some(neighbors) = map.neighbors.get_mut(&entity)
            && neighbors.get(octant) == NavNeighbor::Set(target)
        {
            neighbors.neighbors[octant.to_index()] = NavNeighbor::Auto;
        }
    }
    map.neighbors.retain(|_, neighbors| {
        neighbors
            .neighbors
            .iter()
            .any(|neighbor| *neighbor != NavNeighbor::Auto)
    });
    // The generator only fills Auto slots, so those are the only slots
    // it can have added edges to.
    let candidates: Vec<(Entity, CompassOctant)> = node_slots(nodes)
        .filter(|&(entity, octant)| map.get_neighbor(entity, octant) == NavNeighbor::Auto)
        .collect();
    auto_generate_navigation_edges(map, nodes, auto);
    for (entity, octant) in candidates {
        if let NavNeighbor::Set(target) = map.get_neighbor(entity, octant) {
            tracked.push((entity, octant, target));
        }
    }
}

fn node_slots(nodes: &[FocusableArea]) -> impl Iterator<Item = (Entity, CompassOctant)> + '_ {
    nodes
        .iter()
        .flat_map(|node| ALL_OCTANTS.iter().map(move |&octant| (node.entity, octant)))
}

fn focusable_areas(focusables: &FocusableQuery) -> Vec<FocusableArea> {
    let mut rects: Vec<(Entity, Rect)> = focusables
        .iter()
        .filter(|(_, area)| !area.0.is_empty())
        .map(|(entity, area)| (entity, area.0))
        .collect();
    rects.sort_by_key(|(entity, _)| *entity);
    rects
        .iter()
        .filter(|(_, rect)| !rects.iter().any(|(_, inner)| encloses(*rect, *inner)))
        .map(|(entity, rect)| focusable_area(*entity, *rect))
        .collect()
}

// The edge generator has no containment awareness; a focusable container
// enclosing focusable children would produce zero-distance edges, so it
// drops out of the map instead. Equal rects are siblings, not containers.
fn encloses(outer: Rect, inner: Rect) -> bool {
    outer != inner && outer.union(inner) == outer
}

#[derive(SystemParam)]
pub(crate) struct ModalScope<'w, 's> {
    parents: Query<'w, 's, &'static ChildOf>,
    groups: Query<'w, 's, &'static TabGroup>,
}

impl ModalScope<'_, '_> {
    // Arrows must not escape an open modal subtree, mirroring Tab.
    fn allows(&self, current: Entity, candidate: Entity) -> bool {
        let Some(root) = self.modal_root(current) else {
            return true;
        };
        candidate == root
            || self
                .parents
                .iter_ancestors(candidate)
                .any(|ancestor| ancestor == root)
    }

    fn modal_root(&self, entity: Entity) -> Option<Entity> {
        self.parents
            .iter_ancestors(entity)
            .find(|&ancestor| self.groups.get(ancestor).is_ok_and(|group| group.modal))
    }
}

#[derive(SystemParam)]
pub(crate) struct NavigationOrigin<'w, 's> {
    windows: Query<'w, 's, (), With<Window>>,
    tab_navigation: TabNavigation<'w, 's>,
}

impl NavigationOrigin<'_, '_> {
    // The virtual window holds focus before any widget does, and it is not
    // a navigable position.
    fn current(&self, focus: &InputFocus) -> Option<Entity> {
        focus.get().filter(|&entity| !self.windows.contains(entity))
    }

    // Entry mirrors tab navigation, so it also requires a TabGroup.
    fn focus_first_widget(&self, focus: &mut InputFocus) {
        if let Ok(first) = self
            .tab_navigation
            .navigate(&InputFocus::default(), NavAction::First)
        {
            focus.set(first, FocusCause::Navigated);
        }
    }
}

// Observes the virtual window, so only arrows no focused widget consumed
// arrive here. The event's target is the window by now; the navigation
// source is the actual focus.
pub(crate) fn navigate_on_arrows(
    input: On<FocusedInput<KeyboardInput>>,
    origin: NavigationOrigin,
    scope: ModalScope,
    map: Res<DirectionalNavigationMap>,
    mut focus: ResMut<InputFocus>,
) {
    let Some(octant) = arrow_octant(&input.input) else {
        return;
    };
    let Some(current) = origin.current(&focus) else {
        origin.focus_first_widget(&mut focus);
        return;
    };
    if let Some(candidate) = map.get_neighbor(current, octant).get()
        && scope.allows(current, candidate)
    {
        focus.set(candidate, FocusCause::Navigated);
    }
}

fn arrow_octant(input: &KeyboardInput) -> Option<CompassOctant> {
    if input.state != ButtonState::Pressed || input.repeat {
        return None;
    }
    match input.logical_key {
        Key::ArrowUp => Some(CompassOctant::North),
        Key::ArrowDown => Some(CompassOctant::South),
        Key::ArrowLeft => Some(CompassOctant::West),
        Key::ArrowRight => Some(CompassOctant::East),
        _ => None,
    }
}

fn focusable_area(entity: Entity, rect: Rect) -> FocusableArea {
    let size = Vec2::new(f32::from(rect.width), f32::from(rect.height));
    let origin = Vec2::new(f32::from(rect.x), f32::from(rect.y));
    FocusableArea {
        entity,
        position: origin + size / 2.0,
        size,
    }
}
