use std::io::Write;

use anyhow::{Context, Result};
use crossterm::{
    Command,
    event::{
        DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{self},
};

pub(super) fn enter(stdout: &mut impl Write) -> Result<ComposerTerminalGuard> {
    terminal::enable_raw_mode().context("failed to enable terminal raw mode")?;
    execute!(
        stdout,
        EnableBracketedPaste,
        DisableModifyOtherKeys,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        ),
        EnableModifyOtherKeys,
    )
    .context("failed to configure terminal input mode")?;
    Ok(ComposerTerminalGuard { active: true })
}

pub(super) struct ComposerTerminalGuard {
    active: bool,
}

impl ComposerTerminalGuard {
    pub(super) fn restore(&mut self, stdout: &mut impl Write) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;

        let bracket_result = execute!(
            stdout,
            PopKeyboardEnhancementFlags,
            DisableModifyOtherKeys,
            DisableBracketedPaste,
        )
        .context("failed to restore terminal input mode");
        let raw_result =
            terminal::disable_raw_mode().context("failed to disable terminal raw mode");
        bracket_result.and(raw_result)
    }
}

impl Drop for ComposerTerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = execute!(
                std::io::stdout(),
                PopKeyboardEnhancementFlags,
                ResetKeyboardEnhancementFlags,
                DisableModifyOtherKeys,
                DisableBracketedPaste,
            );
            let _ = terminal::disable_raw_mode();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableModifyOtherKeys;

impl Command for EnableModifyOtherKeys {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[>4;2m")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableModifyOtherKeys;

impl Command for DisableModifyOtherKeys {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[>4;0m")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResetKeyboardEnhancementFlags;

impl Command for ResetKeyboardEnhancementFlags {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        f.write_str("\x1b[<u")
    }
}
