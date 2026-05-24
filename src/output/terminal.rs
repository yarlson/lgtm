use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

use signal_hook::{consts::SIGINT, iterator::Signals};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);
static SIGINT_HANDLER: OnceLock<Result<(), String>> = OnceLock::new();

pub(crate) fn hide_cursor() -> &'static str {
    let _ = ensure_sigint_restore();
    CURSOR_HIDDEN.store(true, Ordering::SeqCst);
    "\x1b[?25l"
}

pub(crate) fn show_cursor() -> &'static str {
    CURSOR_HIDDEN.store(false, Ordering::SeqCst);
    "\x1b[?25h"
}

fn ensure_sigint_restore() -> Result<(), String> {
    SIGINT_HANDLER
        .get_or_init(|| {
            let mut signals = Signals::new([SIGINT]).map_err(|error| error.to_string())?;
            let _ = std::thread::spawn(move || {
                if signals.forever().next().is_some() {
                    if CURSOR_HIDDEN.swap(false, Ordering::SeqCst) {
                        restore_cursor_without_stdout_lock();
                    }
                    std::process::exit(130);
                }
            });
            Ok(())
        })
        .clone()
}

fn restore_cursor_without_stdout_lock() {
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
