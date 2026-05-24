use std::collections::HashSet;

use lgtm_app_server_client::{CompletedTurn, PlanStep, TranscriptItem, TurnStreamEvent};

use crate::{
    RenderOptions, Verbosity,
    item::{ItemRenderer, item_is_final},
    style::{Color, Line, Span, Style, Symbol},
};

#[derive(Debug, Clone)]
pub struct Renderer {
    options: RenderOptions,
    rendered_items: HashSet<String>,
    rendered_plan: Option<Vec<PlanStep>>,
}

impl Renderer {
    pub fn new(options: RenderOptions) -> Self {
        Self {
            options,
            rendered_items: HashSet::new(),
            rendered_plan: None,
        }
    }

    pub fn render_event(&mut self, event: &TurnStreamEvent) -> String {
        match event {
            TurnStreamEvent::PlanUpdated(plan) => self.render_plan_update(plan),
            TurnStreamEvent::ItemUpdated(item) => self.render_item_once(item),
            TurnStreamEvent::ServerRequestDeclined { method } => self.render_declined(method),
            TurnStreamEvent::Completed(turn) => self.render_completed_turn(turn),
        }
    }

    pub fn render_completed_turn(&mut self, turn: &CompletedTurn) -> String {
        let mut out = String::new();
        out.push_str(&self.render_plan_update(&turn.transcript.plan));
        for item in turn.transcript.items() {
            out.push_str(&self.render_item_once(item));
        }
        out
    }

    pub fn options(&self) -> &RenderOptions {
        &self.options
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

    fn render_declined(&self, method: &str) -> String {
        if self.options.verbosity == Verbosity::Quiet {
            return String::new();
        }
        let renderer = ItemRenderer::new(&self.options);
        renderer.render_lines(vec![
            renderer.header("Declined", Color::Yellow, method),
            Line::blank(),
        ])
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(RenderOptions::default())
    }
}
