use serde_json::{Value, json};

use super::{Charset, ColorMode, RenderOptions, Verbosity, renderer::Renderer};
use crate::app_server::{
    CompletedTurn, ItemKind, PlanStep, TranscriptItem, TranscriptItemData, TurnStreamEvent,
    TurnTranscript,
};

fn no_color_renderer() -> Renderer {
    Renderer::new(RenderOptions::default())
}

fn interactive_renderer() -> Renderer {
    Renderer::with_interactive(RenderOptions::default(), true)
}

fn item(value: Value) -> TranscriptItem {
    TranscriptItem::from_app_server_item(&value).expect("fixture should produce transcript item")
}

#[test]
fn renders_plan_updates_as_todos() {
    let mut renderer = no_color_renderer();
    let rendered = renderer.render_event(&TurnStreamEvent::PlanUpdated(vec![
        PlanStep {
            status: "completed".to_string(),
            step: "Inspect".to_string(),
        },
        PlanStep {
            status: "inProgress".to_string(),
            step: "Patch".to_string(),
        },
    ]));

    assert!(rendered.contains("• Updated Plan"));
    assert!(rendered.contains("  ✓ Inspect"));
    assert!(rendered.contains("  □ Patch"));
}

#[test]
fn ascii_mode_replaces_unicode_markers() {
    let mut renderer = Renderer::new(RenderOptions {
        charset: Charset::Ascii,
        ..RenderOptions::default()
    });
    let rendered = renderer.render_event(&TurnStreamEvent::PlanUpdated(vec![PlanStep {
        status: "completed".to_string(),
        step: "Inspect".to_string(),
    }]));

    assert!(rendered.contains("* Updated Plan"));
    assert!(rendered.contains("  [x] Inspect"));
    assert!(!rendered.contains("✓"));
}

#[test]
fn renders_color_when_enabled() {
    let mut renderer = Renderer::new(RenderOptions {
        color_mode: ColorMode::Always,
        ..RenderOptions::default()
    });
    let rendered = renderer.render_event(&TurnStreamEvent::PlanUpdated(vec![PlanStep {
        status: "completed".to_string(),
        step: "Inspect".to_string(),
    }]));

    assert!(rendered.contains("\x1b["));
}

#[test]
fn strips_basic_markdown_from_agent_messages() {
    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "agentMessage",
        "id": "item_1",
        "status": "completed",
        "text": "**Done**\n\n- `cargo test` passed"
    }))));

    assert!(rendered.contains("• Codex"));
    assert!(rendered.contains("  Done"));
    assert!(rendered.contains("  cargo test passed"));
    assert!(!rendered.contains("**Done**"));
}

#[test]
fn renders_command_output_and_truncates_lines() {
    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "commandExecution",
        "id": "cmd_1",
        "command": "make check",
        "status": "completed",
        "exitCode": 0,
        "aggregatedOutput": "fmt\nclippy\ntest\nbuild\nextra\n"
    }))));

    assert!(rendered.contains("• Ran make check"));
    assert!(rendered.contains("  └ fmt"));
    assert!(rendered.contains("    clippy"));
    assert!(rendered.contains("… +1 lines hidden"));
}

#[test]
fn renders_failed_command_without_output_as_evidence() {
    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "commandExecution",
        "id": "cmd_1",
        "command": "cargo test",
        "status": "failed",
        "exitCode": 101
    }))));

    assert!(rendered.contains("• Ran cargo test"));
    assert!(rendered.contains("  └ no output"));
    assert!(rendered.contains("    exit=101"));
}

#[test]
fn renders_file_changes() {
    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "fileChange",
        "id": "file_1",
        "status": "completed",
        "changes": [{ "kind": "update", "path": "src/lib.rs" }]
    }))));

    assert!(rendered.contains("• Edited src/lib.rs"));
    assert!(rendered.contains("  └ ~ src/lib.rs"));
}

#[test]
fn renders_web_search_and_hides_empty_progress() {
    let progress = item(json!({
        "type": "webSearch",
        "id": "web_1",
        "query": "docs",
        "status": "inProgress"
    }));
    assert!(
        no_color_renderer()
            .render_event(&TurnStreamEvent::ItemUpdated(progress))
            .is_empty()
    );

    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "webSearch",
        "id": "web_1",
        "query": "rust cli",
        "status": "completed"
    }))));
    assert!(rendered.contains("• Searched rust cli"));
}

