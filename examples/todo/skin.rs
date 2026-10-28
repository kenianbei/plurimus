//! The terminal skin: a bordered card of plain text, state shown the way a
//! terminal shows it - reversed colours for focus, bold, strikethrough,
//! `[x]`. It answers `Part` with sizes and looks, and supplies the one
//! control the tree leaves to each target: the field, a plurimus
//! `EditableText` placed over its node.

use bevy_color::Color;
use bevy_ecs::prelude::{
    Add, Commands, DetectChangesMut, Entity, Has, On, Query, Res, ResMut, With,
};
use bevy_ui::widget::Text;
use bevy_ui::{BackgroundColor, BorderColor, BorderRadius, Checked, Node, Pressed, UiRect, Val};
use plurimus::bui::ComputedNodeRect;
use plurimus::bui::bevy_text::{FontStyle, FontWeight, Strikethrough, TextColor, TextFont};
use plurimus::core::ratatui_core::layout::Rect;
use plurimus::core::{ResolvedViewport, TerminalCamera, UiArea, local_area};
use plurimus::ui::Hovered;
use plurimus::ui::bevy_input_focus::InputFocus;
use plurimus::widgets::{EditableText, Submit, TextInput, editable_text};

use crate::model::{Done, Part, Todos};

const CARD_WIDTH: f32 = 56.0;
const FRAME: Color = Color::srgb(0.45, 0.45, 0.5);
const TEXT: Color = Color::srgb(0.9, 0.9, 0.9);
const ACCENT: Color = Color::srgb(0.8, 0.4, 0.9);
const MUTED: Color = Color::srgb(0.5, 0.5, 0.5);
const DONE: Color = Color::srgb(0.4, 0.8, 0.4);
const DANGER: Color = Color::srgb(0.9, 0.3, 0.3);
const INK: Color = Color::srgb(0.08, 0.08, 0.1);

const fn cells(count: f32) -> Val {
    Val::Px(count)
}

/// Sizes and borders per part, and the field's terminal widget. Runs once
/// per node, when its `Part` lands.
pub(crate) fn shape(
    add: On<Add<Part>>,
    mut nodes: Query<(&Part, &mut Node)>,
    mut commands: Commands,
) {
    let Ok((part, mut node)) = nodes.get_mut(add.entity) else {
        return;
    };
    let mut entity = commands.entity(add.entity);
    match part {
        Part::Root => node.padding = UiRect::top(cells(1.0)),
        Part::Card => {
            node.width = Val::Percent(100.0);
            node.max_width = cells(CARD_WIDTH);
            node.border = UiRect::all(cells(1.0));
            node.border_radius = BorderRadius::all(cells(1.0));
            node.padding = UiRect::axes(cells(2.0), cells(0.0));
            entity.insert(BorderColor::all(FRAME));
        }
        Part::Title | Part::InputRow => node.margin = UiRect::bottom(cells(1.0)),
        Part::Field => {
            node.height = cells(1.0);
            node.margin = UiRect::right(cells(2.0));
            entity
                .insert((editable_text(""), UiArea::Fixed(Rect::ZERO)))
                .observe(submit_field);
        }
        Part::List => node.min_height = cells(1.0),
        Part::Check => {
            entity.insert(Text::new("[ ]"));
        }
        Part::Footer => node.margin = UiRect::top(cells(1.0)),
        Part::Filters => node.column_gap = cells(1.0),
        _ => {}
    }
    if is_interactive(*part) {
        entity.insert(Hovered::default());
    }
}

const fn is_interactive(part: Part) -> bool {
    matches!(
        part,
        Part::AddButton | Part::Check | Part::Delete | Part::Filter(_) | Part::Clear
    )
}

fn submit_field(submit: On<Submit>, mut todos: ResMut<Todos>, mut commands: Commands) {
    todos.add(&submit.value);
    commands.entity(submit.entity).insert(TextInput::new(""));
}

