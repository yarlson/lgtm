use std::io::IsTerminal;
use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crossterm::cursor;
use crossterm::execute;
use signal_hook::consts::SIGINT;
use signal_hook::iterator::Signals;

#[derive(Debug, Clone)]
pub(crate) struct Spinner {
    interactive: bool,
    line_drawn: bool,
    cursor_hidden: bool,
    text: &'static str,
    frame: usize,
    ticks: usize,
}

static SPINNER_ACTIVE: AtomicBool = AtomicBool::new(false);
static SIGINT_HANDLER: OnceLock<Result<(), String>> = OnceLock::new();

const LABELS: &[&str] = &[
    "procrastinating",
    "daydreaming",
    "doomscrolling",
    "moonwalking",
    "noodling",
    "doodling",
    "bamboozling",
    "yakking",
    "bickering",
    "gossiping",
    "twiddling",
    "fidgeting",
    "foot-tapping",
    "blinking",
    "sighing",
    "sneezing",
    "hiccuping",
    "sniffling",
    "burping",
    "snacking",
    "microwaving",
    "napping",
    "catnapping",
    "facepalming",
    "overthinking",
    "overexplaining",
    "oversharing",
    "overreacting",
    "overanalyzing",
    "overcaffeinating",
    "overwatering",
    "overcooking",
    "oversleeping",
    "overeating",
    "overbuying",
    "overpaying",
    "overpricing",
    "overediting",
    "overchatting",
    "overtweeting",
    "overcommenting",
    "oversinging",
    "overdancing",
    "overzooming",
    "overstaring",
    "overblinking",
    "overgiggling",
    "overyawning",
    "overhugging",
    "overgrinning",
    "overflipping",
    "overdrinking",
    "oversmiling",
    "overfrowning",
    "overtapping",
    "overtwitching",
    "overtumbling",
    "oversquirming",
    "overwiggling",
    "overbouncing",
    "overshuffling",
    "overclapping",
    "overpacing",
    "overbaking",
    "overboasting",
    "overmumbling",
    "overcackling",
    "overarguing",
    "overdoodling",
    "overpainting",
    "overbuilding",
    "overpacking",
    "overmixing",
    "overtyping",
    "overtexting",
    "overcalling",
    "overtalking",
    "overlaughing",
    "overspeaking",
    "overstumbling",
    "overhopping",
    "overpouncing",
    "overchewing",
    "overpeeking",
    "overworrying",
    "overpracticing",
    "overcounting",
    "overhunting",
    "overpursuing",
    "overreading",
    "overwriting",
    "overgasping",
    "overclimbing",
    "overwalking",
    "overflying",
    "overhauling",
    "overboiling",
    "overdressing",
    "overcleaning",
    "overcomplaining",
    "overcelebrating",
    "overcursing",
];

impl Spinner {
    pub(crate) fn new(text: &'static str) -> std::io::Result<Self> {
        let interactive = std::io::stdout().is_terminal();
        if interactive {
            ensure_sigint_handler()?;
        }

        let mut spinner = Self {
            interactive,
            line_drawn: false,
            cursor_hidden: false,
            text,
            frame: 0,
            ticks: 0,
        };
        spinner.hide_cursor();
        Ok(spinner)
    }

    pub(crate) fn tick(&mut self) {
        if !self.interactive {
            return;
        }

        const FRAMES: &[&str] = &[".", "..", "...", ".."];
        let frame = FRAMES[self.frame % FRAMES.len()];
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks.is_multiple_of(3) {
            self.frame = self.frame.wrapping_add(1);
        }

        print!(
            "\r\x1b[2K\x1b[3;37m{}\x1b[0m \x1b[37m{frame}\x1b[0m",
            self.text
        );
        let _ = std::io::stdout().flush();
        self.line_drawn = true;
    }

    pub(crate) fn finish(&mut self) {
        if self.line_drawn || self.cursor_hidden {
            restore_terminal();
        }
        self.line_drawn = false;
        self.show_cursor();
    }

    fn hide_cursor(&mut self) {
        if self.interactive && !self.cursor_hidden {
            SPINNER_ACTIVE.store(true, Ordering::SeqCst);
            let _ = execute!(std::io::stdout(), cursor::Hide);
            self.cursor_hidden = true;
        }
    }

    fn show_cursor(&mut self) {
        if self.cursor_hidden {
            SPINNER_ACTIVE.store(false, Ordering::SeqCst);
            self.cursor_hidden = false;
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.finish();
    }
}

fn ensure_sigint_handler() -> std::io::Result<()> {
    let result = SIGINT_HANDLER.get_or_init(|| {
        let mut signals = Signals::new([SIGINT]).map_err(|error| error.to_string())?;
        let _ = std::thread::spawn(move || {
            if signals.forever().next().is_some() {
                if SPINNER_ACTIVE.swap(false, Ordering::SeqCst) {
                    restore_terminal();
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

pub(crate) fn random_text() -> &'static str {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    LABELS[nanos as usize % LABELS.len()]
}
