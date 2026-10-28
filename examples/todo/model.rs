//! The model and the systems that keep the tree showing it. Nothing here
//! reads a visual component, so a skin can change anything it draws without
//! touching this file.

use bevy_ecs::prelude::{
    Added, Children, Commands, Component, DetectChanges, Entity, Query, Res, Resource,
};
use bevy_ui::Checked;
use bevy_ui::widget::Text;

/// What a node is, for a skin to style. The only thing a skin keys on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Root,
    Card,
    Title,
    InputRow,
    Field,
    AddButton,
    List,
    Empty,
    Check,
    ItemText,
    ItemId,
    Delete,
    Footer,
    Count,
    Filters,
    Filter(Filter),
    Clear,
}

/// A cell of the ledger, naming the todo it shows.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TodoId(pub(crate) u32);

/// A cell whose todo is done.
#[derive(Component, Debug)]
pub(crate) struct Done;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Filter {
    #[default]
    All,
    Active,
    Completed,
}

impl Filter {
    pub(crate) const ALL: [Self; 3] = [Self::All, Self::Active, Self::Completed];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Active => "Active",
            Self::Completed => "Completed",
        }
    }

    const fn shows(self, todo: &Todo) -> bool {
        match self {
            Self::All => true,
            Self::Active => !todo.done,
            Self::Completed => todo.done,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Todo {
    pub(crate) id: u32,
    pub(crate) text: String,
    pub(crate) done: bool,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct Todos {
    items: Vec<Todo>,
    next_id: u32,
}

impl Todos {
    pub(crate) fn seeded() -> Self {
        let mut todos = Self::default();
        for text in ["Try the terminal skin", "Try the desktop skin", "Compare"] {
            todos.add(text);
        }
        todos.items[0].done = true;
        todos
    }

    pub(crate) fn add(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.next_id += 1;
        self.items.push(Todo {
            id: self.next_id,
            text: text.to_owned(),
            done: false,
        });
    }

    pub(crate) fn set_done(&mut self, id: u32, done: bool) {
        if let Some(todo) = self.items.iter_mut().find(|todo| todo.id == id) {
            todo.done = done;
        }
    }

    pub(crate) fn remove(&mut self, id: u32) {
        self.items.retain(|todo| todo.id != id);
    }

    pub(crate) fn clear_done(&mut self) {
        self.items.retain(|todo| !todo.done);
    }

    pub(crate) fn is_done(&self, id: u32) -> bool {
        self.items.iter().any(|todo| todo.id == id && todo.done)
    }

    pub(crate) fn left(&self) -> usize {
        self.items.iter().filter(|todo| !todo.done).count()
    }

    pub(crate) fn visible(&self, filter: Filter) -> impl Iterator<Item = &Todo> {
        self.items.iter().filter(move |todo| filter.shows(todo))
    }
}

// Cells are respawned only when the visible set changes, so a toggle keeps
// focus on the checkbox that was toggled.
pub(crate) fn sync_rows(
    todos: Res<Todos>,
    filter: Res<Filter>,
    lists: Query<(Entity, Option<&Children>, &Part)>,
    checks: Query<(&TodoId, &Part)>,
    mut commands: Commands,
) {
    if !todos.is_changed() && !filter.is_changed() {
        return;
    }
    let Some((list, children, _)) = lists.iter().find(|(.., part)| **part == Part::List) else {
        return;
    };
    let visible: Vec<&Todo> = todos.visible(*filter).collect();
    let shown = children.into_iter().flatten().filter_map(|child| {
        let (id, part) = checks.get(*child).ok()?;
        (*part == Part::Check).then_some(id.0)
    });
    if shown.eq(visible.iter().map(|todo| todo.id)) {
        return;
    }
    commands.entity(list).despawn_children();
    if visible.is_empty() {
        crate::tree::spawn_empty(&mut commands, list);
    }
    for todo in visible {
        crate::tree::spawn_row(&mut commands, list, todo);
    }
}

// Done and Checked follow the model; the checkbox is controlled, never
// self-updating, so the model is the one place "done" is decided. A cell
// spawned this frame is marked too, the model having changed before it
// existed.
pub(crate) fn sync_marks(
    todos: Res<Todos>,
    cells: Query<(Entity, &TodoId, &Part)>,
    spawned: Query<(), Added<TodoId>>,
    mut commands: Commands,
) {
    if !todos.is_changed() && spawned.is_empty() {
        return;
    }
    for (entity, id, part) in &cells {
        let done = todos.is_done(id.0);
        let mut cell = commands.entity(entity);
        if done {
            cell.insert(Done);
        } else {
            cell.remove::<Done>();
        }
        if *part != Part::Check {
            continue;
        }
        if done {
            cell.insert(Checked);
        } else {
            cell.remove::<Checked>();
        }
    }
}

pub(crate) fn sync_count(todos: Res<Todos>, mut counts: Query<(&Part, &mut Text)>) {
    if !todos.is_changed() {
        return;
    }
    let left = todos.left();
    let noun = if left == 1 { "item" } else { "items" };
    for (part, mut text) in &mut counts {
        if *part == Part::Count {
            text.0 = format!("{left} {noun} left");
        }
    }
}

// The selected filter carries Checked, the marker a checkbox's state is,
// so a skin styles "selected" one way.
pub(crate) fn sync_filters(
    filter: Res<Filter>,
    buttons: Query<(Entity, &Part)>,
    mut commands: Commands,
) {
    if !filter.is_changed() {
        return;
    }
    for (entity, part) in &buttons {
        let Part::Filter(own) = part else {
            continue;
        };
        if *own == *filter {
            commands.entity(entity).insert(Checked);
        } else {
            commands.entity(entity).remove::<Checked>();
        }
    }
}
