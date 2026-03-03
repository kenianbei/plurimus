use crate::ui::state::{UiDisabled, UiHoverable, UiHovered, UiPressable, UiPressed};
use crate::widget::area::WidgetRect;
use crate::widget::order::WidgetOrder;
use bevy::prelude::{Commands, Entity, MessageReader, Query, ResMut, Resource, Result, With};
use bevy_ratatui::crossterm::event::{MouseButton, MouseEventKind};
use bevy_ratatui::event::MouseMessage;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct UiPointerState {
    pub hovered: Option<Entity>,
    pub pressed: Option<Entity>,
}

pub fn update_pointer_state(
    mut commands: Commands,
    mut mr_mouse: MessageReader<MouseMessage>,
    mut r_ptr: ResMut<UiPointerState>,
    q_mark_hovered: Query<(), With<UiHovered>>,
    q_mark_pressed: Query<(), With<UiPressed>>,
    q_hoverable: Query<(
        Entity,
        &WidgetRect,
        Option<&WidgetOrder>,
        Option<&UiDisabled>,
        Option<&UiHoverable>,
    )>,
    q_pressable: Query<(
        Entity,
        &WidgetRect,
        Option<&WidgetOrder>,
        Option<&UiDisabled>,
        Option<&UiPressable>,
    )>,
) -> Result {
    sanitize_pointer_state(&mut r_ptr, &q_mark_hovered, &q_mark_pressed);

    if mr_mouse.is_empty() {
        return Ok(());
    }

    for msg in mr_mouse.read() {
        let ev = &msg.0;
        let (x, y) = (ev.column, ev.row);

        let refresh_hover = matches!(
            ev.kind,
            MouseEventKind::Moved
                | MouseEventKind::Drag(_)
                | MouseEventKind::Down(_)
                | MouseEventKind::Up(_)
        );

        if refresh_hover {
            let hover_hit = hit_test_topmost_hover(&q_hoverable, x, y);
            update_hover(&mut commands, &mut r_ptr, &q_mark_hovered, hover_hit);
        }

        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let press_hit = hit_test_topmost_press(&q_pressable, x, y);
                set_pressed(&mut commands, &mut r_ptr, &q_mark_pressed, press_hit);
            }
            MouseEventKind::Drag(MouseButton::Left) => {}
            MouseEventKind::Up(MouseButton::Left) => {
                clear_pressed(&mut commands, &mut r_ptr, &q_mark_pressed);
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {}
            _ => {}
        }
    }

    Ok(())
}

fn sanitize_pointer_state(
    r_ptr: &mut ResMut<UiPointerState>,
    q_mark_hovered: &Query<(), With<UiHovered>>,
    q_mark_pressed: &Query<(), With<UiPressed>>,
) {
    if let Some(e) = r_ptr.hovered
        && q_mark_hovered.get(e).is_err()
    {
        r_ptr.hovered = None;
    }

    if let Some(e) = r_ptr.pressed
        && q_mark_pressed.get(e).is_err()
    {
        r_ptr.pressed = None;
    }
}

fn update_hover(
    commands: &mut Commands,
    r_ptr: &mut ResMut<UiPointerState>,
    q_mark_hovered: &Query<(), With<UiHovered>>,
    new_hover: Option<Entity>,
) {
    if r_ptr.hovered == new_hover {
        return;
    }

    if let Some(prev) = r_ptr.hovered
        && q_mark_hovered.get(prev).is_ok()
    {
        commands.entity(prev).remove::<UiHovered>();
    }

    if let Some(next) = new_hover {
        commands.entity(next).insert(UiHovered);
    }

    r_ptr.hovered = new_hover;
}

fn set_pressed(
    commands: &mut Commands,
    r_ptr: &mut ResMut<UiPointerState>,
    q_mark_pressed: &Query<(), With<UiPressed>>,
    new_pressed: Option<Entity>,
) {
    if let Some(prev) = r_ptr.pressed
        && q_mark_pressed.get(prev).is_ok()
    {
        commands.entity(prev).remove::<UiPressed>();
    }

    if let Some(next) = new_pressed {
        commands.entity(next).insert(UiPressed);
        r_ptr.pressed = Some(next);
    } else {
        r_ptr.pressed = None;
    }
}

fn clear_pressed(
    commands: &mut Commands,
    r_ptr: &mut ResMut<UiPointerState>,
    q_mark_pressed: &Query<(), With<UiPressed>>,
) {
    if let Some(prev) = r_ptr.pressed.take()
        && q_mark_pressed.get(prev).is_ok()
    {
        commands.entity(prev).remove::<UiPressed>();
    }
}

fn hit_test_topmost_hover(
    q: &Query<(
        Entity,
        &WidgetRect,
        Option<&WidgetOrder>,
        Option<&UiDisabled>,
        Option<&UiHoverable>,
    )>,
    x: u16,
    y: u16,
) -> Option<Entity> {
    let mut best: Option<(i32, Entity)> = None;

    for (e, rect, order, disabled, hoverable) in q.iter() {
        if disabled.is_some() || hoverable.is_none() {
            continue;
        }
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

fn hit_test_topmost_press(
    q: &Query<(
        Entity,
        &WidgetRect,
        Option<&WidgetOrder>,
        Option<&UiDisabled>,
        Option<&UiPressable>,
    )>,
    x: u16,
    y: u16,
) -> Option<Entity> {
    let mut best: Option<(i32, Entity)> = None;

    for (e, rect, order, disabled, pressable) in q.iter() {
        if disabled.is_some() {
            continue;
        }

        if pressable.is_none() {
            continue;
        }

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

fn rect_contains(rect: ratatui::prelude::Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y.saturating_add(0)
        && y < rect.y.saturating_add(rect.height)
}
