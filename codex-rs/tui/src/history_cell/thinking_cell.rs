use super::HistoryCell;
use super::markdown_block::MarkdownBlockFormat;
use super::markdown_block::render_markdown_block;
use ratatui::prelude::Line;
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
        render_markdown_block(
            &self.text,
            width,
            self.cwd.as_path(),
            MarkdownBlockFormat::Plain,
        )
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
