use std::{
    io::IsTerminal,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::output::terminal;

const FRAMES: &[&str] = &[".  ", ".. ", "...", ".. "];
const FALLBACK_TERMINAL_WIDTH: u16 = 80;
const MIN_SPINNER_WIDTH: usize = 20;
const LABELS: &[&str] = &[
    "procrastinating",
    "doomscrolling",
    "moonwalking",
    "noodling",
    "doodling",
    "bamboozling",
    "twiddling",
    "fidgeting",
    "foot-tapping",
    "blinking",
    "sighing",
    "napping",
    "facepalming",
];
#[derive(Debug, Clone)]
pub(crate) struct TerminalSpinner {
    interactive: bool,
    line_drawn: bool,
    cursor_hidden: bool,
    color: bool,
    frame: usize,
    ticks: usize,
}

impl TerminalSpinner {
    pub(crate) fn new(color: bool) -> Self {
        Self {
            interactive: std::io::stdout().is_terminal(),
            line_drawn: false,
            cursor_hidden: false,
            color,
            frame: 0,
            ticks: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(interactive: bool) -> Self {
        Self {
            interactive,
            line_drawn: false,
            cursor_hidden: false,
            color: false,
            frame: 0,
            ticks: 0,
        }
    }

    pub(crate) fn tick(&mut self, label: impl AsRef<str>, elapsed: Duration) -> String {
        if !self.interactive {
            return String::new();
        }

        let frame = self.next_frame();
        let Some(line) =
            line_for_width(label.as_ref(), frame, elapsed, terminal_width(), self.color)
        else {
            return self.clear();
        };

        self.line_drawn = true;
        if !self.cursor_hidden {
            self.cursor_hidden = true;
            format!("{}\r\x1b[2K{line}", terminal::hide_cursor())
        } else {
            format!("\r\x1b[2K{line}")
        }
    }

    pub(crate) fn clear(&mut self) -> String {
        if self.line_drawn || self.cursor_hidden {
            self.line_drawn = false;
            if self.cursor_hidden {
                self.cursor_hidden = false;
                format!("\r\x1b[2K{}", terminal::show_cursor())
            } else {
                "\r\x1b[2K".to_string()
            }
        } else {
            String::new()
        }
    }

    fn next_frame(&mut self) -> &'static str {
        let frame = frame(self.frame);
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks.is_multiple_of(3) {
            self.frame = self.frame.wrapping_add(1);
        }
        frame
    }
}

impl Drop for TerminalSpinner {
    fn drop(&mut self) {
        if self.cursor_hidden {
            print!("\r\x1b[2K{}", terminal::show_cursor());
        }
    }
}

pub(crate) fn frame(index: usize) -> &'static str {
    FRAMES[index % FRAMES.len()]
}

pub(crate) fn random_text_except(current: &'static str) -> &'static str {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let mut index = nanos as usize % LABELS.len();
    if LABELS[index] == current {
        index = (index + 1) % LABELS.len();
    }
    LABELS[index]
}

pub(crate) fn line_for_width(
    text: &str,
    frame: &str,
    elapsed: Duration,
    width: u16,
    color: bool,
) -> Option<String> {
    let width = usize::from(width);
    if width < MIN_SPINNER_WIDTH {
        return None;
    }

    let elapsed = elapsed_label(elapsed);
    let suffix = format!("{frame} {elapsed}");
    let suffix_width = 1 + suffix.len();
    if width <= suffix_width {
        return None;
    }

    let label_width = width - suffix_width;
    let label = truncate_label(&one_line_label(text), label_width)?;
    if color {
        Some(format!(
            "\x1b[3;38;5;208m{label}\x1b[0m \x1b[38;5;214m{suffix}\x1b[0m"
        ))
    } else {
        Some(format!("{label} {suffix}"))
    }
}

pub(crate) fn terminal_width() -> u16 {
    crossterm::terminal::size()
        .ok()
        .and_then(|(columns, _rows)| (columns > 0).then_some(columns))
        .unwrap_or(FALLBACK_TERMINAL_WIDTH)
}

fn one_line_label(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_label(label: &str, max_width: usize) -> Option<String> {
    if max_width == 0 {
        return None;
    }
    if label.len() <= max_width {
        return Some(label.to_string());
    }
    if max_width <= 3 {
        return Some(".".repeat(max_width));
    }
    let mut truncated: String = label.chars().take(max_width - 3).collect();
    truncated.push_str("...");
    Some(truncated)
}

fn elapsed_label(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        let minutes = seconds / 60;
        let seconds = seconds % 60;
        format!("{minutes}m{seconds:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_for_width_fits_and_collapses_label() {
        let rendered = line_for_width(
            "running\n\tcommand",
            "...",
            Duration::from_secs(7),
            32,
            false,
        )
        .expect("line");

        assert!(rendered.contains("running command"));
        assert!(rendered.contains("... 7s"));
    }

    #[test]
    fn line_for_width_truncates_long_labels() {
        let rendered = line_for_width(
            "thinking about a very long thing",
            "...",
            Duration::from_secs(7),
            24,
            false,
        )
        .expect("line");

        assert!(rendered.contains("thinking about..."));
        assert!(rendered.contains("... 7s"));
    }

    #[test]
    fn line_for_width_rejects_tiny_widths() {
        assert!(line_for_width("thinking", "...", Duration::from_secs(7), 12, false).is_none());
    }

    #[test]
    fn random_text_except_avoids_immediate_repeat() {
        let current = LABELS[0];

        assert_ne!(random_text_except(current), current);
    }
}
