//! Generic modal-overlay primitives the input routers enforce.
//!
//! A modal overlay (a menu popup, a dialog) carries [`ModalOpen`] while it
//! is showing. The routers then treat modality generically: presses
//! outside every open modal request dismissal and are swallowed, wheel
//! ticks are swallowed wholesale, and interacting with a
//! [`ModalityToggle`] entity defers the rest of the input batch a frame so
//! it hit-tests the settled state.

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, EntityEvent, Query, With};
use bevy_ecs::system::SystemParam;
use plurimus_core::ratatui_core::layout::Position;

use crate::interaction::ComputedWidgetArea;

/// Present on a modal overlay root while it is showing. The root's
/// [`ComputedWidgetArea`] is the geometry that contains wheel ticks; its
/// owner removes the component when the modal closes.
#[derive(Component, Debug, Clone, Copy)]
pub struct ModalOpen;

/// Marks an entity whose activation changes modal state (an opener, a
/// row inside an overlay, the overlay itself). A press on one never
/// dismisses open modals, and a click on one defers the rest of the
/// pointer batch.
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
}

impl ModalGuard<'_, '_> {
    /// Requests dismissal of open modals on a press outside them.
    /// Returns true when the press was swallowed.
    pub(crate) fn dismiss_outside_press(
        &self,
        target: Option<Entity>,
        commands: &mut Commands,
    ) -> bool {
        if self.open.is_empty() || target.is_some_and(|target| self.affects_modality(target)) {
            return false;
        }
        self.dismiss_all(commands);
        true
    }

    /// Swallows a wheel tick while any modal is open, requesting
    /// dismissal when the tick lands outside them all. Returns true when
    /// swallowed.
    ///
    /// Ticks inside a modal are swallowed without closing it: letting one
    /// through would scroll whatever sits beneath the overlay. Hit-tests
    /// geometry rather than the wheel target, since overlays are not
    /// [`WheelReceptive`](crate::WheelReceptive) and so never win
    /// arbitration.
    pub(crate) fn intercept_wheel(&self, position: Position, commands: &mut Commands) -> bool {
        if self.open.is_empty() {
            return false;
        }
        if !self.open.iter().any(|(_, area)| area.0.contains(position)) {
            self.dismiss_all(commands);
        }
        true
    }

    pub(crate) fn affects_modality(&self, target: Entity) -> bool {
        self.toggles.contains(target)
    }

    fn dismiss_all(&self, commands: &mut Commands) {
        for (root, _) in self.open.iter() {
            commands.trigger(ModalDismiss { entity: root });
        }
    }
}
