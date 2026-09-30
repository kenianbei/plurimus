//! Pointer interaction state over widget screen rects.
//!
//! A widget's [`ComputedWidgetArea`] is the only thing hit-testing needs, so
//! hover, press, drag, release and click all resolve by z-ordered rect
//! search: the topmost area containing the cursor wins, and the components
//! that result ([`Hovered`], [`Pressed`], [`Click`]) are what widget crates
//! observe rather than raw mouse messages.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::{Entity, EntityHashSet};
use bevy_ecs::hierarchy::Children;
use bevy_ecs::prelude::{Commands, Component, EntityEvent, Has, Local, Query, Res, With, Without};
use bevy_ecs::query::QueryFilter;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{
    CameraViewports, ComputedHidden, ComputedUiCamera, UiArea, UiOrder, UiWidget, resolve_area,
};
use plurimus_term::CursorCell;

/// Whether the cursor is over the widget. Opt-in: spawn it on widgets that
/// should react to the pointer.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hovered(pub bool);

/// Present while the pointer is pressed on the widget, carrying the click
/// count of the press that set it - see [`PointerPress::count`].
///
/// The count lives here because a gesture outlives the message that started
/// it: the [`Click`] a release completes reports the same number the press
/// did, and a [`PointerDrag`] observer reads it off the entity, a drag
/// through the second press of a run being a different gesture from a drag
/// through the first.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pressed(pub u8);

impl Default for Pressed {
    /// A lone press: what a gesture something other than this router started
    /// counts as.
    fn default() -> Self {
        Self(1)
    }
}

/// Disables all interaction with the widget and everything beneath it
/// through `ChildOf`. What is disabled by an ancestor carries
/// [`ComputedDisabled`], which is what every input path reads.
///
/// A disabled widget is inert, not invisible: it still wins press
/// arbitration and absorbs the press - no event, no focus movement, and
/// nothing beneath it is pressed - the way a disabled control blocks a
/// click everywhere else. [`PressPassThrough`] is the opt-out. Wheel ticks
/// are the exception: a disabled widget consumes none, so they fall
/// through, the same as an axis it cannot scroll.
#[derive(Component, Debug, Clone, Copy)]
pub struct InteractionDisabled;

/// Present on every entity disabled by its own [`InteractionDisabled`] or
/// an ancestor's.
///
/// Resolved in [`UiSystems::Areas`](crate::UiSystems::Areas), before focus
/// dispatch and the pointer router read it, so what is disabled later in a
/// frame takes effect from the next one. Read it rather than write it: the
/// resolver removes it from whatever no [`InteractionDisabled`] reaches.
#[derive(Component, Debug, Clone, Copy)]
pub struct ComputedDisabled;

pub(crate) fn propagate_disabled(
    roots: Query<Entity, With<InteractionDisabled>>,
    children: Query<&Children>,
    marked: Query<Entity, With<ComputedDisabled>>,
    mut reached: Local<EntityHashSet>,
    mut commands: Commands,
) {
    reached.clear();
    for root in &roots {
        reached.insert(root);
        reached.extend(children.iter_descendants(root));
    }
    for entity in &marked {
        if !reached.contains(&entity) {
            commands.entity(entity).try_remove::<ComputedDisabled>();
        }
    }
    for &entity in &*reached {
        if !marked.contains(entity) {
            commands.entity(entity).try_insert(ComputedDisabled);
        }
    }
}

/// Exempts the widget from press hit-testing: a press lands on whatever
/// is beneath it. Presses only - the widget keeps its area for hover, the
/// wheel, and navigation. On a widget with [`InteractionDisabled`], this
/// restores fall-through where the default is to absorb.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PressPassThrough;

/// Keeps a press on the widget from moving focus, while the press itself
/// still lands - [`Pressed`], [`PointerDrag`], [`Click`] all arrive. For a
/// toolbar control beside an editor: tab-reachable through its `TabIndex`,
/// but a click on it leaves the keyboard - and any armed selection - where
/// they were.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PressFocusDisabled;

/// Toggle state for checkboxes and radio buttons.
#[derive(Component, Debug, Clone, Copy)]
pub struct Checked;

/// Screen-space rect the widget occupies, resolved every frame.
///
/// Requires [`ComputedUiCamera`] because a [`UiArea`] is camera-local:
/// resolving one means knowing which camera it is local to, whether or not
/// the entity draws anything of its own.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[require(ComputedUiCamera)]
pub struct ComputedWidgetArea(pub Rect);

/// Pointer click completed on a widget: pressed and released on it.
///
/// The release edge, so a widget that closes or despawns something on being
/// clicked does it after the pointer router is done with the gesture. What
/// the click landed on is resolved from [`position`](Self::position), not
/// from where the press was: a drag that starts on one row and ends on
/// another names the one it ended on.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Click {
    /// The clicked widget.
    pub entity: Entity,
    /// Cursor cell at release time, inside the widget's area.
    pub position: Position,
    /// Click count of the press this completes; see
    /// [`PointerPress::count`].
    pub count: u8,
}

impl Click {
    /// A click completed on `entity`, released at `position`, the first of
    /// its run.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self {
            entity,
            position,
            count: 1,
        }
    }

    /// The same click, counted as the `count`th of its run.
    #[must_use]
    pub const fn with_count(mut self, count: u8) -> Self {
        self.count = count;
        self
    }
}

/// Pointer pressed on a widget. Sent to the topmost hovered widget only,
/// so a press on an overlay never also reaches widgets beneath it.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PointerPress {
    /// The pressed widget.
    pub entity: Entity,
    /// Cursor cell at press time.
    pub position: Position,
    /// How many presses have run together here: 1 for a lone press, 2 for a
    /// double click, and up.
    ///
    /// Synthesized, since no terminal reports one, from the run of presses
    /// this crate keeps against the app-wide
    /// [`MultiClickWindow`](plurimus_term::MultiClickWindow). A run is this
    /// widget's and this cell's: a press elsewhere, on another widget, or on
    /// none at all starts the next press over at 1. It saturates rather than
    /// wrapping, so what a long run means is this widget's to decide.
    pub count: u8,
}

