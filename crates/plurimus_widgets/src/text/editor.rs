//! The multi-line text editor widget, backed by ratatui-textarea.
//!
//! Unlike the single-line field, the editor does not own its text: a
//! [`TextArea`] behind an [`Arc`]/[`Mutex`] does, and this file translates key
//! and wheel input into that engine's own [`Input`] vocabulary. That is why
//! the widget is the crate's one [`LiveWidget`](plurimus_ui::LiveWidget) - its
//! rendered output can change without the [`UiWidget`] component being
//! replaced - and why edits are applied first and announced afterwards with
//! [`TextChanged`].

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Added, Commands, Component, EntityEvent, On, Query, Res, Without};
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::{ButtonInput, ButtonState};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_term::PasteMessage;
use ratatui_textarea::{DataCursor, Input, Key as EditorKey, TextArea};

use super::grapheme::{cluster_len_after, cluster_len_before};
use plurimus_core::UiWidget;
use plurimus_term::bevy_compat::held_modifiers;
use plurimus_ui::{Hovered, InteractionDisabled};
use plurimus_ui::{WheelReceptive, WheelScroll};

use plurimus_ui::LiveWidget;

/// A multi-line text editor. State lives behind a lock (`TextArea` is
/// `!Sync`); the widget is a live view onto it, so edits render without a
/// widget rebuild. Edits emit [`TextChanged`]; read content via
/// [`TextEditor::lock`].
///
/// Keys are dispatched through ratatui-textarea's default keymap, so its
/// emacs-flavored modifier bindings apply: ctrl+u undo, ctrl+r redo,
/// ctrl+x/c/y cut/copy/paste, shift+arrow selection. Tab is withheld for
/// focus navigation.
///
/// Movement and deletion step whole grapheme clusters, but the engine
/// records one history entry per scalar, so undoing a deleted multi-scalar
/// cluster restores it one scalar at a time. Its emacs aliases (ctrl+h,
/// ctrl+d) and selection-extending motions remain scalar-granular.
#[derive(Component, Clone)]
#[require(Hovered, WheelReceptive, LiveWidget)]
pub struct TextEditor(Arc<Mutex<TextArea<'static>>>);

impl TextEditor {
    /// An editor pre-filled with `text`.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let lines: Vec<String> = text.into().lines().map(str::to_owned).collect();
        Self(Arc::new(Mutex::new(TextArea::new(lines))))
    }

    /// Locks the editor state for reading or programmatic editing.
    pub fn lock(&self) -> MutexGuard<'_, TextArea<'static>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The editor's content changed; read it via [`TextEditor::lock`].
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct TextChanged {
    /// The editor entity.
    pub entity: Entity,
}

/// Spawn bundle for a multi-line text editor.
pub fn text_editor(text: impl Into<String>) -> impl Bundle {
    (TextEditor::new(text), TabIndex(0))
}

pub(crate) fn install_editor_views(
    editors: Query<(Entity, &TextEditor), Added<TextEditor>>,
    mut commands: Commands,
) {
    for (entity, editor) in &editors {
        commands
            .entity(entity)
            .insert(UiWidget::new(SharedTextArea(Arc::clone(&editor.0))));
    }
}

pub(crate) fn text_editor_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held_keys: Res<ButtonInput<KeyCode>>,
    editors: Query<&TextEditor, Without<InteractionDisabled>>,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    let Ok(editor) = editors.get(entity) else {
        return;
    };
    if input.input.state != ButtonState::Pressed {
        return;
    }
    let Some(edit) = editor_input(&input.input.logical_key, &held_keys) else {
        return;
    };
    input.propagate(false);
    let mut area = editor.lock();
    let mut edited = false;
    for _ in 0..cluster_steps(&area, &edit) {
        edited |= area.input(edit.clone());
    }
    if edited {
        commands.trigger(TextChanged { entity });
    }
}

/// How many scalar steps one engine input should repeat for.
///
/// Never zero: the engine owns line wrapping and joining at column edges.
/// Never above one while selecting: the first input eats the whole
/// selection. The ctrl/alt check mirrors textarea's keymap; if that moves
/// upstream this degrades to scalar stepping, wrong on screen but never
/// damaging text.
fn cluster_steps(area: &TextArea<'static>, edit: &Input) -> usize {
    let deletes = matches!(edit.key, EditorKey::Backspace | EditorKey::Delete);
    if edit.ctrl || edit.alt || (deletes && area.is_selecting()) {
        return 1;
    }
    let DataCursor(row, column) = area.cursor();
    let Some(line) = area.lines().get(row) else {
        return 1;
    };
    let steps = match edit.key {
        EditorKey::Left | EditorKey::Backspace => cluster_len_before(line, column),
        EditorKey::Right | EditorKey::Delete => cluster_len_after(line, column),
        _ => return 1,
    };
    steps.max(1)
}

fn editor_input(key: &Key, held_keys: &ButtonInput<KeyCode>) -> Option<Input> {
    let key = match key {
        Key::Character(chars) => EditorKey::Char(chars.chars().next()?),
        Key::Space => EditorKey::Char(' '),
        Key::Enter => EditorKey::Enter,
        Key::Backspace => EditorKey::Backspace,
        Key::Delete => EditorKey::Delete,
        Key::ArrowLeft => EditorKey::Left,
        Key::ArrowRight => EditorKey::Right,
        Key::ArrowUp => EditorKey::Up,
        Key::ArrowDown => EditorKey::Down,
        Key::Home => EditorKey::Home,
        Key::End => EditorKey::End,
        Key::PageUp => EditorKey::PageUp,
        Key::PageDown => EditorKey::PageDown,
        _ => return None,
    };
    let held = held_modifiers(held_keys);
    Some(Input {
        key,
        ctrl: held.ctrl,
        alt: held.alt,
        shift: held.shift,
    })
}

pub(crate) fn text_editor_paste(
    mut input: On<FocusedInput<PasteMessage>>,
    editors: Query<&TextEditor, Without<InteractionDisabled>>,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    let Ok(editor) = editors.get(entity) else {
        return;
    };
    input.propagate(false);
    if input.input.0.is_empty() {
        return;
    }
    editor.lock().insert_str(&input.input.0);
    commands.trigger(TextChanged { entity });
}

pub(crate) fn text_editor_wheel(event: On<WheelScroll>, editors: Query<&TextEditor>) {
    let Ok(editor) = editors.get(event.entity) else {
        return;
    };
    let (step_x, step_y) = event.step;
    editor.lock().scroll((step_y, step_x));
}

/// Deliberate exception to the immutable-`UiWidget` contract: reads live
/// state through the lock, which is why `TextEditor` requires `LiveWidget`.
/// Race-free - the render sub-app runs serially.
struct SharedTextArea(Arc<Mutex<TextArea<'static>>>);

impl Widget for &SharedTextArea {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let editor = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Widget::render(&*editor, area, buffer);
    }
}
