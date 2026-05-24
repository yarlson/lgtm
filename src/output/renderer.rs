use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use crate::{
    app_server::{CompletedTurn, PlanStep, TranscriptItem, TranscriptItemData, TurnStreamEvent},
    output::{
        ColorMode, RenderOptions, Verbosity,
        item::{ItemRenderer, item_is_final},
        spinner,
        style::{Color, Line, Span, Style, Symbol},
    },
};

#[derive(Debug, Clone)]
pub struct Renderer {
    options: RenderOptions,
    rendered_items: HashSet<String>,
    rendered_plan: Option<Vec<PlanStep>>,
    active_items: HashMap<String, ActiveItem>,
    active_order: Vec<String>,
    spinner: spinner::TerminalSpinner,
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
    label: String,
    verb: &'static str,
    started_at: Instant,
}

impl Renderer {
    pub fn new(options: RenderOptions) -> Self {
        let spinner_color = options.color_mode == ColorMode::Always;
        Self {
            options,
            rendered_items: HashSet::new(),
            rendered_plan: None,
            active_items: HashMap::new(),
            active_order: Vec::new(),
            spinner: spinner::TerminalSpinner::new(spinner_color),
            phase_status: None,
        }
    }

    pub fn phase_header(&mut self, phase: u32, title: &str, label: &str) -> String {
        self.phase_status = Some(PhaseStatus {
            phase,
            label: label.to_string(),
            verb: "working on",
            started_at: Instant::now(),
        });
        ItemRenderer::new(&self.options).render_lines(vec![
            ItemRenderer::new(&self.options).header(
                "Phase",
                Color::Blue,
                format!("{phase:02} {label}: {title}"),
            ),
            Line::blank(),
        ])
    }

    pub fn render_event(&mut self, event: &TurnStreamEvent) -> String {
        match event {
            TurnStreamEvent::Idle => self.tick(),
            TurnStreamEvent::PlanUpdated(plan) => self.render_plan_update(plan),
            TurnStreamEvent::ItemUpdated(item) => self.render_item_update(item),
            TurnStreamEvent::ServerRequestDeclined { method } => self.render_declined(method),
            TurnStreamEvent::Completed(turn) => self.render_completed_turn(turn),
        }
    }

    pub fn render_completed_turn(&mut self, turn: &CompletedTurn) -> String {
        let mut out = self.clear_active_line();
        out.push_str(&self.render_plan_update(&turn.transcript.plan));
        for item in turn.transcript.items() {
            out.push_str(&self.render_item_once(item));
        }
        out.push_str(&self.finish());
        out
    }

    fn render_plan_update(&mut self, plan: &[PlanStep]) -> String {
        if plan.is_empty() || self.options.verbosity == Verbosity::Quiet {
            return String::new();
        }
        if self.rendered_plan.as_deref() == Some(plan) {
            return String::new();
        }
        self.rendered_plan = Some(plan.to_vec());

        let renderer = ItemRenderer::new(&self.options);
        let mut lines = vec![renderer.header("Updated Plan", Color::Blue, "")];
        for step in plan {
            let done = matches!(step.status.as_str(), "completed");
            let marker = if done {
                renderer.symbol(Symbol::Check)
            } else {
                renderer.symbol(Symbol::EmptyBox)
            };
            lines.push(Line::new(vec![
                Span::raw("  "),
                Span::styled(
                    marker,
                    Style::fg(if done { Color::Green } else { Color::DarkGray }),
                ),
                Span::styled(
                    step.step.clone(),
                    if done {
                        Style::fg(Color::DarkGray).dim().strikethrough()
                    } else {
                        Style::fg(Color::Gray)
                    },
                ),
            ]));
        }
        lines.push(Line::blank());
        renderer.render_lines(lines)
    }

    fn render_item_update(&mut self, item: &TranscriptItem) -> String {
        if self.handle_active_item(item) {
            return self.render_active_line();
        }

        let mut out = self.clear_active_line();
        out.push_str(&self.render_item_once(item));
        out
    }

    fn render_item_once(&mut self, item: &TranscriptItem) -> String {
        if self.rendered_items.contains(item.id()) {
            return String::new();
        }

        let rendered = ItemRenderer::new(&self.options).render_item(item);
        if !rendered.is_empty() && item_is_final(item) {
            self.rendered_items.insert(item.id().to_string());
        }
        rendered
    }

    fn render_declined(&mut self, method: &str) -> String {
        if self.options.verbosity == Verbosity::Quiet {
            return String::new();
        }
        let mut rendered = self.clear_active_line();
        let renderer = ItemRenderer::new(&self.options);
        rendered.push_str(&renderer.render_lines(vec![
            renderer.header("Declined", Color::Yellow, method),
            Line::blank(),
        ]));
        rendered
    }

    pub fn tick(&mut self) -> String {
        if self.active_items.is_empty() {
            return self.render_phase_status_line();
        }
        self.render_active_line()
    }

    pub fn finish(&mut self) -> String {
        let out = self.clear_active_line();
        self.active_items.clear();
        self.active_order.clear();
        self.phase_status = None;
        out
    }

    fn handle_active_item(&mut self, item: &TranscriptItem) -> bool {
        let id = item.id().trim();
        if id.is_empty() {
            return false;
        }

        if item.is_final() {
            self.remove_active(id);
            return false;
        }

        let Some(active) = active_item(item) else {
            return false;
        };
        self.upsert_active(id.to_string(), active);
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

    fn render_active_line(&mut self) -> String {
        let Some(active) = self.current_active().cloned() else {
            return self.clear_active_line();
        };

        let label = match active {
            ActiveItem::Command => self.phase_activity_label("running command"),
            ActiveItem::FileChange { count: 1 } => self.phase_activity_label("editing file"),
            ActiveItem::FileChange { count } => {
                self.phase_activity_label(format!("editing {count} files"))
            }
            ActiveItem::WebSearch => self.phase_activity_label("searching the web"),
        };
        self.render_spinner_line(&label, self.phase_elapsed())
    }

    fn render_phase_status_line(&mut self) -> String {
        let Some(status) = self.phase_status.clone() else {
            return self.clear_active_line();
        };

        self.render_spinner_line(
            format!("{} Phase {} {}", status.verb, status.phase, status.label),
            status.started_at.elapsed(),
        )
    }

    fn render_spinner_line(&mut self, label: impl AsRef<str>, elapsed: Duration) -> String {
        self.spinner.tick(label, elapsed)
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
                status.label,
                activity.as_ref()
            ),
            None => activity.as_ref().to_string(),
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

    fn clear_active_line(&mut self) -> String {
        self.spinner.clear()
    }

    #[cfg(test)]
    pub(crate) fn with_interactive(options: RenderOptions, interactive: bool) -> Self {
        let mut renderer = Self::new(options);
        renderer.spinner = spinner::TerminalSpinner::for_test(interactive);
        renderer
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(RenderOptions::default())
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

fn active_item(item: &TranscriptItem) -> Option<ActiveItem> {
    if !item.is_in_progress() {
        return None;
    }

    match item.data() {
        TranscriptItemData::CommandExecution(_) => Some(ActiveItem::Command),
        TranscriptItemData::FileChange { changes } if !changes.is_empty() => {
            Some(ActiveItem::FileChange {
                count: changes.len(),
            })
        }
        TranscriptItemData::WebSearch(search) if !search.query.trim().is_empty() => {
            Some(ActiveItem::WebSearch)
        }
        _ => None,
    }
}
