use std::io::IsTerminal;

use anyhow::Result;
use clap::Parser;
use lgtm_app_server_client::{AppServerClient, AppServerConfig, TurnControl};
use lgtm_output::{ColorMode, RenderOptions, Renderer};

const VALIDATION_PROMPTS: &[&str] = &[
    "Use the shell command tool to run `pwd` and `ls -la` in the current directory. Then summarize what files are present.",
    "Use the shell command tool to run `find . -maxdepth 5 -type f -not -path './target/*' -not -path './.git/*' | sort`. Then summarize the project layout.",
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

    let config = demo_config();
    let mut server = AppServerClient::connect(config.clone())?;
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

    let mut renderer = Renderer::new(RenderOptions {
        color_mode: stdout_color_mode(),
        ..RenderOptions::default()
    });
    for (index, prompt) in VALIDATION_PROMPTS.iter().enumerate() {
        let n = index + 1;
        print_prompt(n, prompt);
        server.run_turn_streaming(&thread_id, prompt, |event| {
            print!("{}", renderer.render_event(&event));
            TurnControl::Continue
        })?;
    }

    server.stop()?;
    Ok(())
}

fn demo_config() -> AppServerConfig {
    AppServerConfig {
        model: "gpt-5.5".to_string(),
        reasoning_effort: "high".to_string(),
        sandbox: "danger-full-access".to_string(),
        approval_policy: "never".to_string(),
        developer_instructions: "Reply naturally to each user message.".to_string(),
        service_name: "lgtm-rs".to_string(),
        client_name: "lgtm-rs".to_string(),
        client_title: "lgtm-rs".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn print_prompt(n: usize, prompt: &str) {
    println!("Turn {n}");
    println!("  prompt:");
    for line in prompt.lines() {
        println!("    {line}");
    }
    println!();
}

fn stdout_color_mode() -> ColorMode {
    if std::env::var_os("NO_COLOR").is_some() || !std::io::stdout().is_terminal() {
        ColorMode::Never
    } else {
        ColorMode::Always
    }
}
