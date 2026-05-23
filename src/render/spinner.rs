use std::io::IsTerminal;
use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crossterm::cursor;
use crossterm::execute;
use crossterm::terminal;
use signal_hook::consts::SIGINT;
use signal_hook::iterator::Signals;

#[derive(Debug, Clone)]
pub(crate) struct Spinner {
    interactive: bool,
    line_drawn: bool,
    cursor_hidden: bool,
    text: &'static str,
    started_at: Instant,
    frame: usize,
    ticks: usize,
}

static SPINNER_ACTIVE: AtomicBool = AtomicBool::new(false);
static SIGINT_HANDLER: OnceLock<Result<(), String>> = OnceLock::new();
const FRAMES: &[&str] = &[".  ", ".. ", "...", ".. "];
const FALLBACK_TERMINAL_WIDTH: u16 = 80;
const MIN_SPINNER_WIDTH: usize = 20;
const EMBER_ITALIC: &str = "\x1b[3;38;5;208m";
const EMBER: &str = "\x1b[38;5;214m";
const RESET: &str = "\x1b[0m";

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
    "sneezing",
    "hiccuping",
    "sniffling",
    "burping",
    "microwaving",
    "napping",
    "catnapping",
    "facepalming",
];

impl Spinner {
    pub(crate) fn new(text: &'static str) -> std::io::Result<Self> {
        let interactive = std::io::stdout().is_terminal();

        Ok(Self {
            interactive,
            line_drawn: false,
            cursor_hidden: false,
            text,
            started_at: Instant::now(),
            frame: 0,
            ticks: 0,
        })
    }

    pub(crate) fn tick(&mut self) {
        if !self.interactive {
            return;
        }

        let frame = frame(self.frame);
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks.is_multiple_of(3) {
            self.frame = self.frame.wrapping_add(1);
        }

        let Some(rendered) = line_for_width(
            self.text,
            frame,
            self.started_at.elapsed(),
            terminal_width(),
        ) else {
            self.finish();
            return;
        };

        self.hide_cursor();
        print!("\r\x1b[2K{rendered}");
        let _ = std::io::stdout().flush();
        self.line_drawn = true;
    }

    pub(crate) fn finish(&mut self) {
        if self.line_drawn || self.cursor_hidden {
            deactivate_terminal();
        }
        self.line_drawn = false;
        self.cursor_hidden = false;
    }

    fn hide_cursor(&mut self) {
        if !self.cursor_hidden {
            let Ok(cursor_hidden) = activate_terminal() else {
                return;
            };
            if cursor_hidden {
                self.interactive = true;
                self.cursor_hidden = true;
            }
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.finish();
    }
}

pub(crate) fn activate_terminal() -> std::io::Result<bool> {
    if !std::io::stdout().is_terminal() {
        return Ok(false);
    }

    ensure_sigint_handler()?;
    SPINNER_ACTIVE.store(true, Ordering::SeqCst);
    execute!(std::io::stdout(), cursor::Hide)?;
    Ok(true)
}

pub(crate) fn deactivate_terminal() {
    SPINNER_ACTIVE.store(false, Ordering::SeqCst);
    restore_terminal();
}

fn ensure_sigint_handler() -> std::io::Result<()> {
    let result = SIGINT_HANDLER.get_or_init(|| {
        let mut signals = Signals::new([SIGINT]).map_err(|error| error.to_string())?;
        let _ = std::thread::spawn(move || {
            if signals.forever().next().is_some() {
                if SPINNER_ACTIVE.swap(false, Ordering::SeqCst) {
                    restore_terminal_without_stdout_lock();
                }
                std::process::exit(130);
            }
        });
        Ok(())
    });

    result
        .as_ref()
        .map(|_| ())
        .map_err(|message| std::io::Error::other(message.clone()))
}

fn restore_terminal() {
    print!("\r\x1b[2K");
    let _ = execute!(std::io::stdout(), cursor::Show);
    let _ = std::io::stdout().flush();
}

fn restore_terminal_without_stdout_lock() {
    const RESTORE: &[u8] = b"\r\x1b[2K\x1b[?25h";
    // Avoid stdio locking here: SIGINT may arrive while the main thread is flushing stdout.
    unsafe {
        let _ = libc::write(
            libc::STDOUT_FILENO,
            RESTORE.as_ptr().cast::<libc::c_void>(),
            RESTORE.len(),
        );
    }
}

pub(crate) fn random_text() -> &'static str {
    random_label_index(None)
}

pub(crate) fn random_text_except(current: &'static str) -> &'static str {
    random_label_index(Some(current))
}

fn random_label_index(current: Option<&'static str>) -> &'static str {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let mut index = nanos as usize % LABELS.len();
    if current.is_some_and(|current| LABELS[index] == current) {
        index = (index + 1) % LABELS.len();
    }
    LABELS[index]
}

pub(crate) fn frame(index: usize) -> &'static str {
    FRAMES[index % FRAMES.len()]
}

pub(crate) fn line_for_width(
    text: &str,
    frame: &str,
    elapsed: Duration,
    width: u16,
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

    Some(format!(
        "{EMBER_ITALIC}{label}{RESET} {EMBER}{suffix}{RESET}"
    ))
}

pub(crate) fn terminal_width() -> u16 {
    terminal::size()
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

pub(crate) fn elapsed_label(elapsed: Duration) -> String {
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
        let rendered =
            line_for_width("running\n\tcommand", "...", Duration::from_secs(7), 32).expect("line");

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
        )
        .expect("line");

        assert!(rendered.contains("thinking about..."));
        assert!(rendered.contains("... 7s"));
    }

    #[test]
    fn line_for_width_rejects_tiny_widths() {
        assert!(line_for_width("thinking", "...", Duration::from_secs(7), 12).is_none());
    }

    #[test]
    fn random_text_except_avoids_immediate_repeat() {
        let current = random_text();

        assert_ne!(random_text_except(current), current);
    }

    #[test]
    fn random_label_list_excludes_over_and_preposition_verbs() {
        for label in LABELS {
            assert!(!label.starts_with("over"), "{label}");
        }
        for removed in [
            "thinking",
            "daydreaming",
            "yakking",
            "bickering",
            "gossiping",
            "snacking",
        ] {
            assert!(!LABELS.contains(&removed), "{removed}");
        }
    }
}
