use crate::ui::pointer::UiPointerState;
use crate::ui::state::{UiDisabled, UiFocused};
use crate::ui::util::rect_contains;
use crate::widget::area::WidgetRect;
use crate::widget::order::WidgetOrder;
use bevy::prelude::{
    App, Component, Entity, IntoScheduleConfigs, Message, MessageReader, MessageWriter, Query, Res,
    ResMut, Resource, Result, Update, With, Without, World,
};
use bevy_ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use bevy_ratatui::event::{KeyMessage, MouseMessage};
use std::any::{Any, TypeId};
use std::sync::Arc;

#[derive(Resource, Default)]
pub struct UiActionState {
    pub sends: Vec<(Entity, UiErasedMessage)>,
    pub runs: Vec<(
        Entity,
        UiEvent,
        Arc<dyn Fn(&mut World, Entity, UiEvent) -> Result + Send + Sync>,
    )>,
}

/// A component that defines input bindings for a UI element.
/// Each binding consists of a trigger (key or mouse event), an intent (either sending a message or running a function), and a scope (focused, global, or targeted).
///
/// Example usage:
/// ```
/// use bevy_ratatui::crossterm::event::{KeyCode, MouseButton};
/// use plurimus::{UiActions, UiInputBinding, KeyBinding, MouseBinding, UiFocusMessage};
///
/// UiActions::new(vec![
///     UiInputBinding::key_message(KeyBinding::press(KeyCode::Tab), UiFocusMessage::Next).focused(),
///     UiInputBinding::mouse_binding(MouseBinding::down(MouseButton::Left), |world, entity, event| {
///         // Handle mouse click
///         Ok(())
///     }).targeted(),
/// ]);
#[derive(Component, Clone, Default)]
pub struct UiActions {
    pub inner: Vec<UiInputBinding>,
}

impl UiActions {
    pub fn new(bindings: impl Into<Vec<UiInputBinding>>) -> Self {
        Self {
            inner: bindings.into(),
        }
    }
}

impl<const N: usize> From<[UiInputBinding; N]> for UiActions {
    fn from(value: [UiInputBinding; N]) -> Self {
        Self::new(value.to_vec())
    }
}

/// Component that disables all UI actions on an entity.
/// This is separate from `UiDisabled` to allow for entities that are still focusable/hoverable/pressable but do not have any actions.
#[derive(Component, Clone, Copy, Debug)]
pub struct UiActionDisabled;

/// A wrapper for passing the original key or mouse event to the intent function.
///
/// Example usage:
/// ```
/// use bevy::prelude::*;
/// use plurimus::{UiEvent, UiActions, UiInputBinding, KeyBinding};
/// use bevy_ratatui::crossterm::event::{KeyCode, KeyEventKind};
///
/// UiActions::new(vec![
///     UiInputBinding::key_passthrough(|world, entity, event| {
///         if let UiEvent::Key(k) = event {
///             // Handle Enter key press
///         }
///         Ok(())
///     }).focused(),
/// ]);
#[derive(Clone, Copy, Debug)]
pub enum UiEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiActionScope {
    #[default]
    Focused,
    Global,
    Targeted,
}

#[derive(Clone, Debug)]
pub enum UiTrigger {
    Key(KeyBinding),
    Mouse(MouseBinding),
    KeyPassthrough,
    MousePassthrough,
}

impl UiTrigger {
    pub fn matches(&self, ev: UiEvent) -> bool {
        match (self, ev) {
            (UiTrigger::Key(b), UiEvent::Key(k)) => key_binding_matches_event(*b, k),
            (UiTrigger::Mouse(b), UiEvent::Mouse(m)) => mouse_binding_matches_event(*b, m),
            (UiTrigger::KeyPassthrough, UiEvent::Key(_)) => true,
            (UiTrigger::MousePassthrough, UiEvent::Mouse(_)) => true,
            _ => false,
        }
    }
}

