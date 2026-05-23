mod event;
mod format;
mod markdown;
mod spinner;

use std::collections::HashMap;
use std::io::IsTerminal;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

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
use crate::render::format::text;
use crate::terminal::Color;
use crate::terminal::Text;
use crate::terminal::text_to_string;

pub(crate) use spinner::Spinner;
pub(crate) use spinner::random_text as random_spinner_text;

#[derive(Debug, Clone)]
pub struct Renderer {
    color: bool,
    interactive: bool,
    active_items: HashMap<String, ActiveItem>,
    active_order: Vec<String>,
    active_line_drawn: bool,
    cursor_hidden: bool,
    spinner_frame: usize,
    spinner_ticks: usize,
    phase_status: Option<PhaseStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActiveItem {
    Command,
    FileChange { count: usize },
    WebSearch,
}

#[derive(Debug, Clone)]
struct PhaseStatus {
    phase: u32,
    action: String,
    verb: &'static str,
    started_at: Instant,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            color: stdout_color_enabled(),
            interactive: std::io::stdout().is_terminal(),
            active_items: HashMap::new(),
            active_order: Vec::new(),
            active_line_drawn: false,
            cursor_hidden: false,
            spinner_frame: 0,
            spinner_ticks: 0,
            phase_status: None,
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
            cursor_hidden: false,
            spinner_frame: 0,
            spinner_ticks: 0,
            phase_status: None,
        }
    }

    pub fn phase_header(&mut self, phase: u32, title: &str, action: &str) {
        self.phase_status = Some(PhaseStatus {
            phase,
            action: action.to_string(),
            verb: random_spinner_text(),
            started_at: Instant::now(),
        });
        self.emit(text(vec![
            header(
                "Phase",
                Color::LightBlue,
                format!("{phase:02} {}: {title}", phase_action_label(action)),
            ),
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
        self.refresh_spinner_verb();
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
        self.refresh_spinner_verb();
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
            self.render_phase_status_line();
            return;
        }
        self.render_active_line();
    }

    pub fn finish(&mut self) {
        self.clear_active_line();
        self.active_items.clear();
        self.active_order.clear();
        self.phase_status = None;
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

        let Some(id) = active_key(item) else {
            return false;
        };

        if is_terminal_event(event.kind, item.status) {
            self.remove_active(id);
            return false;
        }

        let Some(active) = active_item(item) else {
            return false;
        };

        self.upsert_active(id.to_string(), active);
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

        let label = match active {
            ActiveItem::Command => self.phase_activity_label("running command"),
            ActiveItem::FileChange { count: 1 } => self.phase_activity_label("editing file"),
            ActiveItem::FileChange { count } => {
                self.phase_activity_label(format!("editing {count} files"))
            }
            ActiveItem::WebSearch => self.phase_activity_label("searching the web"),
        };
        self.render_spinner_line(&label, self.phase_elapsed());
    }

    fn render_phase_status_line(&mut self) {
        if !self.interactive {
            return;
        }
        let Some(status) = self.phase_status.clone() else {
            self.clear_active_line();
            return;
        };

        self.render_spinner_line(
            format!(
                "{} Phase {} {}",
                status.verb,
                status.phase,
                phase_action_label(&status.action)
            ),
            status.started_at.elapsed(),
        );
    }

    fn render_spinner_line(&mut self, label: impl AsRef<str>, elapsed: Duration) {
        self.render_spinner_line_for_width(label, elapsed, spinner::terminal_width());
    }

    fn render_spinner_line_for_width(
        &mut self,
        label: impl AsRef<str>,
        elapsed: Duration,
        width: u16,
    ) {
        let frame = self.next_spinner_frame();
        let Some(rendered) = spinner::line_for_width(label.as_ref(), frame, elapsed, width) else {
            self.clear_active_line();
            return;
        };
        self.hide_cursor();
        print!("\r\x1b[2K{rendered}");
        let _ = std::io::stdout().flush();
        self.active_line_drawn = true;
    }

    fn hide_cursor(&mut self) {
        if self.interactive && !self.cursor_hidden && spinner::activate_terminal().unwrap_or(false)
        {
            self.cursor_hidden = true;
        }
    }

    fn next_spinner_frame(&mut self) -> &'static str {
        let frame = spinner::frame(self.spinner_frame);
        self.spinner_ticks = self.spinner_ticks.wrapping_add(1);
        if self.spinner_ticks.is_multiple_of(3) {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
        frame
    }

    fn phase_elapsed(&self) -> Duration {
        self.phase_status
            .as_ref()
            .map(|status| status.started_at.elapsed())
            .unwrap_or_default()
    }

    fn phase_activity_label(&self, activity: impl AsRef<str>) -> String {
        match &self.phase_status {
            Some(status) => format!(
                "{} Phase {} {} - {}",
                status.verb,
                status.phase,
                phase_action_label(&status.action),
                activity.as_ref()
            ),
            None => activity.as_ref().to_string(),
        }
    }

    fn refresh_spinner_verb(&mut self) {
        if let Some(status) = &mut self.phase_status {
            status.verb = spinner::random_text_except(status.verb);
        }
    }

    fn current_active(&self) -> Option<&ActiveItem> {
        for priority in [
            ActivePriority::Command,
            ActivePriority::FileChange,
            ActivePriority::WebSearch,
        ] {
            if let Some(active) = self
                .active_order
                .iter()
                .rev()
                .filter_map(|id| self.active_items.get(id))
                .find(|active| active.priority() == priority)
            {
                return Some(active);
            }
        }
        None
    }

    fn clear_active_line(&mut self) {
        if self.active_line_drawn || self.cursor_hidden {
            if self.cursor_hidden {
                spinner::deactivate_terminal();
                self.cursor_hidden = false;
            } else {
                print!("\r\x1b[2K");
                let _ = std::io::stdout().flush();
            }
            self.active_line_drawn = false;
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.clear_active_line();
    }
}

pub(crate) fn plan_message_to_string(message: &str) -> String {
    markdown::markdown_to_string(message, stdout_color_enabled())
}

fn stdout_color_enabled() -> bool {
    supports_color::on_cached(supports_color::Stream::Stdout).is_some()
        && std::env::var_os("NO_COLOR").is_none()
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
        ItemPayload::CommandExecution { .. } => Some(ActiveItem::Command),
        ItemPayload::FileChange { changes } if !changes.is_empty() => {
            Some(ActiveItem::FileChange {
                count: changes.len(),
            })
        }
        ItemPayload::WebSearch { .. } => Some(ActiveItem::WebSearch),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivePriority {
    Command,
    FileChange,
    WebSearch,
}

impl ActiveItem {
    fn priority(&self) -> ActivePriority {
        match self {
            ActiveItem::Command => ActivePriority::Command,
            ActiveItem::FileChange { .. } => ActivePriority::FileChange,
            ActiveItem::WebSearch => ActivePriority::WebSearch,
        }
    }
}

fn active_key(item: &CodexItem) -> Option<&str> {
    let id = item.id.trim();
    (!id.is_empty()).then_some(id)
}

fn is_progress_status(status: ItemStatus) -> bool {
    matches!(status, ItemStatus::InProgress)
}

fn phase_action_label(action: &str) -> &str {
    match action {
        "implement" => "implementation",
        "validate" => "validation",
        "review" => "review",
        other => other,
    }
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
    fn hides_codex_protocol_lifecycle_events() {
        let thread =
            CodexEvent::parse(r#"{"type":"thread.started","thread_id":"thread-test"}"#).unwrap();
        let turn = CodexEvent::parse(r#"{"type":"turn.started"}"#).unwrap();
        let renderer = Renderer::without_color();

        assert!(renderer.render_to_string(&thread).is_empty());
        assert!(renderer.render_to_string(&turn).is_empty());
    }

    #[test]
    fn hides_turn_completion_usage_accounting() {
        let event = CodexEvent::parse(
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":4}}"#,
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
    fn renders_completed_command_without_empty_output_noise() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"git status --short","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran git status --short"));
        assert!(!rendered.contains("no output"));
    }

    #[test]
    fn renders_failed_command_without_output_as_failure_evidence() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo test","exit_code":101,"status":"failed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran cargo test"));
        assert!(rendered.contains("  └ no output"));
        assert!(rendered.contains("    exit=101"));
    }

    #[test]
    fn hides_in_progress_command_rows_even_with_output() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"type":"command_execution","command":"cargo test","aggregated_output":"partial output","status":"in_progress"}}"#,
        )
        .unwrap();

        let mut renderer = Renderer::without_color();
        let rendered = renderer.render_stateful_to_string(&event);

        assert!(rendered.is_empty());
        assert!(renderer.active_items.is_empty());
    }

    #[test]
    fn renders_no_id_failed_command_as_failure_evidence() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"type":"command_execution","command":"cargo test","exit_code":101,"status":"failed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran cargo test"));
        assert!(rendered.contains("  └ no output"));
        assert!(rendered.contains("    exit=101"));
    }

    #[test]
    fn hides_internal_reasoning_placeholders() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"hidden\nreasoning","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn reasoning_does_not_create_active_state() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"reasoning_0","type":"reasoning","status":"in_progress"}}"#,
        )
        .unwrap();
        let mut renderer = Renderer::without_color();

        let rendered = renderer.render_stateful_to_string(&event);

        assert!(rendered.is_empty());
        assert!(renderer.active_items.is_empty());
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
    fn web_search_active_state_does_not_store_query() {
        let event = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"search_0","type":"web_search","query":"secret query","status":"in_progress"}}"#,
        )
        .unwrap();
        let mut renderer = Renderer::without_color();

        let rendered = renderer.render_stateful_to_string(&event);

        assert!(rendered.is_empty());
        assert_eq!(
            renderer.active_items.get("search_0"),
            Some(&ActiveItem::WebSearch)
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
    fn hides_in_progress_todo_list_rows() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":false}],"status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn in_progress_todo_list_does_not_create_active_state() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"todo_0","type":"todo_list","items":[{"text":"Inspect","completed":false}],"status":"in_progress"}}"#,
        )
        .unwrap();
        let mut renderer = Renderer::without_color();

        let rendered = renderer.render_stateful_to_string(&event);

        assert!(rendered.is_empty());
        assert!(renderer.active_items.is_empty());
    }

    #[test]
    fn keeps_completed_todo_list_rows_visible() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":true}],"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Updated Plan"));
        assert!(rendered.contains("  ✓ Inspect"));
    }

    #[test]
    fn hides_in_progress_file_change_rows() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"file_0","type":"file_change","changes":[{"path":"src/lib.rs","kind":"update"}],"status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn keeps_completed_file_change_rows_visible() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"file_0","type":"file_change","changes":[{"path":"src/lib.rs","kind":"update"}],"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Edited src/lib.rs"));
        assert!(rendered.contains("  └ ~ src/lib.rs"));
    }

    #[test]
    fn empty_file_change_does_not_render_or_create_active_state() {
        let in_progress = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"file_0","type":"file_change","changes":[],"status":"in_progress"}}"#,
        )
        .unwrap();
        let completed = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"file_0","type":"file_change","changes":[],"status":"completed"}}"#,
        )
        .unwrap();
        let mut renderer = Renderer::without_color();

        assert!(renderer.render_stateful_to_string(&in_progress).is_empty());
        assert!(renderer.active_items.is_empty());
        assert!(renderer.render_stateful_to_string(&completed).is_empty());
    }

    #[test]
    fn active_command_has_priority_over_file_change_and_search() {
        let command = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"cmd_0","type":"command_execution","command":"cargo test","status":"in_progress"}}"#,
        )
        .unwrap();
        let file = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"file_0","type":"file_change","changes":[{"path":"src/lib.rs","kind":"update"}],"status":"in_progress"}}"#,
        )
        .unwrap();
        let search = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"search_0","type":"web_search","query":"secret","status":"in_progress"}}"#,
        )
        .unwrap();
        let mut renderer = Renderer::without_color();

        assert!(renderer.render_stateful_to_string(&command).is_empty());
        assert!(renderer.render_stateful_to_string(&file).is_empty());
        assert!(renderer.render_stateful_to_string(&search).is_empty());

        assert_eq!(renderer.current_active(), Some(&ActiveItem::Command));
    }

    #[test]
    fn tiny_width_spinner_line_does_not_mark_active_line_drawn() {
        let mut renderer = Renderer::without_color();
        renderer.active_line_drawn = true;

        renderer.render_spinner_line_for_width("thinking", Duration::from_secs(1), 12);

        assert!(!renderer.active_line_drawn);
    }

    #[test]
    fn parsed_events_refresh_phase_spinner_verb() {
        let event =
            CodexEvent::parse(r#"{"type":"turn.completed","usage":{"input_tokens":1}}"#).unwrap();
        let mut renderer = Renderer::without_color();
        renderer.phase_header(1, "Skeleton", "implement");
        let first = renderer.phase_status.as_ref().expect("phase").verb;

        renderer.event(&event);

        assert_ne!(renderer.phase_status.as_ref().expect("phase").verb, first);
    }

    #[test]
    fn hides_successful_internal_tool_plumbing() {
        let events = [
            r#"{"type":"item.started","item":{"id":"mcp_0","type":"mcp_tool_call","server":"github","tool":"get_pull_request","status":"in_progress"}}"#,
            r#"{"type":"item.completed","item":{"id":"mcp_0","type":"mcp_tool_call","server":"github","tool":"get_pull_request","status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"collab_0","type":"collab_tool_call","tool":"multi_agent","receiver_thread_ids":["a"],"status":"completed"}}"#,
        ];
        let rendered = events
            .into_iter()
            .map(|event| {
                Renderer::without_color()
                    .render_to_string(&CodexEvent::parse(event).expect("event"))
            })
            .collect::<String>();

        assert!(rendered.is_empty());
    }

    #[test]
    fn keeps_internal_tool_failures_visible() {
        let mcp = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"mcp_0","type":"mcp_tool_call","server":"github","tool":"get_pull_request","message":"permission denied","status":"failed"}}"#,
        )
        .unwrap();
        let collab = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"collab_0","type":"collab_tool_call","tool":"multi_agent","receiver_thread_ids":["a"],"status":"failed"}}"#,
        )
        .unwrap();

        let rendered = format!(
            "{}{}",
            Renderer::without_color().render_to_string(&mcp),
            Renderer::without_color().render_to_string(&collab)
        );

        assert!(rendered.contains("• Ran github/get_pull_request"));
        assert!(rendered.contains("  └ permission denied"));
        assert!(rendered.contains("• Ran multi_agent receiver_threads=1"));
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
