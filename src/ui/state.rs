use crate::ui::focus::UiFocusState;
use crate::ui::pointer::UiPointerState;
use bevy::prelude::{Commands, Component, Entity, Or, Query, ResMut, With};

/// Component that marks an entity as focusable.
/// The `tab_index` field determines the order in which entities are focused when `UiFocusMessage::FocusNext` or `UiFocusMessage::FocusPrevious` is sent.
#[derive(Component, Debug, Clone, Copy)]
pub struct UiFocusable {
    pub tab_index: i32,
    pub enabled: bool,
}

impl UiFocusable {
    pub fn new(tab_index: i32) -> Self {
        Self {
            tab_index,
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Component that marks an entity as hoverable.
/// Hoverable entities can be hovered by the pointer and will receive `UiHovered` when hovered.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct UiHoverable;

/// Component that marks an entity as pressable.
/// Pressable entities can be pressed by the pointer and will receive `UiPressed` when pressed
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct UiPressable;

/// Component that marks an entity as disabled.
/// Disabled entities cannot be focused, hovered, or pressed and will have those states removed if they are already in them.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct UiDisabled;

/// Component that marks an entity as focused.
/// Focused entities are the target of keyboard input and are typically highlighted in some way.
/// An entity can only be focused if it has the `UiFocusable` component and is enabled.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct UiFocused;

/// Component that marks an entity as hovered.
/// Hovered entities are the target of pointer hover and are typically highlighted in some way.
/// An entity can only be hovered if it has the `UiHoverable` component and is enabled.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct UiHovered;

/// Component that marks an entity as pressed.
/// Pressed entities are the target of pointer press and are typically highlighted in some way.
/// An entity can only be pressed if it has the `UiPressable` component and is enabled.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct UiPressed;

pub fn sanitize_disabled_state(
    mut commands: Commands,
    mut r_ptr: ResMut<UiPointerState>,
    mut r_focus: ResMut<UiFocusState>,
    q_disabled_marked: Query<
        Entity,
        (
            With<UiDisabled>,
            Or<(With<UiFocused>, With<UiHovered>, With<UiPressed>)>,
        ),
    >,
    q_disabled: Query<(), With<UiDisabled>>,
) {
    for e in &q_disabled_marked {
        commands
            .entity(e)
            .remove::<UiFocused>()
            .remove::<UiHovered>()
            .remove::<UiPressed>();
    }

    if r_ptr.hovered.is_some_and(|e| q_disabled.get(e).is_ok()) {
        r_ptr.hovered = None;
    }
    if r_ptr.pressed.is_some_and(|e| q_disabled.get(e).is_ok()) {
        r_ptr.pressed = None;
    }
    if r_focus.focused.is_some_and(|e| q_disabled.get(e).is_ok()) {
        r_focus.focused = None;
    }
}
