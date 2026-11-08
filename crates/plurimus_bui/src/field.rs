//! Upstream's `TextInput` on terminal cells.
//!
//! `bevy_ui_widgets`' own observers turn keys and presses into the
//! [`TextEdit`]s they queue on a field's [`EditableText`]; on a window
//! `bevy_text` applies them through a font engine, and here they are applied
//! to `plurimus_ui`'s [`TextInput`] engine instead, whose value is written
//! back into the editor so [`EditableText::value`] answers on both targets.
//! The row is drawn by the engine's [`TextRow`], as a cell field draws, and
//! the caret's cell is published through [`WidgetCursor`] for the terminal's
//! own.

use bevy_app::{App, PostUpdate};
use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Component, Has, IntoScheduleConfigs, On, Query, Ref, RemovedComponents, Res, With,
};
use bevy_input_focus::{FocusedInput, InputFocus};
use bevy_math::Vec2;
use bevy_text::{EditableText, TextColor, TextCursorStyle, TextEdit, TextFont, TextReadWriteMode};
use bevy_ui::{
    AvailableSpace, ComputedNode, ContentSize, Measure, MeasureArgs, NodeMeasure, ResolvedAxis,
    UiGlobalTransform, UiSystems,
};
use bevy_ui_widgets::TextInputPlugin;
use bevy_window::Ime;
use plurimus_core::CellSize;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::ratatui_core::style::Style;
use plurimus_term::{Clipboard, PasteMessage, TerminalCursorStyle};
use plurimus_ui::{
    ComputedWidgetArea, InteractionDisabled, TextInput, TextInputAction, TextRow, UiTheme,
    WidgetCursor,
};

use super::extract::cell_color;
use super::rect::{ComputedNodeRect, compute_node_rects, node_boxes};
use super::text::cell_style;

/// Upstream's field marker, named apart from the engine it is edited by.
type UpstreamField = bevy_ui_widgets::TextInput;

/// The row a field draws this frame, read by extraction.
#[derive(Component, Debug, Clone, Default)]
pub(crate) struct FieldRow(pub(crate) Option<TextRow>);

pub(crate) fn install(app: &mut App) {
    app.add_message::<Ime>();
    if !app.is_plugin_added::<TextInputPlugin>() {
        app.add_plugins(TextInputPlugin);
    }
    app.register_required_components::<UpstreamField, TextInput>()
        .register_required_components::<UpstreamField, FieldRow>()
        .register_required_components::<UpstreamField, FieldExtent>()
        .register_required_components_with::<UpstreamField, WidgetCursor>(|| {
            WidgetCursor::nowhere().with_style(TerminalCursorStyle::SteadyBar)
        });
    app.add_systems(
        PostUpdate,
        (
            (drain_text_edits, size_fields)
                .chain()
                .in_set(UiSystems::Content),
            draw_fields
                .in_set(UiSystems::PostLayout)
                .after(compute_node_rects),
        ),
    );
    app.add_observer(paste_into_field);
}

/// Where a field's row sits in cells, from its node, rounded as the node's
/// own content box is so the row, the caret and a press agree.
#[derive(Clone, Copy)]
struct RowGeometry {
    origin_x: f32,
    left: i32,
    width: u16,
    cell_x: f32,
}

impl RowGeometry {
    fn of(node: &ComputedNode, transform: &UiGlobalTransform, cell: Vec2) -> Self {
        let (_, content) = node_boxes(node, transform, Rect::ZERO, cell);
        Self {
            origin_x: transform.translation.x + node.content_box().min.x,
            left: content.left,
            width: content.width(),
            cell_x: cell.x,
        }
    }

    // Upstream's point is in pixels from the content box's corner; the
    // column is the cell it falls in, counted from the cell the row starts at.
    fn column(self, point: Vec2) -> u16 {
        let cell = ((point.x + self.origin_x) / self.cell_x).floor() as i32;
        u16::try_from(cell - self.left).unwrap_or(0)
    }

    fn index(self, engine: &TextInput, point: Vec2) -> usize {
        engine.char_at_cell(None, self.width, self.column(point))
    }
}

type Fields<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut EditableText,
        &'static mut TextInput,
        &'static ComputedNode,
        &'static UiGlobalTransform,
    ),
    With<UpstreamField>,
>;

