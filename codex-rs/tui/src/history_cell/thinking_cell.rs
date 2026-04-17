use super::HistoryCell;
use super::RtOptions;
use super::adaptive_wrap_lines;
use super::append_markdown;
use ratatui::prelude::Line;
use ratatui::style::Style;
use ratatui::style::Stylize;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct ThinkingBlockCell {
    text: String,
    cwd: PathBuf,
}

impl ThinkingBlockCell {
    pub(crate) fn new(cwd: &Path) -> Self {
        Self {
            text: String::new(),
            cwd: cwd.to_path_buf(),
        }
    }

    pub(crate) fn push_delta(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let text = self.text.trim();
        if text.is_empty() {
            return Vec::new();
        }

        let mut lines: Vec<Line<'static>> = Vec::new();
        append_markdown(
            text,
            Some(width as usize),
            Some(self.cwd.as_path()),
            &mut lines,
        );

        let style = Style::default().dim().italic();
        let styled_lines = lines
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

        adaptive_wrap_lines(&styled_lines, RtOptions::new(width as usize))
    }
}

impl HistoryCell for ThinkingBlockCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.lines(width)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.lines(width)
    }
}

pub(crate) fn new_active_thinking_block(cwd: &Path) -> ThinkingBlockCell {
    ThinkingBlockCell::new(cwd)
}
