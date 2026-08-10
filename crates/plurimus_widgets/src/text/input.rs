//! The single-line text input widget.
//!
//! [`EditableText`] is the component an app spawns; the editing state lives
//! in [`TextInput`](super::TextInput) beside it and the row is drawn by
//! `field`. Keys are handled here rather than by an engine, which is what
//! keeps the field's bindings deliberately aligned with the multi-line
//! editor's - the same chord moves by word in both - while leaving the field
//! free of ratatui-textarea entirely.

use bevy_ecs::bundle::Bundle;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, Component, On, Query, Res, With, Without};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::{ButtonInput, ButtonState};
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_input_focus::{FocusLost, FocusedInput, InputFocus};
use plurimus_input::PasteMessage;

use super::field::TextField;
use super::state::TextInput;
use super::word::{word_end_forward, word_start_backward, word_start_forward};
use crate::stylist::{StateQuery, Stylable, StylistCache, hashed_bits, observed};
use crate::{ALT_KEYS, CTRL_KEYS, ValueChange, placeholder};
use plurimus_core::UiWidget;
use plurimus_ui::{Hovered, InteractionDisabled, UiTheme};

/// A single-line editable text field. Edits mutate [`TextInput`] directly
/// and emit [`ValueChange<String>`]: `is_final: false` per edit, `true`
/// on Enter and on focus loss.
#[derive(Component, Debug, Clone, Copy)]
#[require(Hovered, StylistCache, TextInput)]
pub struct EditableText;

/// Spawn bundle for a single-line text field.
pub fn editable_text(value: impl Into<String>) -> impl Bundle {
    (
        EditableText,
        TextInput::new(value),
        TabIndex(0),
        placeholder(),
    )
}

pub(crate) fn text_input_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held_keys: Res<ButtonInput<KeyCode>>,
    mut fields: Query<&mut TextInput, (With<EditableText>, Without<InteractionDisabled>)>,
    mut commands: Commands,
) {
    let field = input.focused_entity;
    let Ok(mut text) = fields.get_mut(field) else {
        return;
    };
    if input.input.state != ButtonState::Pressed {
        return;
    }
    let key = &input.input.logical_key;
    let length_before = text.value().len();
    let handled = word_edit(key, &held_keys, &mut text)
        || cluster_edit(key, &mut text)
        || apply_key(key, &mut text, field, &mut commands);
    if !handled {
        return;
    }
    input.propagate(false);
    if text.value().len() != length_before {
        emit(field, &text, false, &mut commands);
    }
}

fn apply_key(key: &Key, text: &mut TextInput, field: Entity, commands: &mut Commands) -> bool {
    match key {
        Key::Character(chars) => chars.chars().for_each(|c| text.insert(c)),
        Key::Space => text.insert(' '),
        Key::Home => text.move_start(),
        Key::End => text.move_end(),
        Key::Enter => emit(field, text, true, commands),
        _ => return false,
    }
    true
}

// Word bindings mirror TextEditor's, whose engine binds ctrl+arrows to
// word motion and alt+Backspace/Delete to word deletion.
fn word_edit(key: &Key, held_keys: &ButtonInput<KeyCode>, text: &mut TextInput) -> bool {
    let ctrl = held_keys.any_pressed(CTRL_KEYS);
    let alt = held_keys.any_pressed(ALT_KEYS);
    let cursor = text.cursor();
    match key {
        Key::ArrowLeft if ctrl => text.move_to(word_start_backward(text.value(), cursor)),
        Key::ArrowRight if ctrl => text.move_to(word_start_forward(text.value(), cursor)),
        Key::Backspace if alt => text.delete_to(word_start_backward(text.value(), cursor)),
        Key::Delete if alt => text.delete_to(word_end_forward(text.value(), cursor)),
        _ => return false,
    }
    true
}

// A one-past-the-cursor target is a whole cluster step: the snap carries
// it the rest of the way across the cluster.
fn cluster_edit(key: &Key, text: &mut TextInput) -> bool {
    let cursor = text.cursor();
    match key {
        Key::ArrowLeft => text.move_to(cursor.saturating_sub(1)),
        Key::ArrowRight => text.move_to(cursor + 1),
        Key::Backspace => text.delete_to(cursor.saturating_sub(1)),
        Key::Delete => text.delete_to(cursor + 1),
        _ => return false,
    }
    true
}

fn emit(field: Entity, text: &TextInput, is_final: bool, commands: &mut Commands) {
    commands.trigger(ValueChange {
        source: field,
        value: text.value().to_owned(),
        is_final,
    });
}

pub(crate) fn text_input_paste(
    mut input: On<FocusedInput<PasteMessage>>,
    mut fields: Query<&mut TextInput, (With<EditableText>, Without<InteractionDisabled>)>,
    mut commands: Commands,
) {
    let field = input.focused_entity;
    let Ok(mut text) = fields.get_mut(field) else {
        return;
    };
    input.propagate(false);
    let mut edited = false;
    for pasted in input.input.0.chars().filter(|c| !c.is_control()) {
        text.insert(pasted);
        edited = true;
    }
    if edited {
        emit(field, &text, false, &mut commands);
    }
}

pub(crate) fn text_input_blur(
    lost: On<FocusLost>,
    fields: Query<&TextInput, With<EditableText>>,
    mut commands: Commands,
) {
    if let Ok(text) = fields.get(lost.entity) {
        emit(lost.entity, text, true, &mut commands);
    }
}

pub(crate) fn style_text_inputs(
    theme: Res<UiTheme>,
    focus: Res<InputFocus>,
    mut fields: Query<
        (StateQuery, &TextInput, &mut StylistCache, &mut UiWidget),
        Stylable<EditableText>,
    >,
) {
    for (state, text, mut cache, mut widget) in &mut fields {
        let next = observed(state, &focus, hashed_bits(text));
        if !theme.is_changed() && next == *cache {
            continue;
        }
        *cache = next;
        *widget = UiWidget::new(TextField {
            value: text.value().to_owned(),
            cursor: text.cursor(),
            style: next.style(&theme),
        });
    }
}