/// Applies what upstream queued, after taking any value the app wrote into
/// the editor itself - `clear`, `set_text` - as the engine's, cursor at the
/// end, which the edit upstream queues beside such a write then moves.
fn drain_text_edits(mut fields: Fields, cell: Res<CellSize>, mut clipboard: Clipboard) {
    for (mut editable, mut engine, node, transform) in &mut fields {
        let written = editable.editor.raw_text();
        if written != engine.value() {
            *engine = TextInput::new(written);
        }
        if editable.pending_edits.is_empty() {
            continue;
        }
        let host = HostEdit {
            row: RowGeometry::of(node, transform, cell.vec()),
            limit: editable.max_characters,
        };
        for edit in std::mem::take(&mut editable.pending_edits) {
            match edit_action(&edit) {
                Some(action) => _ = engine.apply(action),
                None => host.apply(&mut engine, edit, &mut clipboard),
            }
        }
        if engine.value() != editable.editor.raw_text() {
            editable.editor.set_text(engine.value());
        }
    }
}

// A field is one row, so every line and document edge is the value's.
fn edit_action(edit: &TextEdit) -> Option<TextInputAction> {
    use TextInputAction as Action;
    let either =
        |is_selecting: bool, moving, selecting| Some(if is_selecting { selecting } else { moving });
    match *edit {
        TextEdit::Left(shift) => either(shift, Action::Left, Action::SelectLeft),
        TextEdit::Right(shift) => either(shift, Action::Right, Action::SelectRight),
        TextEdit::WordLeft(shift) => either(shift, Action::WordLeft, Action::SelectWordLeft),
        TextEdit::WordRight(shift) => either(shift, Action::WordRight, Action::SelectWordRight),
        TextEdit::TextStart(shift)
        | TextEdit::HardLineStart(shift)
        | TextEdit::LineStart(shift) => either(shift, Action::Home, Action::SelectHome),
        TextEdit::TextEnd(shift) | TextEdit::HardLineEnd(shift) | TextEdit::LineEnd(shift) => {
            either(shift, Action::End, Action::SelectEnd)
        }
        TextEdit::Backspace => Some(Action::Backspace),
        TextEdit::BackspaceWord => Some(Action::WordBackspace),
        TextEdit::Delete => Some(Action::Delete),
        TextEdit::DeleteWord => Some(Action::WordDelete),
        TextEdit::SelectAll
        | TextEdit::SelectLineAtPoint(_)
        | TextEdit::SelectedHardLineAtPoint(_) => Some(Action::SelectAll),
        TextEdit::CollapseSelection => Some(Action::CancelSelection),
        _ => None,
    }
}

/// What the engine has no action for and this host carries out: the
/// clipboard, a pointer's point, and text bounded by `max_characters`.
#[derive(Clone, Copy)]
struct HostEdit {
    row: RowGeometry,
    limit: Option<usize>,
}

impl HostEdit {
    // Vertical motion, scrolling and composition have nothing to act on in
    // one row.
    fn apply(self, engine: &mut TextInput, edit: TextEdit, clipboard: &mut Clipboard) {
        match edit {
            TextEdit::SelectAllIfCollapsed if engine.selection().is_none() => engine.select_all(),
            TextEdit::Insert(text) => self.insert(engine, &text),
            TextEdit::Paste => {
                if let Some(copied) = clipboard.last_copied() {
                    self.insert(engine, copied);
                }
            }
            TextEdit::Copy => engine.copy_selection(clipboard, false),
            TextEdit::Cut => engine.copy_selection(clipboard, true),
            TextEdit::MoveToPoint(point) => engine.move_to(self.row.index(engine, point)),
            TextEdit::ShiftClickExtension(point) | TextEdit::ExtendSelectionToPoint(point) => {
                engine.select_to(self.row.index(engine, point));
            }
            TextEdit::SelectWordAtPoint(point) => {
                engine.select_word_at(self.row.index(engine, point));
            }
            _ => {}
        }
    }

    /// Inserts `text` unless the value would pass the limit, which upstream
    /// refuses whole rather than truncating.
    fn insert(self, engine: &mut TextInput, text: &str) {
        let mut edited = engine.clone();
        if edited.paste(text)
            && self
                .limit
                .is_none_or(|limit| edited.value().chars().count() <= limit)
        {
            *engine = edited;
        }
    }
}

/// The size a field's measure last stated, so an edit that leaves it alone
/// does not ask taffy to lay the node out again.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
struct FieldExtent(Option<(Option<f32>, Option<f32>)>);

/// Sizes the node as upstream's measure does, a cell standing in for a
/// glyph's advance and a row for a line: `visible_width` cells wide and
/// `visible_lines` rows tall, either left to the node's own style when unset.
fn size_fields(
    cell: Res<CellSize>,
    mut fields: Query<(&EditableText, &mut FieldExtent, &mut ContentSize), With<UpstreamField>>,
) {
    for (editable, mut extent, mut content) in &mut fields {
        let width = editable.visible_width.map(|columns| columns * cell.x);
        let height = editable.visible_lines.map(|rows| rows * cell.y);
        let stated = FieldExtent(Some((width, height)));
        if *extent == stated && !cell.is_changed() {
            continue;
        }
        *extent = stated;
        content.set(NodeMeasure::Custom(Box::new(FieldMeasure {
            width,
            height,
        })));
    }
}