/// Keeps the field widget on its node, wherever layout puts it.
pub(crate) fn place_field(
    cameras: Query<&ResolvedViewport, With<TerminalCamera>>,
    mut fields: Query<(&ComputedNodeRect, &mut UiArea), With<EditableText>>,
) {
    let Ok(viewport) = cameras.single().map(|resolved| resolved.0) else {
        return;
    };
    for (node, mut area) in &mut fields {
        area.set_if_neq(UiArea::Fixed(local_area(node.rect, viewport)));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Look {
    fg: Color,
    bg: Color,
    weight: FontWeight,
    style: FontStyle,
    is_struck: bool,
}

impl Look {
    const fn plain(fg: Color) -> Self {
        Self {
            fg,
            bg: Color::NONE,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
            is_struck: false,
        }
    }

    const fn bold(self) -> Self {
        Self {
            weight: FontWeight::BOLD,
            ..self
        }
    }

    /// Reversed colours, a terminal's focus ring.
    const fn reversed(self) -> Self {
        Self {
            fg: INK,
            bg: self.fg,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the five facts a control is drawn from, read together"
)]
struct State {
    is_hovered: bool,
    is_pressed: bool,
    is_focused: bool,
    is_checked: bool,
    is_done: bool,
}

type Painted<'a> = (
    Entity,
    &'a Part,
    Option<&'a Hovered>,
    Has<Pressed>,
    Has<Checked>,
    Has<Done>,
    Has<Strikethrough>,
    Option<&'a mut Text>,
);

/// Maps behaviour state to looks, every frame, writing only what differs.
pub(crate) fn paint(mut nodes: Query<Painted>, focus: Res<InputFocus>, mut commands: Commands) {
    for (entity, part, hovered, is_pressed, is_checked, is_done, is_struck, text) in &mut nodes {
        let state = State {
            is_hovered: hovered.is_some_and(|hovered| hovered.0),
            is_pressed,
            is_focused: focus.get() == Some(entity),
            is_checked,
            is_done,
        };
        let Some(look) = look(*part, state) else {
            continue;
        };
        if *part == Part::Check
            && let Some(mut text) = text
        {
            let mark = if state.is_checked { "[x]" } else { "[ ]" };
            if text.0 != mark {
                mark.clone_into(&mut text.0);
            }
        }
        let mut entity = commands.entity(entity);
        entity.insert((
            TextColor(look.fg),
            BackgroundColor(look.bg),
            TextFont {
                weight: look.weight,
                style: look.style,
                ..TextFont::default()
            },
        ));
        if look.is_struck && !is_struck {
            entity.insert(Strikethrough);
        } else if !look.is_struck && is_struck {
            entity.remove::<Strikethrough>();
        }
    }
}

const fn interactive(base: Look, state: State) -> Look {
    let look = if state.is_hovered {
        Look { fg: ACCENT, ..base }
    } else {
        base
    };
    let look = if state.is_pressed { look.bold() } else { look };
    if state.is_focused {
        look.reversed()
    } else {
        look
    }
}

const fn look(part: Part, state: State) -> Option<Look> {
    Some(match part {
        Part::Title => Look::plain(ACCENT).bold(),
        Part::AddButton => interactive(Look::plain(TEXT), state),
        Part::Check => {
            let base = Look::plain(if state.is_checked { DONE } else { TEXT });
            interactive(base, state)
        }
        Part::ItemText | Part::ItemId if state.is_done => Look {
            is_struck: true,
            ..Look::plain(MUTED)
        },
        Part::ItemText => Look::plain(TEXT),
        Part::ItemId | Part::Count | Part::Empty => Look::plain(MUTED),
        Part::Delete => {
            let base = Look::plain(if state.is_hovered { DANGER } else { MUTED });
            interactive(
                base,
                State {
                    is_hovered: false,
                    ..state
                },
            )
        }
        Part::Filter(_) => {
            let base = if state.is_checked {
                Look::plain(TEXT).bold()
            } else {
                Look::plain(MUTED)
            };
            interactive(base, state)
        }
        Part::Clear => interactive(Look::plain(MUTED), state),
        _ => return None,
    })
}
