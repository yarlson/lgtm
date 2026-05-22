mod event;
mod format;

use std::collections::HashMap;
use std::io::IsTerminal;
use std::io::Write;

use crate::events::CodexEvent;
use crate::events::CodexItem;
use crate::events::EventKind;
use crate::events::EventPayload;
use crate::events::ItemPayload;
use crate::events::ItemStatus;
use crate::render::event::render_event;
use crate::render::format::blank;
use crate::render::format::child;
use crate::render::format::header;
use crate::render::format::header_with_marker;
use crate::render::format::text;
use crate::terminal::Color;
use crate::terminal::Text;
use crate::terminal::text_to_string;

#[derive(Debug, Clone)]
pub struct Renderer {
    color: bool,
    interactive: bool,
    active_items: HashMap<String, ActiveItem>,
    active_order: Vec<String>,
    active_line_drawn: bool,
    spinner_frame: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActiveItem {
    Command { command: String },
    WebSearch { query: String },
}

impl Renderer {
    pub fn new() -> Self {
        let color = supports_color::on_cached(supports_color::Stream::Stdout).is_some()
            && std::env::var_os("NO_COLOR").is_none();
        Self {
            color,
            interactive: std::io::stdout().is_terminal(),
            active_items: HashMap::new(),
            active_order: Vec::new(),
            active_line_drawn: false,
            spinner_frame: 0,
        }
    }

    #[cfg(test)]
    pub fn without_color() -> Self {
        Self {
            color: false,
            interactive: false,
            active_items: HashMap::new(),
            active_order: Vec::new(),
            active_line_drawn: false,
            spinner_frame: 0,
        }
    }

    pub fn phase_header(&self, phase: u32, title: &str, action: &str, log_path: &str) {
        self.emit(text(vec![
            header(
                "Ran",
                Color::LightBlue,
                format!("phase={phase:02} pass={action} title=\"{title}\""),
            ),
            child(format!("raw_jsonl {log_path}")),
            blank(),
        ]));
    }

    pub fn system(&self, message: impl Into<String>) {
        self.emit(text(vec![
            header("Snap-rs", Color::DarkGray, message),
            blank(),
        ]));
    }

    pub fn sleep(&self, seconds: u64, next_phase: u32) {
        self.emit(text(vec![
            header(
                "Waiting",
                Color::DarkGray,
                format!("{seconds}s before Phase {next_phase}"),
            ),
            blank(),
        ]));
    }

    pub fn raw_parse_error(&mut self, raw_line: &str, error: &serde_json::Error) {
        self.emit_committed(text(vec![
            header(
                "Warning",
                Color::Yellow,
                format!("json parse_error=\"{error}\""),
            ),
            child(raw_line.trim_end()),
            blank(),
        ]));
    }

    pub fn event(&mut self, event: &CodexEvent) {
        if self.handle_active_event(event) {
            return;
        }
        self.emit_committed(render_event(event, self.color));
    }

    #[cfg(test)]
    pub fn render_to_string(&self, event: &CodexEvent) -> String {
        text_to_string(render_event(event, self.color), self.color)
    }

    #[cfg(test)]
    pub fn render_stateful_to_string(&mut self, event: &CodexEvent) -> String {
        if self.handle_active_event(event) {
            return String::new();
        }
        text_to_string(render_event(event, self.color), self.color)
    }

    pub fn tick(&mut self) {
        if self.active_items.is_empty() {
            return;
        }
        self.render_active_line();
    }

    pub fn finish(&mut self) {
        self.clear_active_line();
        self.active_items.clear();
        self.active_order.clear();
    }

    fn emit(&self, rendered: Text) {
        print!("{}", text_to_string(rendered, self.color));
    }

    fn emit_committed(&mut self, rendered: Text) {
        self.clear_active_line();
        if rendered.lines.is_empty() {
            return;
        }
        self.emit(rendered);
    }

    fn handle_active_event(&mut self, event: &CodexEvent) -> bool {
        let EventPayload::Item { item } = &event.payload else {
            return false;
        };

        if is_terminal_event(event.kind, item.status) {
            self.remove_active(&item.id);
            return false;
        }

        let Some(active) = active_item(item) else {
            return false;
        };

        self.upsert_active(item.id.clone(), active);
        self.render_active_line();
        true
    }

