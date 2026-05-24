mod item;

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{
    json::{get_str, get_string, get_value},
    text::non_empty,
};

use item::ItemKind;

pub use item::TranscriptItem;

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedTurn {
    pub turn_id: String,
    pub status: String,
    pub transcript: TurnTranscript,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnTranscript {
    pub plan: Vec<PlanStep>,
    item_order: Vec<String>,
    items: BTreeMap<String, TranscriptItem>,
}

impl TurnTranscript {
    fn upsert_item(&mut self, item: TranscriptItem) {
        if !self.items.contains_key(item.id()) {
            self.item_order.push(item.id().to_string());
        }
        self.items.insert(item.id().to_string(), item);
    }

    fn append_delta(&mut self, item_id: String, kind: ItemKind, delta: &str) {
        if !self.items.contains_key(&item_id) {
            self.upsert_item(TranscriptItem::placeholder(item_id.clone(), kind));
        }

        if let Some(item) = self.items.get_mut(&item_id) {
            item.push_output(delta);
        }
    }

    pub fn response_text(&self) -> String {
        self.ordered_items()
            .filter(|item| item.kind() == ItemKind::AgentMessage)
            .filter_map(|item| item.text().or(non_empty(item.output())))
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn activity_items(&self) -> Vec<&TranscriptItem> {
        self.ordered_items()
            .filter(|item| item.kind() != ItemKind::AgentMessage && item.kind() != ItemKind::Plan)
            .filter(|item| item.is_renderable())
            .collect()
    }

    fn ordered_items(&self) -> impl Iterator<Item = &TranscriptItem> {
        self.item_order
            .iter()
            .filter_map(|item_id| self.items.get(item_id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    pub status: String,
    pub step: String,
}

pub(crate) struct TurnCollector<'a> {
    thread_id: &'a str,
    turn_id: &'a str,
    transcript: TurnTranscript,
}

impl<'a> TurnCollector<'a> {
    pub(crate) fn new(thread_id: &'a str, turn_id: &'a str) -> Self {
        Self {
            thread_id,
            turn_id,
            transcript: TurnTranscript::default(),
        }
    }

    pub(crate) fn handle(&mut self, message: &Value) -> Result<Option<CompletedTurn>> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Ok(None);
        };

        if method == "error" {
            let msg = get_string(message, &["params", "error", "message"])
                .unwrap_or_else(|_| "unknown Codex error".to_string());
            bail!("codex app-server error: {msg}");
        }

        if method == "turn/completed" {
            if self.matches_completed_turn(message) {
                self.record_turn_items(message);
                return self.completed_turn(message).map(Some);
            }
            return Ok(None);
        }

        if !self.matches_item_turn(message) {
            return Ok(None);
        }

        match method {
            "item/agentMessage/delta" => self.record_text_delta(message, ItemKind::AgentMessage)?,
            "item/plan/delta" => self.record_text_delta(message, ItemKind::Plan)?,
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                self.record_text_delta(message, ItemKind::Reasoning)?;
            }
            "item/commandExecution/outputDelta" => {
                self.record_text_delta(message, ItemKind::CommandExecution)?;
            }
            "item/fileChange/patchUpdated" => self.record_file_patch_update(message)?,
            "item/started" | "item/completed" => self.record_item(message, &["params", "item"]),
            "turn/plan/updated" => self.record_plan_update(message),
            _ => {}
        }

        Ok(None)
    }

    fn matches_item_turn(&self, message: &Value) -> bool {
        get_str(message, &["params", "threadId"]) == Some(self.thread_id)
            && get_str(message, &["params", "turnId"]) == Some(self.turn_id)
    }

    fn matches_completed_turn(&self, message: &Value) -> bool {
        get_str(message, &["params", "threadId"]) == Some(self.thread_id)
            && get_str(message, &["params", "turn", "id"]) == Some(self.turn_id)
    }

    fn record_text_delta(&mut self, message: &Value, kind: ItemKind) -> Result<()> {
        let item_id = get_string(message, &["params", "itemId"]).context("delta had no itemId")?;
        let delta = get_str(message, &["params", "delta"]).unwrap_or_default();
        self.transcript.append_delta(item_id, kind, delta);
        Ok(())
    }

    fn record_file_patch_update(&mut self, message: &Value) -> Result<()> {
        let item_id =
            get_string(message, &["params", "itemId"]).context("patch update had no itemId")?;
        let item = TranscriptItem::file_patch_update(item_id, &message["params"]);
        self.transcript.upsert_item(item);
        Ok(())
    }

    fn record_plan_update(&mut self, message: &Value) {
        let Some(plan) = message
            .get("params")
            .and_then(|params| params.get("plan"))
            .and_then(Value::as_array)
        else {
            return;
        };

        self.transcript.plan = plan
            .iter()
            .filter_map(|step| {
                Some(PlanStep {
                    status: get_str(step, &["status"])?.to_string(),
                    step: get_str(step, &["step"])?.to_string(),
                })
            })
            .collect();
    }

    fn record_item(&mut self, message: &Value, path: &[&str]) {
        let Some(item) = get_value(message, path) else {
            return;
        };
        self.record_thread_item(item);
    }

    fn record_turn_items(&mut self, message: &Value) {
        let Some(items) = message
            .get("params")
            .and_then(|params| params.get("turn"))
            .and_then(|turn| turn.get("items"))
            .and_then(Value::as_array)
        else {
            return;
        };

        for item in items {
            self.record_thread_item(item);
        }
    }

    fn record_thread_item(&mut self, item: &Value) {
        if let Some(item) = TranscriptItem::from_thread_item(item) {
            self.transcript.upsert_item(item);
        }
    }

    fn completed_turn(&self, message: &Value) -> Result<CompletedTurn> {
        let status = get_string(message, &["params", "turn", "status"])
            .context("turn/completed did not include turn.status")?;
        let error = get_string(message, &["params", "turn", "error", "message"]).ok();
        if status == "failed" {
            bail!(
                "turn failed: {}",
                error.unwrap_or_else(|| "unknown error".to_string())
            );
        }

        Ok(CompletedTurn {
            turn_id: self.turn_id.to_string(),
            status,
            transcript: self.transcript.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn turn_collector_prefers_completed_agent_message() {
        let mut collector = TurnCollector::new("thr_123", "turn_456");
        collector
            .handle(&json!({
                "method": "item/agentMessage/delta",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "itemId": "item_1",
                    "delta": "Hel"
                }
            }))
            .unwrap();
        collector
            .handle(&json!({
                "method": "item/completed",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "completedAtMs": 1,
                    "item": {
                        "type": "agentMessage",
                        "id": "item_1",
                        "text": "Hello.",
                        "phase": "final_answer"
                    }
                }
            }))
            .unwrap();

        let completed = collector
            .handle(&json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_456",
                        "items": [],
                        "status": "completed"
                    }
                }
            }))
            .unwrap()
            .unwrap();

        assert_eq!(
            completed,
            CompletedTurn {
                turn_id: "turn_456".to_string(),
                status: "completed".to_string(),
                transcript: {
                    let mut transcript = TurnTranscript::default();
                    let mut item =
                        TranscriptItem::placeholder("item_1".to_string(), ItemKind::AgentMessage);
                    item.title = "assistant".to_string();
                    item.set_text("Hello.");
                    transcript.upsert_item(item);
                    transcript
                },
            }
        );
    }

    #[test]
    fn turn_collector_ignores_other_turns() {
        let mut collector = TurnCollector::new("thr_123", "turn_456");
        let result = collector
            .handle(&json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_other",
                        "items": [],
                        "status": "completed"
                    }
                }
            }))
            .unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn turn_collector_records_plan_and_tool_items() {
        let mut collector = TurnCollector::new("thr_123", "turn_456");
        collector
            .handle(&json!({
                "method": "turn/plan/updated",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "plan": [
                        { "step": "Search docs", "status": "completed" },
                        { "step": "Run tests", "status": "inProgress" }
                    ]
                }
            }))
            .unwrap();
        collector
            .handle(&json!({
                "method": "item/completed",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "completedAtMs": 1,
                    "item": {
                        "type": "webSearch",
                        "id": "web_1",
                        "query": "rust cli best practices"
                    }
                }
            }))
            .unwrap();
        collector
            .handle(&json!({
                "method": "item/completed",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "completedAtMs": 2,
                    "item": {
                        "type": "commandExecution",
                        "id": "cmd_1",
                        "command": "cargo test",
                        "cwd": "/repo",
                        "commandActions": [],
                        "status": "completed",
                        "aggregatedOutput": "ok",
                        "exitCode": 0,
                        "durationMs": 123
                    }
                }
            }))
            .unwrap();

        assert_eq!(
            collector.transcript.plan,
            vec![
                PlanStep {
                    status: "completed".to_string(),
                    step: "Search docs".to_string(),
                },
                PlanStep {
                    status: "inProgress".to_string(),
                    step: "Run tests".to_string(),
                },
            ]
        );
        let activity = collector.transcript.activity_items();
        assert_eq!(activity.len(), 2);
        assert_eq!(activity[0].title, "web search: rust cli best practices");
        assert_eq!(activity[1].title, "$ cargo test");
        assert_eq!(activity[1].output(), "ok");
    }

    #[test]
    fn transcript_omits_user_messages_and_empty_items() {
        let mut transcript = TurnTranscript::default();
        let user_item = TranscriptItem::from_thread_item(&json!({
            "type": "userMessage",
            "id": "user_1",
            "content": [{ "type": "text", "text": "hi" }]
        }));
        assert_eq!(user_item, None);

        transcript.upsert_item(TranscriptItem::placeholder(
            "reasoning_1".to_string(),
            ItemKind::Reasoning,
        ));
        assert!(transcript.activity_items().is_empty());
    }
}