#[test]
fn idle_tick_renders_phase_spinner_when_interactive() {
    let mut renderer = interactive_renderer();
    let header = renderer.phase_header(2, "Output Polish", "validation");

    let tick = renderer.render_event(&TurnStreamEvent::Idle);
    let finish = renderer.finish();

    assert!(header.contains("• Phase 02 validation: Output Polish"));
    assert!(tick.contains("\r\x1b[2K"));
    assert!(tick.contains("working on Phase 2 validation"));
    assert!(finish.contains("\x1b[?25h"));
}

#[test]
fn active_command_row_is_replaced_by_completed_output() {
    let mut renderer = interactive_renderer();
    let _ = renderer.phase_header(1, "Skeleton", "implementation");

    let progress = renderer.render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "commandExecution",
        "id": "cmd_1",
        "command": "cargo test",
        "status": "inProgress"
    }))));
    let completed = renderer.render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "commandExecution",
        "id": "cmd_1",
        "command": "cargo test",
        "status": "completed",
        "exitCode": 0,
        "aggregatedOutput": "ok"
    }))));

    assert!(progress.contains("running command"));
    assert!(completed.starts_with("\r\x1b[2K"));
    assert!(completed.contains("• Ran cargo test"));
    assert!(completed.contains("  └ ok"));
}

#[test]
fn hides_successful_tool_calls_but_keeps_failures() {
    let ok = item(json!({
        "type": "mcpToolCall",
        "id": "tool_1",
        "server": "github",
        "tool": "get_pull_request",
        "status": "completed"
    }));
    assert!(
        no_color_renderer()
            .render_event(&TurnStreamEvent::ItemUpdated(ok))
            .is_empty()
    );

    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "mcpToolCall",
        "id": "tool_2",
        "server": "github",
        "tool": "get_pull_request",
        "status": "failed",
        "error": "permission denied"
    }))));
    assert!(rendered.contains("• Ran mcp github/get_pull_request"));
    assert!(rendered.contains("  └ \"permission denied\""));
}

#[test]
fn renders_completed_turn_without_duplicating_streamed_items() {
    let final_message = item(json!({
        "type": "agentMessage",
        "id": "msg_1",
        "text": "Done.",
        "status": "completed"
    }));
    let turn = CompletedTurn {
        turn_id: "turn_1".to_string(),
        status: "completed".to_string(),
        transcript: TurnTranscript::from_items(Vec::new(), vec![final_message.clone()]),
    };
    let mut renderer = no_color_renderer();

    let streamed = renderer.render_event(&TurnStreamEvent::ItemUpdated(final_message));
    let completed = renderer.render_event(&TurnStreamEvent::Completed(turn));

    assert!(streamed.contains("Done."));
    assert!(completed.is_empty());
}

#[test]
fn renders_declined_server_requests_outside_quiet_mode() {
    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ServerRequestDeclined {
        method: "item/fileChange/requestApproval".to_string(),
    });
    assert!(rendered.contains("• Declined item/fileChange/requestApproval"));

    let mut quiet = Renderer::new(RenderOptions {
        verbosity: Verbosity::Quiet,
        ..RenderOptions::default()
    });
    assert!(
        quiet
            .render_event(&TurnStreamEvent::ServerRequestDeclined {
                method: "item/fileChange/requestApproval".to_string(),
            })
            .is_empty()
    );
}

#[test]
fn suppresses_duplicate_final_items() {
    let item = item(json!({
        "type": "fileChange",
        "id": "file_1",
        "status": "completed",
        "changes": [{ "kind": "update", "path": "src/lib.rs" }]
    }));
    let mut renderer = no_color_renderer();

    let first = renderer.render_event(&TurnStreamEvent::ItemUpdated(item.clone()));
    let second = renderer.render_event(&TurnStreamEvent::ItemUpdated(item));

    assert!(!first.is_empty());
    assert!(second.is_empty());
}

#[test]
fn renders_unknown_items_from_typed_payload() {
    let rendered = no_color_renderer().render_event(&TurnStreamEvent::ItemUpdated(item(json!({
        "type": "futureItem",
        "id": "future_1",
        "value": "kept"
    }))));

    assert!(rendered.contains("• Event unknown item: futureItem"));
    assert!(rendered.contains("\"value\":\"kept\""));
}

#[test]
fn transcript_items_expose_typed_payloads() {
    let item = item(json!({
        "type": "commandExecution",
        "id": "cmd_1",
        "command": "cargo test",
        "status": "completed",
        "exitCode": 0,
        "aggregatedOutput": "ok"
    }));

    assert_eq!(item.item_kind(), ItemKind::CommandExecution);
    assert!(matches!(
        item.data(),
        TranscriptItemData::CommandExecution(command)
            if command.command == "cargo test" && command.exit_code == Some(0)
    ));
}