    fn upsert_active(&mut self, id: String, active: ActiveItem) {
        if !self.active_items.contains_key(&id) {
            self.active_order.push(id.clone());
        }
        self.active_items.insert(id, active);
    }

    fn remove_active(&mut self, id: &str) {
        self.active_items.remove(id);
        self.active_order.retain(|candidate| candidate != id);
    }

    fn render_active_line(&mut self) {
        if !self.interactive {
            return;
        }
        let Some(active) = self.current_active().cloned() else {
            self.clear_active_line();
            return;
        };

        const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let marker = SPINNER[self.spinner_frame % SPINNER.len()];
        self.spinner_frame = self.spinner_frame.wrapping_add(1);

        let line = match &active {
            ActiveItem::Command { command } => {
                header_with_marker(marker, "Running", Color::Cyan, command)
            }
            ActiveItem::WebSearch { query } if query.trim().is_empty() => {
                header_with_marker(marker, "Searching the web", Color::LightBlue, "")
            }
            ActiveItem::WebSearch { query } => {
                header_with_marker(marker, "Searching the web", Color::LightBlue, query)
            }
        };
        let rendered = text_to_string(text(vec![line]), self.color)
            .trim_end_matches('\n')
            .to_string();
        print!("\r\x1b[2K{rendered}");
        let _ = std::io::stdout().flush();
        self.active_line_drawn = true;
    }

    fn current_active(&self) -> Option<&ActiveItem> {
        self.active_order
            .iter()
            .rev()
            .find_map(|id| self.active_items.get(id))
    }

    fn clear_active_line(&mut self) {
        if self.active_line_drawn {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
            self.active_line_drawn = false;
        }
    }
}

fn is_terminal_event(kind: EventKind, status: ItemStatus) -> bool {
    matches!(kind, EventKind::ItemCompleted)
        || matches!(
            status,
            ItemStatus::Completed | ItemStatus::Failed | ItemStatus::Declined
        )
}

fn active_item(item: &CodexItem) -> Option<ActiveItem> {
    if !is_progress_status(item.status) {
        return None;
    }

    match &item.payload {
        ItemPayload::CommandExecution { command, .. } => Some(ActiveItem::Command {
            command: command.clone(),
        }),
        ItemPayload::WebSearch { query } => Some(ActiveItem::WebSearch {
            query: query.clone(),
        }),
        _ => None,
    }
}

fn is_progress_status(status: ItemStatus) -> bool {
    matches!(status, ItemStatus::InProgress)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CodexEvent;

    #[test]
    fn renders_command_completion_as_append_only_run_block() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran cargo check"));
        assert!(rendered.contains("  └ Finished"));
        assert!(!rendered.contains("exit=0"));
        assert!(!rendered.contains("| Finished"));
    }

