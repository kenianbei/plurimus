//! Fontless text for `bevy_ui` nodes: grapheme-width measurement through
//! `ContentSize`, no font machinery involved.
//!
//! `bevy_text` never runs, so a node's text is measured the way a terminal
//! actually renders it - by display width in cells, where a wide grapheme
//! counts as two - and taffy lays out against that measurement like any other
//! content. [`Text`] carries styled [`TextSpan`]s that patch the node's
//! [`TextStyle`], which is what lets one node mix styles without `bevy_text`'s
//! section machinery.

use bevy_ecs::prelude::{Changed, Component, Query};
use bevy_math::Vec2;
use bevy_ui::measurement::AvailableSpace;
use bevy_ui::{ContentSize, Measure, MeasureArgs, NodeMeasure};
use plurimus_core::ratatui_core::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Text content of a `bevy_ui` node, measured and drawn in cells.
#[derive(Component, Debug, Clone, Default, PartialEq)]
#[require(bevy_ui::Node, ContentSize, TextStyle)]
pub struct Text(pub Vec<TextSpan>);

/// One styled run inside a [`Text`]; its style patches the node's
/// [`TextStyle`].
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct TextSpan {
    /// The span's text content.
    pub content: String,
    /// Patch applied over the node's [`TextStyle`] for this span.
    pub style: Style,
}

impl TextSpan {
    /// A span with an explicit style patch.
    #[must_use]
    pub fn new(content: impl Into<String>, style: Style) -> Self {
        Self {
            content: content.into(),
            style,
        }
    }
}

impl From<&str> for Text {
    fn from(content: &str) -> Self {
        Self::from(content.to_owned())
    }
}

impl From<String> for Text {
    fn from(content: String) -> Self {
        Self(vec![TextSpan {
            content,
            style: Style::default(),
        }])
    }
}

/// Base style the rasterizer applies to the node's text cells; span
/// styles patch over it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TextStyle(pub Style);

pub(crate) type Seg = (String, Style);

/// Span contents with `base` patched under each span's style.
pub(crate) fn resolved_spans(text: &Text, base: Style) -> Vec<Seg> {
    text.0
        .iter()
        .map(|span| (span.content.clone(), base.patch(span.style)))
        .collect()
}

fn line_width(line: &[Seg]) -> usize {
    line.iter().map(|(content, _)| content.width()).sum()
}

struct GraphemeMeasure {
    spans: Vec<Seg>,
}

impl Measure for GraphemeMeasure {
    fn measure(&mut self, args: MeasureArgs<'_>) -> Vec2 {
        let limit = args.known_width.or(match args.available_width {
            AvailableSpace::Definite(width) => Some(width),
            AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
        });
        let lines = wrap_spans(&self.spans, limit.map(|width| width.floor() as usize));
        let width = lines.iter().map(|line| line_width(line)).max().unwrap_or(0);
        Vec2::new(width as f32, lines.len().max(1) as f32)
    }
}

pub(crate) fn measure_bui_text(mut texts: Query<(&Text, &mut ContentSize), Changed<Text>>) {
    for (text, mut content) in &mut texts {
        content.set(NodeMeasure::Custom(Box::new(GraphemeMeasure {
            spans: resolved_spans(text, Style::default()),
        })));
    }
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
