//! The one `bevy_ui` tree. It states structure only - which nodes exist, how
//! they nest, which way they flow, and what text they carry - and tags each
//! node with a `Part`. Every size, colour, border and glyph is a skin's, the
//! id column's width included. The field is upstream's `TextInput` on both
//! targets, so committing it is the tree's too.

use bevy_ecs::prelude::{ChildOf, Commands, Entity, On, Query, ResMut};
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_ui::widget::Text;
use bevy_ui::{
    AlignItems, Display, FlexDirection, GridPlacement, GridTrack, JustifyContent, Node, Val,
};
use plurimus::bui::bevy_text::{EditableText, Justify, LineBreak, TextLayout};
use plurimus::bui::bevy_ui_widgets::{Activate, Button, Checkbox, TextInput, ValueChange};
use plurimus::ui::bevy_input_focus::tab_navigation::{TabGroup, TabIndex};
use plurimus::ui::bevy_input_focus::{FocusedInput, InputFocus};

use crate::model::{Filter, Part, Todo, TodoId, Todos};

/// The ledger's columns: mark, text, id, delete.
const LEDGER_COLUMNS: u16 = 4;

fn column() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        ..Node::default()
    }
}

fn row() -> Node {
    Node {
        align_items: AlignItems::Center,
        ..Node::default()
    }
}

/// A line that never wraps, so a ledger cell stays one row tall.
const fn unwrapped(justify: Justify) -> TextLayout {
    TextLayout::new(justify, LineBreak::NoWrap)
}

pub(crate) fn spawn_tree(mut commands: Commands) {
    let root = commands
        .spawn((
            Part::Root,
            TabGroup::new(0),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                ..column()
            },
        ))
        .id();
    let card = commands.spawn((Part::Card, column(), ChildOf(root))).id();
    commands.spawn((Part::Title, Text::new("todos"), ChildOf(card)));

    let field = spawn_input_row(&mut commands, card);
    commands.insert_resource(InputFocus::from_entity(field));

    commands.spawn((
        Part::List,
        Node {
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::auto(),
                GridTrack::flex(1.0),
                GridTrack::auto(),
                GridTrack::auto(),
            ],
            column_gap: Val::Px(1.0),
            ..Node::default()
        },
        ChildOf(card),
    ));
    spawn_footer(&mut commands, card);
}

fn spawn_footer(commands: &mut Commands, card: Entity) {
    let footer = commands
        .spawn((
            Part::Footer,
            Node {
                justify_content: JustifyContent::SpaceBetween,
                ..row()
            },
            ChildOf(card),
        ))
        .id();
    commands.spawn((Part::Count, Text::default(), ChildOf(footer)));
    let filters = commands.spawn((Part::Filters, row(), ChildOf(footer))).id();
    for filter in Filter::ALL {
        commands
            .spawn((
                Part::Filter(filter),
                Button,
                TabIndex(0),
                Text::new(filter.label()),
                ChildOf(filters),
            ))
            .observe(move |_: On<Activate>, mut current: ResMut<Filter>| *current = filter);
    }
    commands
        .spawn((
            Part::Clear,
            Button,
            TabIndex(0),
            Text::new("Clear done"),
            ChildOf(footer),
        ))
        .observe(|_: On<Activate>, mut todos: ResMut<Todos>| todos.clear_done());
}

pub(crate) fn spawn_empty(commands: &mut Commands, list: Entity) {
    commands.spawn((
        Part::Empty,
        Text::new("Nothing here."),
        Node {
            grid_column: GridPlacement::span(LEDGER_COLUMNS),
            ..Node::default()
        },
        ChildOf(list),
    ));
}

/// One ledger row: four cells placed straight into the grid.
pub(crate) fn spawn_row(commands: &mut Commands, list: Entity, todo: &Todo) {
    let id = TodoId(todo.id);
    commands
        .spawn((
            Part::Check,
            Checkbox,
            TabIndex(0),
            id,
            Node::default(),
            ChildOf(list),
        ))
        .observe(
            move |change: On<ValueChange<bool>>, mut todos: ResMut<Todos>| {
                todos.set_done(id.0, change.value);
            },
        );
    commands.spawn((
        Part::ItemText,
        id,
        Text::new(todo.text.clone()),
        unwrapped(Justify::Left),
        ChildOf(list),
    ));
    commands.spawn((
        Part::ItemId,
        id,
        Text::new(format!("#{}", todo.id)),
        unwrapped(Justify::Right),
        ChildOf(list),
    ));
    commands
        .spawn((
            Part::Delete,
            Button,
            TabIndex(0),
            id,
            Text::new("×"),
            ChildOf(list),
        ))
        .observe(move |_: On<Activate>, mut todos: ResMut<Todos>| todos.remove(id.0));
}

/// The field and its Add button, returning the field.
fn spawn_input_row(commands: &mut Commands, card: Entity) -> Entity {
    let input = commands.spawn((Part::InputRow, row(), ChildOf(card))).id();
    let field = commands
        .spawn((
            Part::Field,
            TextInput,
            TabIndex(0),
            Node {
                flex_grow: 1.0,
                ..Node::default()
            },
            ChildOf(input),
        ))
        .observe(submit_field)
        .id();
    commands
        .spawn((
            Part::AddButton,
            Button,
            TabIndex(0),
            Text::new("Add"),
            ChildOf(input),
        ))
        .observe(add_from_field);
    field
}

// Upstream's `TextInput` leaves Enter to propagate, so the field's own
// observer is what commits it.
fn submit_field(
    mut input: On<FocusedInput<KeyboardInput>>,
    mut fields: Query<&mut EditableText>,
    mut todos: ResMut<Todos>,
) {
    let key = &input.event().input;
    if key.state != ButtonState::Pressed || key.logical_key != Key::Enter {
        return;
    }
    if let Ok(mut field) = fields.get_mut(input.event().focused_entity) {
        commit(&mut field, &mut todos);
        input.propagate(false);
    }
}

fn add_from_field(
    _: On<Activate>,
    mut fields: Query<(&Part, &mut EditableText)>,
    mut todos: ResMut<Todos>,
) {
    for (part, mut field) in &mut fields {
        if *part == Part::Field {
            commit(&mut field, &mut todos);
        }
    }
}

fn commit(field: &mut EditableText, todos: &mut Todos) {
    todos.add(&field.value().to_string());
    field.clear();
}