    #[test]
    fn skips_progress_only_command_rows() {
        let event = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"item_0","type":"command_execution","command":"cargo check","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn stateful_renderer_commits_command_once_after_progress() {
        let started = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"item_0","type":"command_execution","command":"cargo check","status":"in_progress"}}"#,
        )
        .unwrap();
        let completed = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();
        let mut renderer = Renderer::without_color();

        assert!(renderer.render_stateful_to_string(&started).is_empty());
        assert!(renderer.active_items.contains_key("item_0"));

        let rendered = renderer.render_stateful_to_string(&completed);

        assert!(!renderer.active_items.contains_key("item_0"));
        assert_eq!(rendered.matches("• Ran cargo check").count(), 1);
        assert!(rendered.contains("  └ Finished"));
    }

    #[test]
    fn skips_progress_command_rows_with_blank_output() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn renders_command_with_missing_status_instead_of_treating_it_as_progress() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":""}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran cargo check"));
        assert!(rendered.contains("  └ no output"));
    }

    #[test]
    fn renders_completed_command_without_output_once() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"git status --short","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran git status --short"));
        assert!(rendered.contains("  └ no output"));
    }

    #[test]
    fn skips_empty_web_search_rows() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_0","type":"web_search","query":"","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn skips_progress_web_search_without_query() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_0","type":"web_search","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn renders_web_search_with_query() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"web_search","query":"clap derive Parser docs","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Searched clap derive Parser docs"));
    }

    #[test]
    fn stateful_renderer_commits_web_search_once_after_progress() {
        let started = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"search_0","type":"web_search","query":"","status":"in_progress"}}"#,
        )
        .unwrap();
        let completed = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"search_0","type":"web_search","query":"clap derive Parser docs","status":"completed"}}"#,
        )
        .unwrap();
        let mut renderer = Renderer::without_color();

        assert!(renderer.render_stateful_to_string(&started).is_empty());
        assert!(renderer.active_items.contains_key("search_0"));

        let rendered = renderer.render_stateful_to_string(&completed);

        assert!(!renderer.active_items.contains_key("search_0"));
        assert_eq!(
            rendered
                .matches("• Searched clap derive Parser docs")
                .count(),
            1
        );
    }

    #[test]
    fn renders_agent_message_as_codex_block() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"**Implemented** Phase 6\n\nChanged:\n- Makefile"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("────────────────"));
        assert!(rendered.contains("• Codex"));
        assert!(rendered.contains("  Implemented Phase 6"));
        assert!(rendered.contains("  Changed:"));
        assert!(rendered.contains("Makefile"));
        assert!(!rendered.contains("**Implemented**"));
        assert!(!rendered.contains("begin"));
        assert!(!rendered.contains("end lines="));
        assert!(!rendered.contains("| Implemented"));
    }

    #[test]
    fn renders_todo_list_as_plan_block() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":true},{"text":"Patch","completed":false}]}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Updated Plan"));
        assert!(rendered.contains("  ✓ Inspect"));
        assert!(rendered.contains("  □ Patch"));
        assert!(!rendered.contains("-- checklist"));
    }

    #[test]
    fn renders_unknown_item_from_payload_type() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"new_tool_call","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Event new_tool_call"));
    }

    #[test]
    fn renders_malformed_known_item_as_warning() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(
            rendered.contains(
                "• Warning command_execution malformed: command_execution missing command"
            )
        );
    }

    #[test]
    fn colors_action_and_keeps_child_output_readable() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        let mut renderer = Renderer::without_color();
        renderer.color = true;
        let rendered = renderer.render_to_string(&event);

        assert!(rendered.contains("\x1b[32m•"));
        assert!(rendered.contains("\x1b[1;32mRan"));
        assert!(rendered.contains("\x1b[90mFinished"));
        assert!(!rendered.contains("\x1b[2;90mFinished"));
    }

    #[test]
    fn colors_completed_plan_items_as_dim_struck_through() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":true},{"text":"Patch","completed":false}]}}"#,
        )
        .unwrap();

        let mut renderer = Renderer::without_color();
        renderer.color = true;
        let rendered = renderer.render_to_string(&event);

        assert!(rendered.contains("\x1b[2;9;90mInspect"));
        assert!(rendered.contains("□ "));
        assert!(!rendered.contains("\x1b[2;9;37mPatch"));
    }

    #[test]
    fn renders_representative_unicode_transcript_shape() {
        let events = [
            r#"{"type":"item.completed","item":{"id":"cmd","type":"command_execution","command":"make check","aggregated_output":"cargo fmt --all --check\ncargo clippy --all-targets --all-features -- -D warnings\ncargo test --all-features\nFinished tests\ncargo build --all-targets --all-features\nFinished build\n","exit_code":0,"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"todo","type":"todo_list","items":[{"text":"Make target setup preflight non-mutating before Git init and skill install","completed":true},{"text":"Fix repo ignore drift and run full verification","completed":false}],"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"file","type":"file_change","changes":[{"path":".gitignore","kind":"update"}],"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"bad","type":"command_execution","status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"msg","type":"agent_message","text":"Implemented all five fixes.\n\nVerification: make check passed."}}"#,
        ];
        let rendered = events
            .into_iter()
            .map(|event| {
                Renderer::without_color()
                    .render_to_string(&CodexEvent::parse(event).expect("event"))
            })
            .collect::<String>();

        assert!(rendered.contains("• Ran make check"));
        assert!(rendered.contains("    … +2 lines hidden"));
        assert!(rendered.contains("• Updated Plan"));
        assert!(rendered.contains("  ✓ Make target setup preflight"));
        assert!(rendered.contains("  □ Fix repo ignore drift"));
        assert!(rendered.contains("• Edited .gitignore"));
        assert!(rendered.contains("  └ ~ .gitignore"));
        assert!(rendered.contains("• Warning command_execution malformed"));
        assert!(rendered.contains("• Codex"));
        assert!(rendered.contains("  Verification: make check passed."));
    }
}
