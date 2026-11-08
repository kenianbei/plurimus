//! Fontless text for `bevy_ui` nodes: `Text` roots and their `TextSpan`
//! children, measured by grapheme width through `ContentSize` and resolved
//! once per change into the cell styles the rasterizer draws.
//!
//! `bevy_text` never runs, so a node's text is measured the way a terminal
//! renders it - by display width in cells, where a wide grapheme counts as
//! two - and taffy lays out against that measurement like any other content.
//! A font is one row tall whatever its size; what survives of a `TextFont`
//! is its weight and style, and of a colour its RGB.

use bevy_color::Color;
use bevy_ecs::entity::{Entity, EntityHashSet};
use bevy_ecs::prelude::{
    Added, Changed, ChildOf, Children, Commands, Component, DetectChanges, Has, Or, Query,
    RemovedComponents, Res, With,
};
use bevy_ecs::system::SystemParam;
use bevy_math::Vec2;
use bevy_text::{
    FontStyle, FontWeight, Justify, LineBreak, Strikethrough, TextColor, TextElement, TextFont,
    TextLayout, TextSpan, Underline,
};
use bevy_ui::widget::{Text, TextUiReader};
use bevy_ui::{AvailableSpace, ContentSize, Measure, MeasureArgs, NodeMeasure};
use plurimus_core::ratatui_core::style::{Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::extract::cell_color;
use plurimus_core::CellSize;

/// A `Text` root's spans as cell styles, kept from the last change to any
/// of them so neither measurement nor extraction walks the spans again.
#[derive(Component, Debug, Clone)]
pub(crate) struct ResolvedText {
    pub(crate) spans: Vec<Seg>,
    pub(crate) justify: Justify,
    pub(crate) is_wrapping: bool,
}

pub(crate) type Seg = (String, Style);

/// What marks a root for resolution: a change on it, a change on one of
/// its spans, or a decoration leaving either.
#[derive(SystemParam)]
pub(crate) struct TextDirt<'w, 's> {
    changed: Query<
        'w,
        's,
        Entity,
        Or<(
            Changed<Text>,
            Changed<TextSpan>,
            Changed<TextLayout>,
            Changed<TextFont>,
            Changed<TextColor>,
            Changed<Children>,
            Added<Strikethrough>,
            Added<Underline>,
        )>,
    >,
    struck: RemovedComponents<'w, 's, Strikethrough>,
    underlined: RemovedComponents<'w, 's, Underline>,
    parents: Query<'w, 's, &'static ChildOf>,
    is_root: Query<'w, 's, Entity, With<Text>>,
    cell: Res<'w, CellSize>,
}

impl TextDirt<'_, '_> {
    // A new cell size remeasures everything, every measure being in pixels.
    fn roots(&mut self) -> EntityHashSet {
        if self.cell.is_changed() {
            return self.is_root.iter().collect();
        }
        let Self {
            changed,
            struck,
            underlined,
            parents,
            is_root,
            ..
        } = self;
        let root_of = |entity: Entity| {
            std::iter::once(entity)
                .chain(parents.iter_ancestors(entity))
                .find(|&candidate| is_root.contains(candidate))
        };
        changed
            .iter()
            .chain(struck.read())
            .chain(underlined.read())
            .filter_map(root_of)
            .collect()
    }
}

pub(crate) fn resolve_bui_text(
    mut dirt: TextDirt,
    mut reader: TextUiReader,
    decorations: Query<(Has<Strikethrough>, Has<Underline>)>,
    mut roots: Query<(&TextLayout, &mut ContentSize), With<Text>>,
    mut commands: Commands,
) {
    let cell = dirt.cell.vec();
    for root in dirt.roots() {
        let Ok((layout, mut content)) = roots.get_mut(root) else {
            continue;
        };
        let spans = resolved_spans(&mut reader, &decorations, root);
        let is_wrapping = layout.linebreak != LineBreak::NoWrap;
        let min_content = wrapped_size(&spans, Some(longest_word(&spans))) * cell;
        content.set(NodeMeasure::Custom(Box::new(GraphemeMeasure {
            spans: spans.clone(),
            min_content,
            is_wrapping,
            cell,
        })));
        commands.entity(root).insert(ResolvedText {
            spans,
            justify: layout.justify,
            is_wrapping,
        });
    }
}

fn resolved_spans(
    reader: &mut TextUiReader,
    decorations: &Query<(Has<Strikethrough>, Has<Underline>)>,
    root: Entity,
) -> Vec<Seg> {
    let mut spans = Vec::new();
    for (entity, _, element) in reader.iter(root) {
        let TextElement::Text {
            text, font, color, ..
        } = element
        else {
            continue;
        };
        let (is_struck, is_underlined) = decorations.get(entity).unwrap_or_default();
        spans.push((
            text.to_owned(),
            cell_style(font, color, is_struck, is_underlined),
        ));
    }
    spans
}

// Alpha zero draws nothing, as on desktop: the rasterizer skips a hidden
// cluster rather than asking the terminal to conceal it.
pub(crate) fn cell_style(
    font: &TextFont,
    color: Color,
    is_struck: bool,
    is_underlined: bool,
) -> Style {
    let mut modifiers = Modifier::empty();
    modifiers.set(Modifier::BOLD, font.weight.0 >= FontWeight::SEMIBOLD.0);
    modifiers.set(Modifier::ITALIC, !matches!(font.style, FontStyle::Normal));
    modifiers.set(Modifier::CROSSED_OUT, is_struck);
    modifiers.set(Modifier::UNDERLINED, is_underlined);
    let fg = cell_color(color);
    modifiers.set(Modifier::HIDDEN, fg.is_none());
    let style = Style::new().add_modifier(modifiers);
    fg.map_or(style, |fg| style.fg(fg))
}

pub(crate) fn line_width(line: &[Seg]) -> usize {
    line.iter().map(|(content, _)| content.width()).sum()
}

/// Measures in pixels, as taffy asks, a grapheme being one cell wide and a
/// line one cell tall.
struct GraphemeMeasure {
    spans: Vec<Seg>,
    min_content: Vec2,
    is_wrapping: bool,
    cell: Vec2,
}

impl Measure for GraphemeMeasure {
    // `known_width` is the border box; `available_width` is already the
    // content box the text wraps inside.
    fn measure(&mut self, args: MeasureArgs<'_>) -> Vec2 {
        match args.available_width {
            AvailableSpace::Definite(width) if self.is_wrapping => {
                let columns = (width / self.cell.x).floor() as usize;
                wrapped_size(&self.spans, Some(columns)) * self.cell
            }
            AvailableSpace::MinContent if self.is_wrapping => self.min_content,
            AvailableSpace::Definite(_)
            | AvailableSpace::MinContent
            | AvailableSpace::MaxContent => wrapped_size(&self.spans, None) * self.cell,
        }
    }
}

fn wrapped_size(spans: &[Seg], limit: Option<usize>) -> Vec2 {
    let lines = wrap_spans(spans, limit);
    let width = lines.iter().map(|line| line_width(line)).max().unwrap_or(0);
    Vec2::new(width as f32, lines.len().max(1) as f32)
}

// Min-content is the longest word, as in CSS; the unwrapped line would widen
// an auto grid track past the viewport.
fn longest_word(spans: &[Seg]) -> usize {
    let (mut longest, mut current) = (0, 0);
    for (ch, _) in styled_chars(spans) {
        current = if ch.is_whitespace() {
            0
        } else {
            current + ch.width().unwrap_or(0)
        };
        longest = longest.max(current);
    }
    longest
}

/// Greedy word wrap by display width over styled spans; explicit
/// newlines respected, words wider than the limit hard-broken, wrap
/// points free to cross span boundaries.
pub(crate) fn wrap_spans(spans: &[Seg], limit: Option<usize>) -> Vec<Vec<Seg>> {
    let lines = split_newlines(spans);
    let Some(limit) = limit else {
        return lines;
    };
    let mut wrapped = Vec::new();
    for line in &lines {
        wrap_line(&split_words(line), limit.max(1), &mut wrapped);
    }
    if wrapped.is_empty() {
        wrapped.push(Vec::new());
    }
    wrapped
}

fn split_newlines(spans: &[Seg]) -> Vec<Vec<Seg>> {
    let mut lines: Vec<Vec<Seg>> = vec![Vec::new()];
    for (content, style) in spans {
        let mut parts = content.split('\n');
        if let Some(first) = parts.next()
            && !first.is_empty()
            && let Some(line) = lines.last_mut()
        {
            append_run(line, first, *style);
        }
        for part in parts {
            lines.push(Vec::new());
            if !part.is_empty()
                && let Some(line) = lines.last_mut()
            {
                append_run(line, part, *style);
            }
        }
    }
    if lines.len() > 1 && lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    lines
}

/// A styled run of segments and its display width - the accumulator for
/// both words and wrapped lines.
#[derive(Default)]
struct Run {
    segments: Vec<Seg>,
    width: usize,
}

impl Run {
    const fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    fn push_char(&mut self, ch: char, style: Style) {
        append_run(&mut self.segments, ch.encode_utf8(&mut [0u8; 4]), style);
        self.width += ch.width().unwrap_or(0);
    }

    fn push_space(&mut self) {
        if let Some(style) = self.segments.last().map(|(_, style)| *style) {
            append_run(&mut self.segments, " ", style);
            self.width += 1;
        }
    }

    fn push_word(&mut self, word: &Run) {
        for (content, style) in &word.segments {
            append_run(&mut self.segments, content, *style);
        }
        self.width += word.width;
    }

    fn take_segments(&mut self) -> Vec<Seg> {
        self.width = 0;
        core::mem::take(&mut self.segments)
    }
}

fn styled_chars(segments: &[Seg]) -> impl Iterator<Item = (char, Style)> + '_ {
    segments
        .iter()
        .flat_map(|(content, style)| content.chars().map(move |ch| (ch, *style)))
}

