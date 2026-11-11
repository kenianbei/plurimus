//! Fitting a line of text to a width: the one rule every cell widget cuts
//! its text by, so a label, a row and a cell end the same way.

use std::borrow::Cow;

use plurimus_core::ratatui_core::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// What a widget draws when its text is wider than the room it has.
///
/// Set app-wide as [`UiTheme::overflow`](crate::UiTheme::overflow), and
/// applied through [`TextOverflow::fit`], which a widget family outside this
/// workspace calls to cut the way the stock widgets do.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextOverflow {
    /// Cut at the edge with nothing to show it, as ratatui draws.
    #[default]
    Clip,
    /// End the kept text in this string, `"…"` or `"..."`, in the last
    /// columns of the room.
    Ellipsis(&'static str),
}

impl TextOverflow {
    /// `line` fitted to `width` columns, measured by display width as
    /// [`Line::width`] measures.
    ///
    /// Under [`Ellipsis`](Self::Ellipsis), a line that does not fit keeps
    /// whole grapheme clusters while they leave room for the ellipsis, which
    /// takes the style of the span the cut fell in; the line's own style and
    /// alignment are kept. The result can be a column short of `width` where
    /// a wide glyph would straddle the cut, since half of one cannot be
    /// drawn. Borrowed whenever nothing is cut: under [`Clip`](Self::Clip),
    /// for a line that fits, and for a `width` narrower than the ellipsis
    /// itself, which clips as [`Clip`](Self::Clip) does.
    #[must_use]
    pub fn fit<'a>(self, line: &'a Line<'static>, width: u16) -> Cow<'a, Line<'static>> {
        let Self::Ellipsis(ellipsis) = self else {
            return Cow::Borrowed(line);
        };
        let width = usize::from(width);
        let marker = ellipsis.width();
        if line.width() <= width || marker > width {
            return Cow::Borrowed(line);
        }
        cut(line, width - marker, ellipsis).map_or(Cow::Borrowed(line), Cow::Owned)
    }
}

// `None` where the clusters fit after all: a line's width is its whole
// string's, which can differ from the sum of its clusters'.
fn cut(line: &Line<'static>, room: usize, ellipsis: &'static str) -> Option<Line<'static>> {
    let mut kept = Vec::with_capacity(line.spans.len() + 1);
    let mut used = 0;
    for span in &line.spans {
        let mut end = 0;
        for cluster in span.content.graphemes(true) {
            let next = used + cluster.width();
            if next > room {
                if end > 0 {
                    kept.push(Span::styled(span.content[..end].to_owned(), span.style));
                }
                kept.push(Span::styled(ellipsis, span.style));
                let mut fitted = Line::from(kept);
                fitted.style = line.style;
                fitted.alignment = line.alignment;
                return Some(fitted);
            }
            used = next;
            end += cluster.len();
        }
        kept.push(span.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use plurimus_core::ratatui_core::layout::Alignment;
    use plurimus_core::ratatui_core::style::{Color, Style};
    use plurimus_core::ratatui_core::text::{Line, Span};

    use super::TextOverflow;

    const ELLIPSIS: TextOverflow = TextOverflow::Ellipsis("…");

    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn fit_str(overflow: TextOverflow, content: &'static str, width: u16) -> (String, usize) {
        let line = Line::from(content);
        let fitted = overflow.fit(&line, width);
        (text(&fitted), fitted.width())
    }

    #[test]
    fn clip_and_a_line_that_fits_are_borrowed() {
        let line = Line::from("a long label");
        assert!(matches!(TextOverflow::Clip.fit(&line, 4), Cow::Borrowed(_)));
        assert!(matches!(ELLIPSIS.fit(&line, 12), Cow::Borrowed(_)));
    }

    #[test]
    fn a_cut_line_ends_in_the_ellipsis_at_the_width() {
        assert_eq!(fit_str(ELLIPSIS, "a long label", 6), ("a lon…".into(), 6));
    }

    #[test]
    fn a_wide_glyph_is_never_split_by_the_cut() {
        assert_eq!(fit_str(ELLIPSIS, "漢字漢字", 4), ("漢…".into(), 3));
    }

    #[test]
    fn a_width_narrower_than_the_ellipsis_clips() {
        let line = Line::from("label");
        let fitted = TextOverflow::Ellipsis("...").fit(&line, 2);
        assert!(matches!(fitted, Cow::Borrowed(_)));
    }

    #[test]
    fn a_wider_ellipsis_takes_its_whole_width() {
        let dots = TextOverflow::Ellipsis("...");
        assert_eq!(fit_str(dots, "a long label", 6), ("a l...".into(), 6));
    }

    #[test]
    fn spans_keep_their_styles_and_the_ellipsis_takes_the_cut_one() {
        let red = Style::new().fg(Color::Red);
        let blue = Style::new().fg(Color::Blue);
        let line = Line::from(vec![Span::styled("ab", red), Span::styled("cdef", blue)])
            .style(Style::new().bg(Color::Black))
            .alignment(Alignment::Center);
        let fitted = ELLIPSIS.fit(&line, 4);
        let spans: Vec<_> = fitted
            .spans
            .iter()
            .map(|span| (span.content.as_ref(), span.style))
            .collect();
        assert_eq!(spans, [("ab", red), ("c", blue), ("…", blue)]);
        assert_eq!(fitted.style, line.style);
        assert_eq!(fitted.alignment, Some(Alignment::Center));
    }

    #[test]
    fn a_combining_mark_stays_with_its_base() {
        let marked = "e\u{301}e\u{301}e\u{301}";
        assert_eq!(fit_str(ELLIPSIS, marked, 2), ("e\u{301}…".into(), 2));
    }

    #[test]
    fn a_cut_at_a_span_boundary_drops_the_empty_remainder() {
        let line = Line::from(vec![Span::raw("abc"), Span::raw("def")]);
        let fitted = ELLIPSIS.fit(&line, 4);
        let contents: Vec<_> = fitted
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(contents, ["abc", "…"]);
    }
}