/// A struct that defines a single input binding for a UI element, consisting of a trigger, an intent, and a scope.
///
/// Example usage:
/// ```
/// use bevy::prelude::*;
/// use plurimus::{UiInputBinding, KeyBinding, UiFocusMessage};
/// use bevy_ratatui::crossterm::event::{KeyCode, KeyEventKind};
///
/// UiInputBinding::key_message(KeyBinding::press(KeyCode::Tab), UiFocusMessage::Next).focused();
/// ```
#[derive(Clone)]
pub struct UiInputBinding {
    pub trigger: UiTrigger,
    pub intent: UiActionIntent,
    pub scope: UiActionScope,
}

impl UiInputBinding {
    pub fn global(mut self) -> Self {
        self.scope = UiActionScope::Global;
        self
    }

    pub fn focused(mut self) -> Self {
        self.scope = UiActionScope::Focused;
        self
    }

    pub fn targeted(mut self) -> Self {
        self.scope = UiActionScope::Targeted;
        self
    }

    pub fn key_message<T: Send + Sync + 'static>(key: KeyBinding, msg: T) -> Self {
        Self {
            trigger: UiTrigger::Key(key),
            intent: UiActionIntent::Send(UiErasedMessage::new(msg)),
            scope: UiActionScope::Focused,
        }
    }

    pub fn mouse_message<T: Send + Sync + 'static>(mouse: MouseBinding, msg: T) -> Self {
        Self {
            trigger: UiTrigger::Mouse(mouse),
            intent: UiActionIntent::Send(UiErasedMessage::new(msg)),
            scope: UiActionScope::Targeted,
        }
    }

    pub fn key_binding(
        key: KeyBinding,
        f: impl Fn(&mut World, Entity, UiEvent) -> Result + Send + Sync + 'static,
    ) -> Self {
        Self {
            trigger: UiTrigger::Key(key),
            intent: UiActionIntent::Run(Arc::new(f)),
            scope: UiActionScope::Focused,
        }
    }

    pub fn mouse_binding(
        mouse: MouseBinding,
        f: impl Fn(&mut World, Entity, UiEvent) -> Result + Send + Sync + 'static,
    ) -> Self {
        Self {
            trigger: UiTrigger::Mouse(mouse),
            intent: UiActionIntent::Run(Arc::new(f)),
            scope: UiActionScope::Targeted,
        }
    }

    pub fn key_passthrough(
        f: impl Fn(&mut World, Entity, UiEvent) -> Result + Send + Sync + 'static,
    ) -> Self {
        Self {
            trigger: UiTrigger::KeyPassthrough,
            intent: UiActionIntent::Run(Arc::new(f)),
            scope: UiActionScope::Focused,
        }
    }

    pub fn mouse_passthrough(
        f: impl Fn(&mut World, Entity, UiEvent) -> Result + Send + Sync + 'static,
    ) -> Self {
        Self {
            trigger: UiTrigger::MousePassthrough,
            intent: UiActionIntent::Run(Arc::new(f)),
            scope: UiActionScope::Targeted,
        }
    }
}

