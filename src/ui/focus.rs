use crate::ui::pointer::UiPointerState;
use crate::ui::state::{UiDisabled, UiFocusable, UiFocused};
use crate::ui::util::rect_contains;
use crate::widget::area::WidgetRect;
use crate::widget::order::WidgetOrder;
use bevy::ecs::entity::EntityIndex;
use bevy::prelude::{
    Commands, Entity, Message, MessageReader, MessageWriter, Query, Res, ResMut, Resource, Result,
    With,
};
use bevy_ratatui::crossterm::event::{MouseButton, MouseEventKind};
use bevy_ratatui::event::MouseMessage;

#[derive(Resource, Default)]
pub struct UiFocusState {
    pub focused: Option<Entity>,
}

/// Messages that can be sent to change the focused entity.
/// `Next` and `Prev` will focus the next or previous focusable entity in the tab order, respectively.
/// `First` will focus the first focusable entity in the tab order.
/// `Clear` will unfocus all entities.
/// `Set` will focus the specified entity if it is focusable and enabled, otherwise it will do nothing.
///
/// Example usage:
/// ```rust
/// use bevy::prelude::*;
/// use plurimus::UiFocusMessage;
///
/// fn some_system(mut mw_focus: MessageWriter<UiFocusMessage>) {
///     // Focus the next entity in the tab order
///     mw_focus.write(UiFocusMessage::next());
///
///     // Focus the previous entity in the tab order
///     mw_focus.write(UiFocusMessage::prev());
///
///     // Focus the first entity in the tab order
///     mw_focus.write(UiFocusMessage::first());
///
///     // Clear focus from all entities
///     mw_focus.write(UiFocusMessage::clear());
/// }
/// ```
#[derive(Clone, Copy, Debug, Message)]
pub enum UiFocusMessage {
    Next,
    Prev,
    First,
    Clear,
    Set(Entity),
}

impl UiFocusMessage {
    pub fn next() -> Self {
        Self::Next
    }
    pub fn prev() -> Self {
        Self::Prev
    }
    pub fn first() -> Self {
        Self::First
    }
    pub fn clear() -> Self {
        Self::Clear
    }
    pub fn set(entity: Entity) -> Self {
        Self::Set(entity)
    }
}
pub fn apply_focus_intents(
    mut commands: Commands,
    mut m_focus: MessageReader<UiFocusMessage>,
    q_focusable: Query<(Entity, &UiFocusable, Option<&UiDisabled>)>,
    q_focused: Query<Entity, With<UiFocused>>,
    mut r_focus: ResMut<UiFocusState>,
) -> Result {
    let mut ordered: Vec<(i32, EntityIndex, Entity)> = q_focusable
        .iter()
        .filter_map(|(e, f, disabled)| {
            if disabled.is_some() || !f.enabled {
                return None;
            }
            Some((f.tab_index, e.index(), e))
        })
        .collect();

    ordered.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let ordered: Vec<Entity> = ordered.into_iter().map(|(_, _, e)| e).collect();

    let current_raw = r_focus.focused.or_else(|| q_focused.iter().next());
    let current = current_raw.filter(|e| ordered.contains(e));

    if ordered.is_empty() {
        r_focus.focused = None;
        for e in q_focused.iter() {
            commands.entity(e).remove::<UiFocused>();
        }
        return Ok(());
    }

    if current_raw.is_some() && current.is_none() {
        for e in q_focused.iter() {
            commands.entity(e).remove::<UiFocused>();
        }
        r_focus.focused = None;
    }

    let mut target = current;

    for message in m_focus.read() {
        target = match message {
            UiFocusMessage::Clear => None,
            UiFocusMessage::First => Some(ordered[0]),
            UiFocusMessage::Next => {
                let idx = target
                    .and_then(|e| ordered.iter().position(|&x| x == e))
                    .unwrap_or(usize::MAX);

                Some(if idx == usize::MAX {
                    ordered[0]
                } else {
                    ordered[(idx + 1) % ordered.len()]
                })
            }
            UiFocusMessage::Prev => {
                let idx = target
                    .and_then(|e| ordered.iter().position(|&x| x == e))
                    .unwrap_or(usize::MAX);

                let prev = if idx == usize::MAX || idx == 0 {
                    ordered.len() - 1
                } else {
                    idx - 1
                };

                Some(ordered[prev])
            }
            UiFocusMessage::Set(e) => {
                if ordered.contains(e) {
                    Some(*e)
                } else {
                    target
                }
            }
        };
    }

    if m_focus.is_empty() && target == current {
        r_focus.focused = current;
        return Ok(());
    }

    if target == current {
        return Ok(());
    }

    for e in q_focused.iter() {
        commands.entity(e).remove::<UiFocused>();
    }

    if let Some(next) = target {
        commands.entity(next).insert(UiFocused);
    }

    r_focus.focused = target;
    Ok(())
}

pub fn focus_on_pointer(
    mut mw_focus: MessageWriter<UiFocusMessage>,
    mut mr_mouse: MessageReader<MouseMessage>,
    r_ptr: Option<Res<UiPointerState>>,
    q_focusable: Query<(
        Entity,
        &UiFocusable,
        Option<&UiDisabled>,
        Option<&WidgetRect>,
        Option<&WidgetOrder>,
    )>,
) {
    if mr_mouse.is_empty() {
        return;
    }

    let captured = r_ptr.as_ref().and_then(|p| p.pressed);
    for msg in mr_mouse.read() {
        let ev = &msg.0;

        let should_focus = matches!(
            ev.kind,
            MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Up(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left)
        );

        if !should_focus {
            continue;
        }

        let target = match ev.kind {
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                captured.or_else(|| hit_test_topmost_focusable(&q_focusable, ev.column, ev.row))
            }
            MouseEventKind::Down(MouseButton::Left) => {
                hit_test_topmost_focusable(&q_focusable, ev.column, ev.row)
            }
            _ => None,
        };

        if let Some(e) = target {
            mw_focus.write(UiFocusMessage::set(e));
        }
    }
}

fn hit_test_topmost_focusable(
    q: &Query<(
        Entity,
        &UiFocusable,
        Option<&UiDisabled>,
        Option<&WidgetRect>,
        Option<&WidgetOrder>,
    )>,
    x: u16,
    y: u16,
) -> Option<Entity> {
    let mut best: Option<(i32, Entity)> = None;

    for (e, focusable, disabled, rect, order) in q.iter() {
        if disabled.is_some() || !focusable.enabled {
            continue;
        }
        let Some(rect) = rect else { continue };
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
