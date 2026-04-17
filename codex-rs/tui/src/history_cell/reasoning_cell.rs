use super::HistoryCell;
use super::RtOptions;
use super::adaptive_wrap_lines;
use super::append_markdown;
use codex_config::types::ReasoningBlockMode;
use ratatui::prelude::Line;
use ratatui::style::Style;
use ratatui::style::Stylize;
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
        let text = self.visible_text().trim();
        if text.is_empty() {
            return Vec::new();
        }

        let mut lines: Vec<Line<'static>> = Vec::new();
        append_markdown(
            text,
            Some((width as usize).saturating_sub(2)),
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

        adaptive_wrap_lines(
            &styled_lines,
            RtOptions::new(width as usize)
                .initial_indent("• ".dim().into())
                .subsequent_indent("  ".into()),
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