impl PointerPress {
    /// A press on `entity` at `position`, the first of its run.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self {
            entity,
            position,
            count: 1,
        }
    }

    /// The same press, counted as the `count`th of its run.
    #[must_use]
    pub const fn with_count(mut self, count: u8) -> Self {
        self.count = count;
        self
    }
}

/// Pointer moved while a widget is [`Pressed`]. Captured to that widget:
/// it arrives regardless of where the cursor has since moved.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PointerDrag {
    /// The widget the gesture started on.
    pub entity: Entity,
    /// Current cursor cell.
    pub position: Position,
}

impl PointerDrag {
    /// A drag captured to `entity`, now at `position`.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self { entity, position }
    }
}

/// Pointer released, ending the gesture on a [`Pressed`] widget. Captured
/// like [`PointerDrag`]; [`Click`] is the separate activation event, sent
/// only when the release lands back on the widget.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PointerRelease {
    /// The widget the gesture started on.
    pub entity: Entity,
    /// Cursor cell at release time.
    pub position: Position,
}

impl PointerRelease {
    /// A release ending the gesture on `entity`.
    #[must_use]
    pub const fn new(entity: Entity, position: Position) -> Self {
        Self { entity, position }
    }
}

/// The gesture on a [`Pressed`] widget ended without a release, as it does
/// when the terminal loses focus: no [`PointerRelease`] and no [`Click`]
/// follow, and a release reported afterwards reaches nothing. For a widget
/// that has to settle what a drag left behind.
#[derive(EntityEvent, Debug, Clone, Copy)]
#[non_exhaustive]
pub struct PointerCancel {
    /// The widget the gesture started on.
    pub entity: Entity,
}

impl PointerCancel {
    /// A cancel ending the gesture on `entity`.
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

/// Arbitration key: highest band first, then the innermost (smallest)
/// rect, then a stable tiebreak. Innermost-wins is what routes a tick to
/// a nested scroller rather than its scrolling ancestor.
pub(crate) fn z_order_key(
    entity: Entity,
    order: Option<&UiOrder>,
    area: Rect,
) -> (i32, std::cmp::Reverse<u32>, Entity) {
    let cells = u32::from(area.width) * u32::from(area.height);
    (
        order.map_or(0, |order| order.0),
        std::cmp::Reverse(cells),
        entity,
    )
}

pub(crate) fn attach_widget_areas(
    widgets: Query<Entity, (With<UiWidget>, Without<ComputedWidgetArea>)>,
    mut commands: Commands,
) {
    for entity in &widgets {
        commands
            .entity(entity)
            .insert(ComputedWidgetArea::default());
    }
}

pub(crate) fn compute_widget_areas(
    cameras: CameraViewports,
    mut widgets: Query<(
        &UiArea,
        &ComputedUiCamera,
        &mut ComputedWidgetArea,
        Has<ComputedHidden>,
    )>,
) {
    for (area, target, mut computed, hidden) in &mut widgets {
        let resolved = cameras
            .of(target.0)
            .filter(|_| !hidden)
            .map_or(Rect::ZERO, |viewport| resolve_area(*area, viewport));
        computed.set_if_neq(ComputedWidgetArea(resolved));
    }
}

pub(crate) fn hover_widgets(
    cursor: Res<CursorCell>,
    mut widgets: Query<(&ComputedWidgetArea, &mut Hovered)>,
) {
    for (area, mut hovered) in &mut widgets {
        let over = cursor.0.is_some_and(|position| area.0.contains(position));
        hovered.set_if_neq(Hovered(over));
    }
}

/// Widgets an input router arbitrates between, `F` selecting which kind
/// of input they receive.
pub(crate) type AreaTargetQuery<'w, 's, F> = Query<
    'w,
    's,
    (
        Entity,
        &'static ComputedWidgetArea,
        Option<&'static UiOrder>,
    ),
    F,
>;

/// The topmost target containing `position` that `accepts` the input, by
/// [`z_order_key`]. Rejected targets fall through to the next beneath.
pub(crate) fn topmost_at<F: QueryFilter>(
    position: Position,
    targets: &AreaTargetQuery<'_, '_, F>,
    accepts: impl Fn(Entity) -> bool,
) -> Option<Entity> {
    targets
        .iter()
        .filter(|(entity, area, _)| area.0.contains(position) && accepts(*entity))
        .max_by_key(|(entity, area, order)| z_order_key(*entity, *order, area.0))
        .map(|(entity, ..)| entity)
}

/// A widget's value changed. `T` is the value type: `bool` for toggles,
/// `f32` for sliders, [`Entity`] for selections, `String` for text,
/// `Position` for scroll offsets.
#[derive(EntityEvent, Debug, Clone)]
#[non_exhaustive]
pub struct ValueChange<T: Send + Sync + 'static> {
    /// The widget whose value changed.
    #[event_target]
    pub source: Entity,
    /// The new value.
    pub value: T,
    /// Whether this ends an interaction (a released drag, Enter) rather
    /// than an intermediate step of one.
    pub is_final: bool,
}

impl<T: Send + Sync + 'static> ValueChange<T> {
    /// A change of `source` to `value`, `is_final` when it ends an
    /// interaction rather than stepping through one.
    #[must_use]
    pub const fn new(source: Entity, value: T, is_final: bool) -> Self {
        Self {
            source,
            value,
            is_final,
        }
    }
}
