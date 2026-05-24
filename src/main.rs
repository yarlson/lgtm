use anyhow::Result;
use clap::Parser;
use lgtm_app_server_client::{AppServerClient, AppServerConfig, CompletedTurn};

const VALIDATION_PROMPTS: &[&str] = &[
    "Use the shell command tool to run `pwd` and `ls -la` in the current directory. Then summarize what files are present.",
    "Use the shell command tool to run `find . -maxdepth 4 -type f -not -path './target/*' -not -path './.git/*' | sort`. Then summarize the project layout.",
    "Use the shell command tool to inspect `Cargo.toml` and report the package name, edition, and dependencies.",
    "Use web search to find current Rust CLI best practices in 2026. Return three concise bullets and include source URLs.",
    "Use web search to find current OpenAI Codex app-server or Codex open-source documentation. Return the most relevant source URL and one sentence.",
    "Use your todo/plan tool to create a three-step plan for validating this CLI's transcript rendering. Then complete the plan in your answer without running commands.",
    "Use your todo/plan tool to plan a tiny refactor of this CLI into transport, transcript, and main modules. Do not edit files; just produce the plan and final recommendation.",
    "Use the shell command tool to run `cargo test --workspace`. Summarize pass/fail status.",
    "Use the shell command tool to run `cargo clippy --workspace --all-targets -- -D warnings`. Summarize pass/fail status.",
    "Use both a short todo/plan and a shell command: plan two steps, run `cargo fmt -- --check`, then report the result.",
];

#[derive(Debug, Parser)]
#[command(version, about = "Run a simple Codex app-server validation loop")]
struct Cli;

fn main() -> Result<()> {
    Cli::parse();

    let config = AppServerConfig::default();
    let mut server = AppServerClient::start_with_config(config.clone())?;
    server.initialize()?;
    let thread_id = server.start_thread()?;

    println!("Codex app-server validation loop");
    println!("Thread: {thread_id}");
    println!("Model: {}", config.model);
    println!("Effort: {}", config.reasoning_effort);
    println!(
        "Mode: yolo ({}, approvals {})",
        config.sandbox, config.approval_policy
    );
    println!();

    for (index, prompt) in VALIDATION_PROMPTS.iter().enumerate() {
        let n = index + 1;
        let turn = server.run_turn(&thread_id, prompt)?;
        print_turn(n, prompt, &turn);
    }

    server.stop()?;
    Ok(())
}

fn print_turn(n: usize, prompt: &str, turn: &CompletedTurn) {
    println!("Turn {n}");
    println!("  prompt:");
    for line in prompt.lines() {
        println!("    {line}");
    }
    println!("  id: {}", turn.turn_id);
    println!("  status: {}", turn.status);

    if !turn.transcript.plan.is_empty() {
        println!("  plan:");
        for step in &turn.transcript.plan {
            println!("    [{}] {}", step.status, step.step);
        }
    }

    let activity = turn.transcript.activity_items();
    if !activity.is_empty() {
        println!("  activity:");
        for item in activity {
            println!("    - {}", item.title);
            for detail in &item.details {
                println!("      {detail}");
            }
            if let Some(output) = item.output_preview() {
                println!("      output:");
                for line in output.lines() {
                    println!("        {line}");
                }
            }
        }
    }

    let response = turn.transcript.response_text();
    if response.trim().is_empty() {
        println!("  response: <empty>");
    } else {
        println!("  response:");
        for line in response.trim().lines() {
            println!("    {line}");
        }
    }

    println!();
}
