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
use bevy_ecs::prelude::{
    Added, Commands, Component, EntityEvent, MessageWriter, On, Query, Res, Without,
};
use bevy_ecs::system::SystemParam;
use bevy_input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy_input::{ButtonInput, ButtonState};
use bevy_input_focus::FocusedInput;
use bevy_input_focus::tab_navigation::TabIndex;
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::widgets::Widget;
use plurimus_term::{KeyModifiers, LastCopied, PasteMessage, TerminalRequest};
use ratatui_textarea::{DataCursor, Input, Key as EditorKey, TextArea};

use super::grapheme::{cluster_len_after, cluster_len_before};
use plurimus_core::UiWidget;
use plurimus_term::bevy_compat::held_modifiers;
use plurimus_ui::{Hovered, InteractionDisabled};
use plurimus_ui::{ScrollBy, WheelReceptive};

use plurimus_ui::LiveWidget;

/// A multi-line text editor. State lives behind a lock (`TextArea` is
/// `!Sync`); the widget is a live view onto it, so edits render without a
/// widget rebuild. Edits emit [`TextChanged`]; read content via
/// [`TextEditor::lock`].
///
/// Keys are dispatched through ratatui-textarea's default keymap, so its
/// emacs-flavored modifier bindings apply: ctrl+u undo, ctrl+r redo,
/// ctrl+y paste, shift+arrow selection. Tab is withheld for focus
/// navigation.
///
/// ctrl+c, ctrl+x and ctrl+v are taken over rather than forwarded. Copy and
/// cut act as the engine would and also ask the terminal for the text
/// through [`TerminalRequest`], so a copy leaves the app; neither sends
/// anything when there is no selection. Whether it reaches a system
/// clipboard is the backend's business - `plurimus_crossterm` writes none
/// until asked - but it always reaches [`LastCopied`], and ctrl+v inserts
/// from there, so a copy in one editor is a paste in another.
///
/// ctrl+v therefore means the app's clipboard and ctrl+y the engine's own
/// kill ring, which ctrl+k and ctrl+w fill. They are deliberately separate:
/// a copy anywhere in the app would otherwise displace a kill the user has
/// not yet put back.
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

/// The clipboard both ways: what an editor offers the terminal, and what
/// it pastes from.
#[derive(SystemParam)]
pub(crate) struct Clipboard<'w> {
    requests: MessageWriter<'w, TerminalRequest>,
    copied: Res<'w, LastCopied>,
}

pub(crate) fn text_editor_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    held_keys: Res<ButtonInput<KeyCode>>,
    editors: Query<&TextEditor, Without<InteractionDisabled>>,
    mut clipboard: Clipboard,
    mut commands: Commands,
) {
    let entity = input.focused_entity;
    let Ok(editor) = editors.get(entity) else {
        return;
    };
    if input.input.state != ButtonState::Pressed {
        return;
    }
    let held = held_modifiers(&held_keys);
    if let Some(action) = clipboard_action(&input.input.logical_key, held) {
        input.propagate(false);
        if apply_clipboard(&mut editor.lock(), action, &mut clipboard) {
            commands.trigger(TextChanged { entity });
        }
        return;
    }
    let Some(edit) = editor_input(&input.input.logical_key, held) else {
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

/// What a clipboard chord asks for, taken over from textarea's keymap so
/// the text also reaches the terminal.
#[derive(Debug, Clone, Copy)]
enum ClipboardAction {
    Copy,
    Cut,
    Paste,
}

/// The modifier test mirrors textarea's own, which requires ctrl without
/// alt; matching it loosely would claim chords the engine would have
/// handled differently.
fn clipboard_action(key: &Key, held: KeyModifiers) -> Option<ClipboardAction> {
    let Key::Character(chars) = key else {
        return None;
    };
    if !held.ctrl || held.alt {
        return None;
    }
    match chars.as_str() {
        "c" => Some(ClipboardAction::Copy),
        "x" => Some(ClipboardAction::Cut),
        // The engine spends ctrl+v on page-down, which the PageDown key
        // already reaches; the paste every user expects is worth more.
        "v" => Some(ClipboardAction::Paste),
        _ => None,
    }
}

/// Applies the chord and reports whether the text changed.
fn apply_clipboard(
    area: &mut TextArea<'static>,
    action: ClipboardAction,
    clipboard: &mut Clipboard,
) -> bool {
    match action {
        // `copy` cancels the selection and reports nothing, so whether
        // there was one has to be asked before the call rather than after.
        ClipboardAction::Copy => {
            if area.is_selecting() {
                area.copy();
                offer_yank(area, &mut clipboard.requests);
            }
            false
        }
        ClipboardAction::Cut => {
            let cut = area.cut();
            if cut {
                offer_yank(area, &mut clipboard.requests);
            }
            cut
        }
        // `insert_str` is `paste` with the text supplied rather than taken
        // from the engine's yank, which is what keeps a kill intact.
        ClipboardAction::Paste => clipboard
            .copied
            .0
            .as_deref()
            .is_some_and(|text| area.insert_str(text)),
    }
}

/// Sends what the engine just yanked to the terminal, unless it is empty -
/// an empty copy would take away whatever the user last put there.
fn offer_yank(area: &TextArea<'static>, requests: &mut MessageWriter<TerminalRequest>) {
    let yanked = area.yank_text();
    if !yanked.is_empty() {
        requests.write(TerminalRequest::copy(yanked));
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

fn editor_input(key: &Key, held: KeyModifiers) -> Option<Input> {
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

pub(crate) fn text_editor_scrolled(event: On<ScrollBy>, editors: Query<&TextEditor>) {
    let Ok(editor) = editors.get(event.entity) else {
        return;
    };
    let (step_x, step_y) = event.step;
    // The engine's own delta is i16; a larger step is a jump it clamps
    // to its extent anyway.
    editor.lock().scroll((narrowed(step_y), narrowed(step_x)));
}

fn narrowed(step: i32) -> i16 {
    step.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
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