/// `bevy_ui`'s editable-text measure, which is private: a stated size wins,
/// then the field's own, then whatever space is definite.
struct FieldMeasure {
    width: Option<f32>,
    height: Option<f32>,
}

impl Measure for FieldMeasure {
    fn measure(&mut self, args: MeasureArgs<'_>) -> Vec2 {
        let definite = |space| match space {
            AvailableSpace::Definite(length) => length,
            AvailableSpace::MinContent | AvailableSpace::MaxContent => 0.0,
        };
        let resolve = |axis: ResolvedAxis, own: Option<f32>, space| {
            let length = axis.effective.or(own).unwrap_or_else(|| definite(space));
            let floor = axis.min.map_or(length, |min| length.max(min));
            axis.max.map_or(floor, |max| floor.min(max))
        };
        Vec2::new(
            resolve(args.resolve_width(), self.width, args.available_width),
            resolve(args.resolve_height(), self.height, args.available_height),
        )
        .ceil()
    }
}

type Drawn<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Ref<'static, TextInput>,
        Ref<'static, ComputedNodeRect>,
        (&'static ComputedNode, &'static UiGlobalTransform),
        (
            Ref<'static, TextFont>,
            Ref<'static, TextColor>,
            Option<Ref<'static, TextCursorStyle>>,
        ),
        (
            &'static mut FieldRow,
            &'static mut WidgetCursor,
            &'static mut ComputedWidgetArea,
        ),
    ),
    With<UpstreamField>,
>;

/// Draws the value as a cell field does, windowed to keep the caret in view,
/// and publishes the caret's cell as the terminal's caret.
fn draw_fields(
    mut fields: Drawn,
    focus: Res<InputFocus>,
    theme: Res<UiTheme>,
    cell: Res<CellSize>,
    mut unstyled: RemovedComponents<TextCursorStyle>,
) {
    let is_restyled = focus.is_changed() || theme.is_changed() || cell.is_changed();
    let unstyled: Vec<Entity> = unstyled.read().collect();
    for (entity, engine, rect, (node, transform), (font, color, look), drawn) in &mut fields {
        let is_stale = is_restyled
            || engine.is_changed()
            || rect.is_changed()
            || font.is_changed()
            || color.is_changed()
            || look.as_ref().is_some_and(DetectChanges::is_changed)
            || unstyled.contains(&entity);
        if !is_stale {
            continue;
        }
        let (mut row, mut cursor, mut area) = drawn;
        let width = RowGeometry::of(node, transform, cell.vec()).width;
        let drawn_row = engine.row(None);
        cursor.cell = Some(Position::new(drawn_row.caret_column(width), 0));
        area.set_if_neq(ComputedWidgetArea(rect.content));
        let style = cell_style(&font, color.0, false, false);
        let is_focused = focus.get() == Some(entity);
        let text_row = TextRow::new(drawn_row, style, is_focused, &theme);
        row.0 = Some(match look {
            Some(look) => with_cursor_style(text_row, &look, is_focused),
            None => text_row,
        });
    }
}

/// Upstream's `TextCursorStyle` in place of the theme's caret and selection,
/// as a window draws them: colours as cell backgrounds, a transparent one
/// drawing nothing.
fn with_cursor_style(mut row: TextRow, look: &TextCursorStyle, is_focused: bool) -> TextRow {
    let background = |color| cell_color(color).map_or_else(Style::new, |bg| Style::new().bg(bg));
    let selection = if is_focused {
        look.selection_color
    } else {
        look.unfocused_selection_color
    };
    row.selection_style = background(selection);
    if let Some(fg) = look.selected_text_color.and_then(cell_color) {
        row.selection_style = row.selection_style.fg(fg);
    }
    row.caret = row.caret.map(|_| background(look.color));
    row
}

/// A bracketed paste into the focused field, queued as upstream queues
/// typing, the terminal's paste having no bevy message of its own.
fn paste_into_field(
    mut paste: On<FocusedInput<PasteMessage>>,
    mut fields: Query<
        (
            &mut EditableText,
            &TextReadWriteMode,
            Has<InteractionDisabled>,
        ),
        With<UpstreamField>,
    >,
) {
    let Ok((mut editable, mode, is_disabled)) = fields.get_mut(paste.focused_entity) else {
        return;
    };
    paste.propagate(false);
    if *mode == TextReadWriteMode::Editable && !is_disabled {
        editable.queue_edit(TextEdit::Insert(paste.input.0.as_str().into()));
    }
}
