//! The one `bevy_ui` tree. It states structure only - which nodes exist, how
//! they nest, which way they flow, and what text they carry - and tags each
//! node with a `Part`. Every size, colour, border and glyph is a skin's, the
//! id column's width included.

use bevy_ecs::prelude::{ChildOf, Commands, Entity, On, ResMut};
use bevy_ui::widget::Text;
use bevy_ui::{
    AlignItems, Display, FlexDirection, GridPlacement, GridTrack, JustifyContent, Node, Val,
};
use plurimus::bui::bevy_text::{Justify, LineBreak, TextLayout};
use plurimus::bui::bevy_ui_widgets::{Activate, Button, Checkbox, ValueChange};
use plurimus::ui::bevy_input_focus::InputFocus;
use plurimus::ui::bevy_input_focus::tab_navigation::{TabGroup, TabIndex};

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

    let input = commands.spawn((Part::InputRow, row(), ChildOf(card))).id();
    let field = commands
        .spawn((
            Part::Field,
            Node {
                flex_grow: 1.0,
                ..Node::default()
            },
            ChildOf(input),
        ))
        .id();
    commands.insert_resource(InputFocus::from_entity(field));
    commands.spawn((
        Part::AddButton,
        Button,
        TabIndex(0),
        Text::new("Add"),
        ChildOf(input),
    ));

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
