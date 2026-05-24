use lgtm_app_server_client::{
    DynamicToolCall, ItemKind, McpToolCall, TranscriptItem, TranscriptItemData,
};

use crate::{
    RenderOptions, Verbosity,
    markdown::markdown_lines,
    style::{Color, Line, Span, Style, Symbol, render_lines},
};

pub(crate) struct ItemRenderer<'a> {
    options: &'a RenderOptions,
}

impl<'a> ItemRenderer<'a> {
    pub(crate) fn new(options: &'a RenderOptions) -> Self {
        Self { options }
    }

    pub(crate) fn render_item(&self, item: &TranscriptItem) -> String {
        match item.data() {
            TranscriptItemData::AgentMessage { text } => {
                let Some(message) = text.as_deref().or_else(|| non_empty(item.output_text()))
                else {
                    return String::new();
                };
                self.render_agent_message(message, item)
            }
            TranscriptItemData::Plan { text } => {
                let Some(message) = text.as_deref().or_else(|| non_empty(item.output_text()))
                else {
                    return String::new();
                };
                self.render_plan_item(message, item)
            }
            TranscriptItemData::Reasoning { .. } => String::new(),
            TranscriptItemData::CommandExecution(command) => self.render_command(item, command),
            TranscriptItemData::FileChange { changes } => self.render_file_changes(item, changes),
            TranscriptItemData::WebSearch(search) => self.render_web_search(item, &search.query),
            TranscriptItemData::McpToolCall(tool) => self.render_mcp_tool_call(item, tool),
            TranscriptItemData::DynamicToolCall(tool) => self.render_dynamic_tool_call(item, tool),
            TranscriptItemData::Other { details, output } => {
                self.render_unknown(item, details, output)
            }
        }
    }

