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

/// Marks an entity whose activation changes modal state. It answers two
/// questions, and a widget usually wants one of them: a press on one
/// outside every open modal routes instead of dismissing, which is how an
/// opener closes what it opened, and a click on one defers the rest of the
/// pointer batch, which is what a row inside an overlay needs.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ModalityToggle;

/// A press or wheel tick landed outside every open modal: the modal's
/// owner closes it (and restores focus) in response.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct ModalDismiss {
    /// The open modal root to dismiss.
    pub entity: Entity,
}

/// The open-modal state the input routers consult.
#[derive(SystemParam)]
pub(crate) struct ModalGuard<'w, 's> {
    open: Query<'w, 's, (Entity, &'static ComputedWidgetArea), With<ModalOpen>>,
    toggles: Query<'w, 's, (), With<ModalityToggle>>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl ModalGuard<'_, '_> {
    /// Whether a pointer at `position` may reach `entity`: everything,
    /// where no overlay covers the position, else the subtrees of the
    /// overlays that do. Taking their union is what admits a submenu
    /// inside its parent, and needs no ordering among open modals.
    pub(crate) fn admits(&self, position: Position, entity: Entity) -> bool {
        !self.confines(position)
            || iter::once(entity)
                .chain(self.parents.iter_ancestors(entity))
                .any(|ancestor| self.contains(position, ancestor))
    }

    /// Whether a pointer at `position` closes what is open: there is an
    /// open modal, and none of them covers the position.
    pub(crate) fn dismisses(&self, position: Position) -> bool {
        !self.open.is_empty() && !self.confines(position)
    }

    pub(crate) fn affects_modality(&self, target: Entity) -> bool {
        self.toggles.contains(target)
    }

    pub(crate) fn dismiss_all(&self, commands: &mut Commands) {
        for (root, _) in self.open.iter() {
            commands.trigger(ModalDismiss { entity: root });
        }
    }

    fn confines(&self, position: Position) -> bool {
        self.open.iter().any(|(_, area)| area.0.contains(position))
    }

    fn contains(&self, position: Position, entity: Entity) -> bool {
        self.open
            .get(entity)
            .is_ok_and(|(_, area)| area.0.contains(position))
    }
}