fn split_words(line: &[Seg]) -> Vec<Run> {
    let mut words = Vec::new();
    let mut current = Run::default();
    for (ch, style) in styled_chars(line) {
        if ch.is_whitespace() {
            if !current.is_empty() {
                words.push(core::mem::take(&mut current));
            }
        } else {
            current.push_char(ch, style);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn wrap_line(words: &[Run], limit: usize, out: &mut Vec<Vec<Seg>>) {
    let mut line = Run::default();
    for word in words {
        let needed = word.width + usize::from(!line.is_empty());
        if line.width + needed > limit && !line.is_empty() {
            out.push(line.take_segments());
        }
        if word.width > limit {
            hard_break(word, limit, &mut line, out);
            continue;
        }
        line.push_space();
        line.push_word(word);
    }
    out.push(line.take_segments());
}

fn hard_break(word: &Run, limit: usize, line: &mut Run, out: &mut Vec<Vec<Seg>>) {
    for (ch, style) in styled_chars(&word.segments) {
        let char_width = ch.width().unwrap_or(0);
        if line.width + char_width > limit && !line.is_empty() {
            out.push(line.take_segments());
        }
        line.push_char(ch, style);
    }
}

fn append_run(line: &mut Vec<Seg>, content: &str, style: Style) {
    match line.last_mut() {
        Some((existing, last)) if *last == style => existing.push_str(content),
        _ => line.push((content.to_owned(), style)),
    }
}

#[cfg(test)]
mod tests {
    use plurimus_core::ratatui_core::style::{Color, Style};

    use super::{Seg, wrap_spans};

    fn plain(text: &str) -> Vec<Seg> {
        vec![(text.to_owned(), Style::default())]
    }

    fn flat(lines: &[Vec<Seg>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| line.iter().map(|(content, _)| content.as_str()).collect())
            .collect()
    }

    #[test]
    fn wraps_by_display_width() {
        assert_eq!(
            flat(&wrap_spans(&plain("hello wide world"), Some(10))),
            ["hello wide", "world"]
        );
        assert_eq!(flat(&wrap_spans(&plain("ab\ncd"), None)), ["ab", "cd"]);
        assert_eq!(
            flat(&wrap_spans(&plain("ああああ"), Some(4))),
            ["ああ", "ああ"]
        );
        assert_eq!(flat(&wrap_spans(&plain(""), Some(5))), [""]);
    }

    #[test]
    fn wrap_points_cross_span_boundaries() {
        let red = Style::new().fg(Color::Red);
        let blue = Style::new().fg(Color::Blue);
        let spans = vec![("hello wi".to_owned(), red), ("de world".to_owned(), blue)];
        let lines = wrap_spans(&spans, Some(10));
        assert_eq!(flat(&lines), ["hello wide", "world"]);
        assert_eq!(
            lines[0],
            vec![("hello wi".to_owned(), red), ("de".to_owned(), blue)]
        );
        assert_eq!(lines[1], vec![("world".to_owned(), blue)]);
    }

    #[test]
    fn hard_break_keeps_char_styles() {
        let red = Style::new().fg(Color::Red);
        let blue = Style::new().fg(Color::Blue);
        let spans = vec![("abc".to_owned(), red), ("def".to_owned(), blue)];
        let lines = wrap_spans(&spans, Some(4));
        assert_eq!(flat(&lines), ["abcd", "ef"]);
        assert_eq!(
            lines[0],
            vec![("abc".to_owned(), red), ("d".to_owned(), blue)]
        );
    }

    #[test]
    fn newline_inside_span_splits_lines() {
        let red = Style::new().fg(Color::Red);
        let spans = vec![("a\nb".to_owned(), red)];
        assert_eq!(flat(&wrap_spans(&spans, Some(5))), ["a", "b"]);
    }
}