    fn render_plan_item(&self, message: &str, item: &TranscriptItem) -> String {
        if item.is_in_progress() {
            return String::new();
        }
        let body = markdown_lines(message, self.options.markdown);
        if body.is_empty() {
            return String::new();
        }
        let mut lines = vec![self.header("Plan", Color::Blue, "")];
        lines.extend(body);
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    fn render_agent_message(&self, message: &str, item: &TranscriptItem) -> String {
        if item.is_in_progress() {
            return String::new();
        }
        let body = markdown_lines(message, self.options.markdown);
        if body.is_empty() {
            return String::new();
        }

        let mut lines = vec![
            self.separator(),
            Line::blank(),
            self.header("Codex", Color::Magenta, ""),
        ];
        lines.extend(body);
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    fn render_command(
        &self,
        item: &TranscriptItem,
        command: &lgtm_app_server_client::CommandExecution,
    ) -> String {
        if item.is_in_progress() {
            return String::new();
        }

        let output = non_empty(&command.output).unwrap_or_else(|| item.output_text());
        if output.trim().is_empty()
            && item.status() == Some("completed")
            && self.options.verbosity != Verbosity::Verbose
        {
            return self.render_lines(vec![
                self.header("Ran", self.status_color(item), &command.command),
                Line::blank(),
            ]);
        }

        let mut lines = vec![self.header("Ran", self.status_color(item), &command.command)];
        lines.extend(self.output_lines(output, command.exit_code));
        if let Some(exit_code) = command.exit_code.filter(|code| *code != 0) {
            lines.push(self.continuation(format!("exit={exit_code}")));
        }
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    fn render_file_changes(
        &self,
        item: &TranscriptItem,
        changes: &[lgtm_app_server_client::FileChange],
    ) -> String {
        if item.is_in_progress() || changes.is_empty() {
            return String::new();
        }

        let title = if changes.len() == 1 {
            changes[0].path.as_str()
        } else {
            "files"
        };
        let mut lines = vec![self.header("Edited", self.status_color(item), title)];
        for change in changes {
            let marker = match change.kind.as_str() {
                "add" => "+",
                "delete" => "-",
                _ => "~",
            };
            let color = match marker {
                "+" => Color::Green,
                "-" => Color::Red,
                _ => Color::Yellow,
            };
            lines.push(Line::new(vec![
                Span::raw("  "),
                Span::styled(self.symbol(Symbol::Branch), Style::fg(Color::DarkGray)),
                Span::styled(marker, Style::fg(color)),
                Span::raw(" "),
                Span::styled(change.path.clone(), Style::fg(color)),
            ]));
        }
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    fn render_web_search(&self, item: &TranscriptItem, query: &str) -> String {
        if item.is_in_progress() || query.trim().is_empty() || query == "<empty query>" {
            return String::new();
        }

        let mut lines = vec![self.header("Searched", Color::Blue, query)];
        if self.options.verbosity == Verbosity::Verbose
            && let TranscriptItemData::WebSearch(search) = item.data()
            && let Some(action) = &search.action
        {
            lines.push(self.child(format!("action: {action}")));
        }
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    fn render_mcp_tool_call(&self, item: &TranscriptItem, tool: &McpToolCall) -> String {
        if item.is_in_progress() {
            return String::new();
        }
        let title = format!("mcp {}/{}", tool.server, tool.tool);
        let has_evidence =
            non_empty(&tool.result).is_some() || tool.error.is_some() || is_problem_status(item);
        if !has_evidence && self.options.verbosity != Verbosity::Verbose {
            return String::new();
        }

        let mut lines = vec![self.header("Ran", self.status_color(item), title)];
        if self.options.verbosity == Verbosity::Verbose {
            push_tool_detail(&mut lines, self, "duration_ms", tool.duration_ms);
            push_optional_detail(&mut lines, self, "arguments", tool.arguments.as_deref());
            push_optional_detail(&mut lines, self, "error", tool.error.as_deref());
        } else if let Some(error) = &tool.error {
            lines.push(self.child(error));
        }
        if !tool.result.trim().is_empty() {
            lines.extend(self.output_lines(&tool.result, None));
        }
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    fn render_dynamic_tool_call(&self, item: &TranscriptItem, tool: &DynamicToolCall) -> String {
        if item.is_in_progress() {
            return String::new();
        }
        let title = match &tool.namespace {
            Some(namespace) => format!("tool {namespace}/{}", tool.tool),
            None => format!("tool {}", tool.tool),
        };
        let failed_success = tool.success == Some(false);
        let has_evidence =
            non_empty(&tool.content).is_some() || failed_success || is_problem_status(item);
        if !has_evidence && self.options.verbosity != Verbosity::Verbose {
            return String::new();
        }

        let mut lines = vec![self.header("Ran", self.status_color(item), title)];
        if self.options.verbosity == Verbosity::Verbose {
            if let Some(success) = tool.success {
                lines.push(self.child(format!("success: {success}")));
            }
            push_tool_detail(&mut lines, self, "duration_ms", tool.duration_ms);
            push_optional_detail(&mut lines, self, "arguments", tool.arguments.as_deref());
        } else if failed_success {
            lines.push(self.child("success: false"));
        }
        if !tool.content.trim().is_empty() {
            lines.extend(self.output_lines(&tool.content, None));
        }
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    fn render_unknown(&self, item: &TranscriptItem, details: &[String], output: &str) -> String {
        if item.is_in_progress() {
            return String::new();
        }

        let mut lines = vec![self.header("Event", Color::DarkGray, &item.title)];
        for detail in details {
            lines.push(self.child(detail));
        }
        if !output.trim().is_empty() {
            lines.extend(self.output_lines(output, None));
        }
        lines.push(Line::blank());
        self.render_lines(lines)
    }

    pub(crate) fn header(
        &self,
        action: &'static str,
        color: Color,
        message: impl Into<String>,
    ) -> Line {
        let message = message.into();
        let mut spans = vec![
            Span::styled(self.symbol(Symbol::Bullet), Style::fg(Color::Green)),
            Span::raw(" "),
            Span::styled(action, Style::fg(color).bold()),
        ];
        if !message.is_empty() {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(message, Style::fg(Color::Gray)));
        }
        Line::new(spans)
    }

    pub(crate) fn separator(&self) -> Line {
        Line::new(vec![Span::styled(
            self.symbol(Symbol::Rule),
            Style::fg(Color::DarkGray).dim(),
        )])
    }

    pub(crate) fn child(&self, message: impl Into<String>) -> Line {
        Line::new(vec![
            Span::raw("  "),
            Span::styled(self.symbol(Symbol::Branch), Style::fg(Color::DarkGray)),
            Span::styled(message.into(), Style::fg(Color::DarkGray)),
        ])
    }

    fn continuation(&self, message: impl Into<String>) -> Line {
        Line::new(vec![
            Span::raw("    "),
            Span::styled(message.into(), Style::fg(Color::DarkGray)),
        ])
    }

    pub(crate) fn status_color(&self, item: &TranscriptItem) -> Color {
        match item.status() {
            Some("completed") => Color::Green,
            Some("failed") => Color::Red,
            Some("declined") => Color::Yellow,
            Some("inProgress") => Color::Cyan,
            _ => Color::DarkGray,
        }
    }

    pub(crate) fn render_lines(&self, lines: Vec<Line>) -> String {
        render_lines(lines, self.options)
    }

    pub(crate) fn symbol(&self, symbol: Symbol) -> String {
        symbol.render(self.options.charset).to_string()
    }

    fn output_lines(&self, output: &str, exit_code: Option<i64>) -> Vec<Line> {
        if output.trim().is_empty() {
            if exit_code.is_some_and(|code| code != 0)
                || self.options.verbosity == Verbosity::Verbose
            {
                return vec![self.child("no output")];
            }
            return Vec::new();
        }

        let clipped = clip_chars(output, self.options.max_output_chars);
        let output_lines = clipped.lines().collect::<Vec<_>>();
        let mut lines = Vec::new();
        for (index, line) in output_lines
            .iter()
            .take(self.options.max_output_lines)
            .enumerate()
        {
            if index == 0 {
                lines.push(self.child(line.trim_end()));
            } else {
                lines.push(self.continuation(line.trim_end()));
            }
        }

        let hidden = output_lines
            .len()
            .saturating_sub(self.options.max_output_lines);
        if hidden > 0 {
            lines.push(self.continuation(format!(
                "{} +{hidden} lines hidden",
                self.symbol(Symbol::Ellipsis)
            )));
        }
        if clipped.len() < output.len() {
            lines.push(self.continuation(format!(
                "{} output truncated",
                self.symbol(Symbol::Ellipsis)
            )));
        }
        lines
    }
}

fn push_tool_detail(
    lines: &mut Vec<Line>,
    renderer: &ItemRenderer<'_>,
    label: &str,
    value: Option<u64>,
) {
    if let Some(value) = value {
        lines.push(renderer.child(format!("{label}: {value}")));
    }
}

fn push_optional_detail(
    lines: &mut Vec<Line>,
    renderer: &ItemRenderer<'_>,
    label: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        lines.push(renderer.child(format!("{label}: {value}")));
    }
}

fn non_empty(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}

fn is_problem_status(item: &TranscriptItem) -> bool {
    matches!(item.status(), Some("failed" | "declined"))
}

fn clip_chars(value: &str, max_chars: usize) -> &str {
    if max_chars == 0 {
        return "";
    }
    match value.char_indices().nth(max_chars) {
        Some((index, _)) => &value[..index],
        None => value,
    }
}

pub(crate) fn item_is_final(item: &TranscriptItem) -> bool {
    item.is_final()
        || item.message_text().is_some()
        || item.item_kind() == ItemKind::Other
        || matches!(item.data(), TranscriptItemData::Other { .. })
}
