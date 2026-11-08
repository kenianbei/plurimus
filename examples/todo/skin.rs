//! The terminal skin: a bordered card of plain text, state shown the way a
//! terminal shows it - reversed colours for focus, bold, strikethrough,
//! `[x]`. It answers `Part` with sizes and looks.

use bevy_color::Color;
use bevy_ecs::prelude::{Add, Commands, Entity, Has, On, Query, Res};
use bevy_ui::widget::Text;
use bevy_ui::{
    BackgroundColor, BorderColor, BorderRadius, Checked, GridTrack, Node, Pressed, UiRect, Val,
};
use plurimus::bui::bevy_text::{FontStyle, FontWeight, Strikethrough, TextColor, TextFont};
use plurimus::ui::Hovered;
use plurimus::ui::bevy_input_focus::InputFocus;

use crate::model::{Done, Part};
use crate::{State, toggle, write};

const CARD_WIDTH: f32 = 56.0;
/// Wide enough that `#` and two digits still right-align.
const ID_COLUMN: f32 = 4.0;
const FRAME: Color = Color::srgb(0.45, 0.45, 0.5);
const TEXT: Color = Color::srgb(0.9, 0.9, 0.9);
const ACCENT: Color = Color::srgb(0.8, 0.4, 0.9);
const MUTED: Color = Color::srgb(0.5, 0.5, 0.5);
const DONE: Color = Color::srgb(0.4, 0.8, 0.4);
const DANGER: Color = Color::srgb(0.9, 0.3, 0.3);
const INK: Color = Color::srgb(0.08, 0.08, 0.1);
const WELL: Color = Color::srgb(0.16, 0.16, 0.2);

const fn cells(count: f32) -> Val {
    Val::Px(count)
}

/// Sizes and borders per part. Runs once per node, when its `Part` lands.
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
        }
        Part::List => {
            node.min_height = cells(1.0);
            node.grid_template_columns[2] = GridTrack::px(ID_COLUMN);
        }
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

type Painted<'a> = (
    Entity,
    &'a Part,
    Option<&'a Hovered>,
    Has<Pressed>,
    Has<Checked>,
    Has<Done>,
    Has<Strikethrough>,
    Drawn<'a>,
);

/// What a look is written into, each absent until the first paint.
type Drawn<'a> = (
    Option<&'a mut Text>,
    Option<&'a mut TextColor>,
    Option<&'a mut BackgroundColor>,
    Option<&'a mut TextFont>,
);

/// Maps behaviour state to looks, every frame, writing only what differs.
pub(crate) fn paint(mut nodes: Query<Painted>, focus: Res<InputFocus>, mut commands: Commands) {
    for (entity, part, hovered, is_pressed, is_checked, is_done, is_struck, drawn) in &mut nodes {
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
        let (text, color, background, font) = drawn;
        if *part == Part::Check
            && let Some(mut text) = text
        {
            let mark = if state.is_checked { "[x]" } else { "[ ]" };
            if text.0 != mark {
                mark.clone_into(&mut text.0);
            }
        }
        write(entity, color, TextColor(look.fg), &mut commands);
        write(entity, background, BackgroundColor(look.bg), &mut commands);
        let font_look = TextFont {
            weight: look.weight,
            style: look.style,
            ..TextFont::default()
        };
        write(entity, font, font_look, &mut commands);
        let strike = look.is_struck.then_some(Strikethrough);
        toggle(entity, strike, is_struck, &mut commands);
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
        Part::Field => Look {
            bg: WELL,
            ..Look::plain(TEXT)
        },
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
