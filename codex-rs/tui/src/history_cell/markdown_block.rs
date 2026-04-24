use super::RtOptions;
use super::adaptive_wrap_lines;
use super::append_markdown;
use ratatui::prelude::Line;
use ratatui::style::Style;
use ratatui::style::Stylize;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MarkdownBlockFormat {
    Plain,
    Bulleted,
}

pub(super) fn render_markdown_block(
    text: &str,
    width: u16,
    cwd: &Path,
    format: MarkdownBlockFormat,
) -> Vec<Line<'static>> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let width = match format {
        MarkdownBlockFormat::Plain => usize::from(width),
        MarkdownBlockFormat::Bulleted => {
            let Some(width) =
                crate::width::usable_content_width_u16(width, /*reserved_cols*/ 2)
            else {
                return vec![Line::from(vec!["• ".dim()])];
            };
            width
        }
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    append_markdown(text, Some(width), Some(cwd), &mut lines);

    let style = Style::default().dim().italic();
    let lines = lines
        .into_iter()
        .map(|mut line| {
            line.spans = line
                .spans
                .into_iter()
                .map(|span| span.patch_style(style))
                .collect();
            line
        })
        .collect::<Vec<_>>();

    let options = match format {
        MarkdownBlockFormat::Plain => RtOptions::new(width),
        MarkdownBlockFormat::Bulleted => RtOptions::new(width)
            .initial_indent("• ".dim().into())
            .subsequent_indent("  ".into()),
    };

    adaptive_wrap_lines(&lines, options)
}
