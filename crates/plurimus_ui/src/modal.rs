//! Generic modal-overlay primitives the input routers enforce.
//!
//! A modal overlay (a menu popup, a dialog) carries [`ModalOpen`] while it
//! is showing, and the root's screen rect is the geometry both routers ask
//! about. A pointer outside every open modal requests dismissal and is
//! swallowed; a pointer inside one is confined to the subtrees of the
//! modals containing it, so nothing an overlay covers is reachable through
//! it. Interacting with a [`ModalityToggle`] entity outside the overlays
//! routes rather than dismissing, and a click on one defers the rest of the
//! input batch a frame so it hit-tests the settled state.

use core::iter;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, Commands, Component, EntityEvent, Query, With};
use bevy_ecs::system::SystemParam;
use plurimus_core::ratatui_core::layout::Position;

use crate::interaction::ComputedWidgetArea;

/// Present on a modal overlay root while it is showing. The root's
/// [`ComputedWidgetArea`] is the geometry that confines input: a pointer
/// inside it reaches this modal's subtree and nothing else, and one outside
/// every open modal dismisses. Its owner removes the component when the
/// modal closes.
#[derive(Component, Debug, Clone, Copy)]
pub struct ModalOpen;

/// Marks an entity whose activation changes modal state (an opener, a row
/// inside an overlay, the overlay itself). A press on one outside every
/// open modal routes instead of dismissing, which is how an opener closes
/// what it opened, and a click on one defers the rest of the pointer batch.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ModalityToggle;

/// A press or wheel tick landed outside every open modal: the modal's
/// owner closes it (and restores focus) in response.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct ModalDismiss {
    /// The open modal root to dismiss.
    pub entity: Entity,
}

/// Where a pointer at one position may route.
pub(crate) enum ModalRouting {
    /// Nothing is modal: every widget under the pointer is a candidate.
    Unguarded,
    /// Inside at least one open modal: only their subtrees are candidates.
    Confined,
    /// Outside every open modal.
    Outside,
}

/// The open-modal state the input routers consult.
#[derive(SystemParam)]
pub(crate) struct ModalGuard<'w, 's> {
    open: Query<'w, 's, (Entity, &'static ComputedWidgetArea), With<ModalOpen>>,
    toggles: Query<'w, 's, (), With<ModalityToggle>>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl ModalGuard<'_, '_> {
    pub(crate) fn routing(&self, position: Position) -> ModalRouting {
        if self.open.is_empty() {
            return ModalRouting::Unguarded;
        }
        if self.open.iter().any(|(_, area)| area.0.contains(position)) {
            ModalRouting::Confined
        } else {
            ModalRouting::Outside
        }
    }

    /// Whether `entity` belongs to an open modal containing `position`: the
    /// root itself, or anything under it. Walking the entity's ancestors
    /// asks this once per candidate however many modals are open, and the
    /// union of them is what admits a submenu inside its parent.
    pub(crate) fn admits(&self, position: Position, entity: Entity) -> bool {
        iter::once(entity)
            .chain(self.parents.iter_ancestors(entity))
            .any(|ancestor| self.contains(ancestor, position))
    }

    pub(crate) fn affects_modality(&self, target: Entity) -> bool {
        self.toggles.contains(target)
    }

    pub(crate) fn dismiss_all(&self, commands: &mut Commands) {
        for (root, _) in self.open.iter() {
            commands.trigger(ModalDismiss { entity: root });
        }
    }

    fn contains(&self, entity: Entity, position: Position) -> bool {
        self.open
            .get(entity)
            .is_ok_and(|(_, area)| area.0.contains(position))
    }
}