/// A struct that defines a key event trigger for a UI action, consisting of a key code, an event kind (press, release, or repeat), and any modifiers (shift, ctrl, alt).
///
/// Example usage:
/// ```
/// use bevy_ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
/// use plurimus::KeyBinding;
/// KeyBinding::press(KeyCode::Enter).with_modifiers(KeyModifiers::SHIFT);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub kind: KeyEventKind,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    pub fn press(code: KeyCode) -> Self {
        Self {
            code,
            kind: KeyEventKind::Press,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn release(code: KeyCode) -> Self {
        Self {
            code,
            kind: KeyEventKind::Release,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn repeat(code: KeyCode) -> Self {
        Self {
            code,
            kind: KeyEventKind::Repeat,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn with_modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

fn key_binding_matches_event(binding: KeyBinding, ev: KeyEvent) -> bool {
    binding.code == ev.code && binding.kind == ev.kind && binding.modifiers == ev.modifiers
}

/// A struct that defines a mouse event trigger for a UI action, consisting of an event kind (moved, scroll up/down, button down/up, or drag) and any modifiers (shift, ctrl, alt).
///
/// Example usage:
/// ```
/// use bevy_ratatui::crossterm::event::{MouseButton, MouseEventKind, KeyModifiers};
/// use plurimus::MouseBinding;
/// MouseBinding::down(MouseButton::Left).with_modifiers(KeyModifiers::CONTROL);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MouseBinding {
    pub kind: MouseEventKind,
    pub modifiers: KeyModifiers,
}

impl MouseBinding {
    pub fn moved() -> Self {
        Self {
            kind: MouseEventKind::Moved,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn scroll_up() -> Self {
        Self {
            kind: MouseEventKind::ScrollUp,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn scroll_down() -> Self {
        Self {
            kind: MouseEventKind::ScrollDown,
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn down(button: MouseButton) -> Self {
        Self {
            kind: MouseEventKind::Down(button),
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn up(button: MouseButton) -> Self {
        Self {
            kind: MouseEventKind::Up(button),
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn drag(button: MouseButton) -> Self {
        Self {
            kind: MouseEventKind::Drag(button),
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn with_modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

fn mouse_binding_matches_event(binding: MouseBinding, ev: MouseEvent) -> bool {
    binding.modifiers == ev.modifiers && binding.kind == ev.kind
}

#[derive(Clone)]
pub enum UiActionIntent {
    Send(UiErasedMessage),
    Run(Arc<dyn Fn(&mut World, Entity, UiEvent) -> Result + Send + Sync>),
}

#[derive(Clone)]
pub struct UiErasedMessage {
    pub type_id: TypeId,
    pub payload: Arc<dyn Any + Send + Sync>,
}

impl UiErasedMessage {
    pub fn new<T: Send + Sync + 'static>(msg: T) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            payload: Arc::new(msg),
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
}

/// An extension trait for `App` that adds a method to easily set up UI actions for a specific message type.
/// This method adds the message type to the app and sets up a system to dispatch messages of that type from the `UiActionState` after world intents have been run.
///
/// Example usage:
/// ```
/// use bevy::prelude::*;
/// use plurimus::{UiActionsAppExt, UiFocusMessage};
/// App::new()
///     .ui_actions_message::<UiFocusMessage>();
/// ```
pub trait UiActionsAppExt {
    fn ui_actions_message<T>(&mut self) -> &mut Self
    where
        T: Message + Clone;
}

impl UiActionsAppExt for App {
    fn ui_actions_message<T>(&mut self) -> &mut Self
    where
        T: Message + Clone,
    {
        self.add_message::<T>()
            .add_systems(Update, dispatch_ui_messages::<T>.after(run_world_intents));
        self
    }
}

enum UiRoute {
    Key,
    MouseTarget,
    MouseGlobal,
}

fn route_allows(scope: UiActionScope, is_focused: bool, route: &UiRoute) -> bool {
    match route {
        UiRoute::Key => match scope {
            UiActionScope::Global => true,
            UiActionScope::Focused => is_focused,
            UiActionScope::Targeted => false,
        },
        UiRoute::MouseTarget => match scope {
            UiActionScope::Global => false,
            UiActionScope::Targeted => true,
            UiActionScope::Focused => is_focused,
        },
        UiRoute::MouseGlobal => scope == UiActionScope::Global,
    }
}

fn enqueue_bindings(
    entity: Entity,
    actions: &UiActions,
    is_focused: bool,
    ev: UiEvent,
    route: UiRoute,
    q: &mut UiActionState,
) {
    for b in &actions.inner {
        if !route_allows(b.scope, is_focused, &route) {
            continue;
        }
        if !b.trigger.matches(ev) {
            continue;
        }

        match &b.intent {
            UiActionIntent::Send(m) => q.sends.push((entity, m.clone())),
            UiActionIntent::Run(f) => q.runs.push((entity, ev, f.clone())),
        }
    }
}

pub fn collect_key_actions(
    q_actions: Query<
        (Entity, &UiActions, Option<&UiFocused>),
        (Without<UiDisabled>, Without<UiActionDisabled>),
    >,
    mut mr_keys: MessageReader<KeyMessage>,
    mut q: ResMut<UiActionState>,
) {
    for km in mr_keys.read() {
        for (entity, actions, focused) in q_actions.iter() {
            enqueue_bindings(
                entity,
                actions,
                focused.is_some(),
                UiEvent::Key(km.0),
                UiRoute::Key,
                &mut q,
            );
        }
    }
}

pub fn collect_mouse_actions(
    q_actions: Query<
        (Entity, &UiActions, Option<&UiFocused>),
        (Without<UiDisabled>, Without<UiActionDisabled>),
    >,
    q_hit: Query<
        (Entity, Option<&WidgetRect>, Option<&WidgetOrder>),
        (
            With<UiActions>,
            Without<UiDisabled>,
            Without<UiActionDisabled>,
        ),
    >,
    r_ptr: Option<Res<UiPointerState>>,
    mut mr_mouse: MessageReader<MouseMessage>,
    mut q: ResMut<UiActionState>,
) {
    if mr_mouse.is_empty() {
        return;
    }

    let pressed = r_ptr
        .as_ref()
        .and_then(|p| p.pressed)
        .filter(|&e| q_actions.get(e).is_ok());

    for msg in mr_mouse.read() {
        let mouse_ev = msg.0;

        let target = match mouse_ev.kind {
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                pressed.or_else(|| hit_test_topmost(&q_hit, mouse_ev.column, mouse_ev.row))
            }
            _ => hit_test_topmost(&q_hit, mouse_ev.column, mouse_ev.row),
        };

        let ev = UiEvent::Mouse(mouse_ev);

        if let Some(entity) = target
            && let Ok((_e, actions, focused)) = q_actions.get(entity)
        {
            enqueue_bindings(
                entity,
                actions,
                focused.is_some(),
                ev,
                UiRoute::MouseTarget,
                &mut q,
            );
        }

        for (entity, actions, focused) in q_actions.iter() {
            enqueue_bindings(
                entity,
                actions,
                focused.is_some(),
                ev,
                UiRoute::MouseGlobal,
                &mut q,
            );
        }
    }
}

pub fn run_world_intents(world: &mut World) {
    let runs = {
        let mut q = world
            .get_resource_mut::<UiActionState>()
            .expect("UiIntentQueue not initialized");
        std::mem::take(&mut q.runs)
    };

    for (entity, ev, f) in runs {
        if f(world, entity, ev).is_err() {
            continue;
        }
    }
}

fn dispatch_ui_messages<T: Message + Clone>(
    mut mw: MessageWriter<T>,
    mut q: ResMut<UiActionState>,
) {
    let mut i = 0;
    while i < q.sends.len() {
        if q.sends[i].1.type_id == TypeId::of::<T>() {
            let (_entity, erased) = q.sends.swap_remove(i);
            if let Some(msg) = erased.downcast_ref::<T>() {
                mw.write(msg.clone());
            }
        } else {
            i += 1;
        }
    }
}

fn hit_test_topmost(
    q: &Query<
        (Entity, Option<&WidgetRect>, Option<&WidgetOrder>),
        (
            With<UiActions>,
            Without<UiDisabled>,
            Without<UiActionDisabled>,
        ),
    >,
    x: u16,
    y: u16,
) -> Option<Entity> {
    let mut best: Option<(i32, Entity)> = None;

    for (e, rect, order) in q.iter() {
        let Some(rect) = rect else {
            continue;
        };
        if !rect_contains(rect.0, x, y) {
            continue;
        }

        let z = order.map(|o| o.0).unwrap_or(0);
        match best {
            None => best = Some((z, e)),
            Some((best_z, _)) if z >= best_z => best = Some((z, e)),
            _ => {}
        }
    }

    best.map(|(_, e)| e)
}
