use super::HistoryCell;
use super::markdown_block::MarkdownBlockFormat;
use super::markdown_block::render_markdown_block;
use codex_config::types::ReasoningBlockMode;
use ratatui::prelude::Line;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct ReasoningBlockCell {
    summary: String,
    raw: String,
    cwd: PathBuf,
    mode: ReasoningBlockMode,
}

impl ReasoningBlockCell {
    pub(crate) fn new(mode: ReasoningBlockMode, cwd: &Path) -> Self {
        Self {
            summary: String::new(),
            raw: String::new(),
            cwd: cwd.to_path_buf(),
            mode,
        }
    }

    pub(crate) fn push_summary_delta(&mut self, delta: &str) {
        self.summary.push_str(delta);
    }

    pub(crate) fn push_raw_delta(&mut self, delta: &str) {
        self.raw.push_str(delta);
    }

    pub(crate) fn push_summary_section_break(&mut self) {
        if !self.summary.is_empty() && !self.summary.ends_with("\n\n") {
            self.summary.push_str("\n\n");
        }
    }

    pub(crate) fn set_mode(&mut self, mode: ReasoningBlockMode) {
        self.mode = mode;
    }

    fn visible_text(&self) -> &str {
        if self.mode == ReasoningBlockMode::Raw && !self.raw.trim().is_empty() {
            &self.raw
        } else if !self.summary.trim().is_empty() {
            &self.summary
        } else {
            &self.raw
        }
    }

    fn lines(&self, width: u16) -> Vec<Line<'static>> {
        render_markdown_block(
            self.visible_text(),
            width,
            self.cwd.as_path(),
            MarkdownBlockFormat::Bulleted,
        )
    }
}

impl HistoryCell for ReasoningBlockCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.lines(width)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.lines(width)
    }
}

pub(crate) fn new_active_reasoning_block(
    mode: ReasoningBlockMode,
    cwd: &Path,
) -> ReasoningBlockCell {
    ReasoningBlockCell::new(mode, cwd)
}
