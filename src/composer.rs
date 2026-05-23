use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use crossterm::cursor;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};

use crate::Error;

const CTRL_C_BURST_WINDOW: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerSubmission {
    Answer(String),
    Finish,
    Quit,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ComposerInput {
    text: String,
}

impl ComposerInput {
    fn insert_char(&mut self, value: char) {
        self.text.push(value);
    }

    fn insert_newline(&mut self) {
        self.text.push('\n');
    }

    fn insert_paste(&mut self, value: &str) {
        self.text.push_str(&normalize_paste(value));
    }

    fn backspace(&mut self) {
        self.text.pop();
    }

    fn clear(&mut self) {
        self.text.clear();
    }

    fn submit(self) -> ComposerSubmission {
        match self.text.as_str() {
            "/finish" => ComposerSubmission::Finish,
            "/quit" => ComposerSubmission::Quit,
            _ => ComposerSubmission::Answer(self.text),
        }
    }
}

fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn read_inline_answer() -> Result<ComposerSubmission, Error> {
    let mut stdout = std::io::stdout();
    terminal::enable_raw_mode().map_err(|source| Error::io("<terminal>", source))?;
    let mut guard = ComposerTerminalGuard { active: true };
    execute!(stdout, EnableBracketedPaste).map_err(|source| Error::io("<stdout>", source))?;

    let result = read_inline_answer_inner(&mut stdout);
    let restore_result = guard.restore(&mut stdout);
    match (result, restore_result) {
        (Ok(submission), Ok(())) => Ok(submission),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn read_inline_answer_inner(stdout: &mut impl Write) -> Result<ComposerSubmission, Error> {
    let mut input = ComposerInput::default();
    let mut last_ctrl_c_at = None;
    let mut rendered_rows = 0;
    redraw(stdout, &input, rendered_rows).map_err(|source| Error::io("<stdout>", source))?;
    rendered_rows = row_count(&input);

    loop {
        match event::read().map_err(|source| Error::io("<terminal>", source))? {
            Event::Key(key) if key_is_active(key) => {
                match handle_key(&mut input, key, &mut last_ctrl_c_at, Instant::now()) {
                    ComposerAction::Continue => {}
                    ComposerAction::Submit => {
                        write!(stdout, "\r\n").map_err(|source| Error::io("<stdout>", source))?;
                        stdout
                            .flush()
                            .map_err(|source| Error::io("<stdout>", source))?;
                        return Ok(input.submit());
                    }
                    ComposerAction::Quit => {
                        write!(stdout, "\r\n").map_err(|source| Error::io("<stdout>", source))?;
                        stdout
                            .flush()
                            .map_err(|source| Error::io("<stdout>", source))?;
                        return Ok(ComposerSubmission::Quit);
                    }
                }
            }
            Event::Paste(value) => {
                last_ctrl_c_at = None;
                input.insert_paste(&value);
            }
            _ => continue,
        }

        redraw(stdout, &input, rendered_rows).map_err(|source| Error::io("<stdout>", source))?;
        rendered_rows = row_count(&input);
    }
}

fn key_is_active(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComposerAction {
    Continue,
    Submit,
    Quit,
}

fn handle_key(
    input: &mut ComposerInput,
    key: KeyEvent,
    last_ctrl_c_at: &mut Option<Instant>,
    now: Instant,
) -> ComposerAction {
    if is_ctrl_c(key) {
        if last_ctrl_c_at
            .is_some_and(|previous| now.duration_since(previous) <= CTRL_C_BURST_WINDOW)
        {
            return ComposerAction::Quit;
        }
        input.clear();
        *last_ctrl_c_at = Some(now);
        return ComposerAction::Continue;
    }

    *last_ctrl_c_at = None;
    match key.code {
        KeyCode::Enter
            if key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
        {
            input.insert_newline();
            ComposerAction::Continue
        }
        KeyCode::Enter => ComposerAction::Submit,
        KeyCode::Char('j' | 'J') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.insert_newline();
            ComposerAction::Continue
        }
        KeyCode::Backspace => {
            input.backspace();
            ComposerAction::Continue
        }
        KeyCode::Tab => {
            input.insert_char('\t');
            ComposerAction::Continue
        }
        KeyCode::Char(value)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            input.insert_char(value);
            ComposerAction::Continue
        }
        _ => ComposerAction::Continue,
    }
}

fn is_ctrl_c(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('c' | 'C')) && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn redraw(
    stdout: &mut impl Write,
    input: &ComposerInput,
    previous_rows: usize,
) -> std::io::Result<()> {
    clear_previous(stdout, previous_rows)?;

    for (index, line) in input.text.split('\n').enumerate() {
        if index > 0 {
            write!(stdout, "\r\n  {line}")?;
        } else {
            write!(stdout, "> {line}")?;
        }
    }

    stdout.flush()
}

fn clear_previous(stdout: &mut impl Write, previous_rows: usize) -> std::io::Result<()> {
    if previous_rows == 0 {
        return Ok(());
    }

    for index in 0..previous_rows {
        queue!(
            stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::CurrentLine)
        )?;
        if index + 1 < previous_rows {
            queue!(stdout, cursor::MoveUp(1))?;
        }
    }

    Ok(())
}

fn row_count(input: &ComposerInput) -> usize {
    input.text.split('\n').count()
}

struct ComposerTerminalGuard {
    active: bool,
}

impl ComposerTerminalGuard {
    fn restore(&mut self, stdout: &mut impl Write) -> Result<(), Error> {
        if !self.active {
            return Ok(());
        }
        self.active = false;

        let bracket_result =
            execute!(stdout, DisableBracketedPaste).map_err(|source| Error::io("<stdout>", source));
        let raw_result =
            terminal::disable_raw_mode().map_err(|source| Error::io("<terminal>", source));
        bracket_result.and(raw_result)
    }
}

impl Drop for ComposerTerminalGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = execute!(std::io::stdout(), DisableBracketedPaste);
            let _ = terminal::disable_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(input: &mut ComposerInput, key: KeyEvent) -> ComposerAction {
        let mut last_ctrl_c_at = None;
        handle_key(input, key, &mut last_ctrl_c_at, Instant::now())
    }

    #[test]
    fn text_input_plus_enter_submits_answer() {
        let mut input = ComposerInput::default();
        input.insert_char('h');
        input.insert_char('i');

        let action = key(
            &mut input,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );

        assert_eq!(action, ComposerAction::Submit);
        assert_eq!(input.submit(), ComposerSubmission::Answer("hi".to_string()));
    }

    #[test]
    fn ctrl_j_inserts_newline() {
        let mut input = ComposerInput::default();
        input.insert_char('a');
        let action = key(
            &mut input,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        );
        input.insert_char('b');

        assert_eq!(action, ComposerAction::Continue);
        assert_eq!(
            input.submit(),
            ComposerSubmission::Answer("a\nb".to_string())
        );
    }

    #[test]
    fn alt_and_shift_enter_insert_newline_when_delivered_distinctly() {
        let mut alt = ComposerInput::default();
        alt.insert_char('a');
        let alt_action = key(&mut alt, KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
        alt.insert_char('b');

        let mut shift = ComposerInput::default();
        shift.insert_char('x');
        let shift_action = key(
            &mut shift,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        );
        shift.insert_char('y');

        assert_eq!(alt_action, ComposerAction::Continue);
        assert_eq!(shift_action, ComposerAction::Continue);
        assert_eq!(alt.submit(), ComposerSubmission::Answer("a\nb".to_string()));
        assert_eq!(
            shift.submit(),
            ComposerSubmission::Answer("x\ny".to_string())
        );
    }

    #[test]
    fn pasted_crlf_and_cr_normalize_to_lf() {
        let mut input = ComposerInput::default();
        input.insert_paste("one\r\ntwo\rthree\nfour");

        assert_eq!(
            input.submit(),
            ComposerSubmission::Answer("one\ntwo\nthree\nfour".to_string())
        );
    }

    #[test]
    fn exact_finish_and_quit_are_recognized_after_submission() {
        let mut finish = ComposerInput::default();
        for value in "/finish".chars() {
            assert_eq!(
                key(
                    &mut finish,
                    KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE)
                ),
                ComposerAction::Continue
            );
        }
        let mut quit = ComposerInput::default();
        for value in "/quit".chars() {
            assert_eq!(
                key(
                    &mut quit,
                    KeyEvent::new(KeyCode::Char(value), KeyModifiers::NONE)
                ),
                ComposerAction::Continue
            );
        }

        assert_eq!(
            key(
                &mut finish,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            ComposerAction::Submit
        );
        assert_eq!(
            key(&mut quit, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ComposerAction::Submit
        );
        assert_eq!(finish.submit(), ComposerSubmission::Finish);
        assert_eq!(quit.submit(), ComposerSubmission::Quit);
    }

    #[test]
    fn slash_prefixed_non_commands_are_answers() {
        let mut spaced = ComposerInput::default();
        spaced.insert_paste(" /quit ");
        let mut extended = ComposerInput::default();
        extended.insert_paste("/finish now");

        assert_eq!(
            spaced.submit(),
            ComposerSubmission::Answer(" /quit ".to_string())
        );
        assert_eq!(
            extended.submit(),
            ComposerSubmission::Answer("/finish now".to_string())
        );
    }

    #[test]
    fn backspace_removes_last_character() {
        let mut input = ComposerInput::default();
        input.insert_paste("ab");
        input.backspace();

        assert_eq!(input.submit(), ComposerSubmission::Answer("a".to_string()));
    }

    #[test]
    fn single_ctrl_c_clears_input_without_submitting() {
        let mut input = ComposerInput::default();
        input.insert_paste("one\ntwo");
        let mut last_ctrl_c_at = None;

        let action = handle_key(
            &mut input,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut last_ctrl_c_at,
            Instant::now(),
        );

        assert_eq!(action, ComposerAction::Continue);
        assert_eq!(input.submit(), ComposerSubmission::Answer(String::new()));
        assert!(last_ctrl_c_at.is_some());
    }

    #[test]
    fn quick_second_ctrl_c_quits() {
        let mut input = ComposerInput::default();
        input.insert_paste("one\ntwo");
        let start = Instant::now();
        let mut last_ctrl_c_at = None;

        assert_eq!(
            handle_key(
                &mut input,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut last_ctrl_c_at,
                start,
            ),
            ComposerAction::Continue
        );
        assert_eq!(
            handle_key(
                &mut input,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut last_ctrl_c_at,
                start + Duration::from_millis(100),
            ),
            ComposerAction::Quit
        );
    }

    #[test]
    fn slow_second_ctrl_c_clears_again_without_quitting() {
        let mut input = ComposerInput::default();
        input.insert_paste("one");
        let start = Instant::now();
        let mut last_ctrl_c_at = None;

        assert_eq!(
            handle_key(
                &mut input,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut last_ctrl_c_at,
                start,
            ),
            ComposerAction::Continue
        );
        assert_eq!(
            handle_key(
                &mut input,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut last_ctrl_c_at,
                start + CTRL_C_BURST_WINDOW + Duration::from_millis(1),
            ),
            ComposerAction::Continue
        );
    }
}
