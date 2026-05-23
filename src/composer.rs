use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use crossterm::cursor;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, KeyboardEnhancementFlags, ModifierKeyCode, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{Command, execute, queue};

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
    cursor: usize,
}

impl ComposerInput {
    fn insert_char(&mut self, value: char) {
        self.insert_str(&value.to_string());
    }

    fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    fn insert_paste(&mut self, value: &str) {
        self.insert_str(&normalize_paste(value));
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let previous = self.previous_boundary();
        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
    }

    fn delete(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        let next = self.next_boundary();
        self.text.replace_range(self.cursor..next, "");
    }

    fn move_left(&mut self) {
        self.cursor = self.previous_boundary();
    }

    fn move_right(&mut self) {
        self.cursor = self.next_boundary();
    }

    fn move_up(&mut self) {
        let line_start = self.line_start();
        if line_start == 0 {
            return;
        }

        let target_col = self.cursor_char_column(line_start);
        let previous_line_end = line_start - 1;
        let previous_line_start = self.text[..previous_line_end]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.cursor = byte_at_char_column(
            previous_line_start,
            &self.text[previous_line_start..previous_line_end],
            target_col,
        );
    }

    fn move_down(&mut self) {
        let line_end = self.line_end();
        if line_end >= self.text.len() {
            return;
        }

        let target_col = self.cursor_char_column(self.line_start());
        let next_line_start = line_end + 1;
        let next_line_end = self.text[next_line_start..]
            .find('\n')
            .map_or(self.text.len(), |offset| next_line_start + offset);
        self.cursor = byte_at_char_column(
            next_line_start,
            &self.text[next_line_start..next_line_end],
            target_col,
        );
    }

    fn move_line_start(&mut self) {
        self.cursor = self.line_start();
    }

    fn move_line_end(&mut self) {
        self.cursor = self.line_end();
    }

    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    fn submit(self) -> ComposerSubmission {
        match self.text.as_str() {
            "/finish" => ComposerSubmission::Finish,
            "/quit" => ComposerSubmission::Quit,
            _ => ComposerSubmission::Answer(self.text),
        }
    }

    fn insert_str(&mut self, value: &str) {
        self.text.insert_str(self.cursor, value);
        self.cursor += value.len();
    }

    fn previous_boundary(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index)
    }

    fn next_boundary(&self) -> usize {
        if self.cursor >= self.text.len() {
            return self.text.len();
        }

        self.cursor
            + self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8)
    }

    fn line_start(&self) -> usize {
        self.text[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |offset| self.cursor + offset)
    }

    fn cursor_char_column(&self, line_start: usize) -> usize {
        self.text[line_start..self.cursor].chars().count()
    }
}

fn byte_at_char_column(line_start: usize, line: &str, target_col: usize) -> usize {
    line.char_indices()
        .nth(target_col)
        .map_or(line_start + line.len(), |(offset, _)| line_start + offset)
}

fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn read_inline_answer() -> Result<ComposerSubmission, Error> {
    let mut stdout = std::io::stdout();
    terminal::enable_raw_mode().map_err(|source| Error::io("<terminal>", source))?;
    let mut guard = ComposerTerminalGuard { active: true };
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
    .map_err(|source| Error::io("<stdout>", source))?;

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
    let mut modifier_state = ModifierState::default();
    let mut rendered_rows = 0;
    redraw(stdout, &input, rendered_rows).map_err(|source| Error::io("<stdout>", source))?;
    rendered_rows = row_count(&input);

    loop {
        match event::read().map_err(|source| Error::io("<terminal>", source))? {
            Event::Key(key) if modifier_state.update(key) => continue,
            Event::Key(key) if key_is_active(key) => {
                let key = modifier_state.apply(key);
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
struct ModifierState {
    active: KeyModifiers,
}

impl Default for ModifierState {
    fn default() -> Self {
        Self {
            active: KeyModifiers::NONE,
        }
    }
}

impl ModifierState {
    fn update(&mut self, key: KeyEvent) -> bool {
        let KeyCode::Modifier(modifier) = key.code else {
            return false;
        };
        let Some(modifiers) = key_modifiers_for_modifier(modifier) else {
            return true;
        };

        match key.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => self.active.insert(modifiers),
            KeyEventKind::Release => self.active.remove(modifiers),
        }

        true
    }

    fn apply(self, mut key: KeyEvent) -> KeyEvent {
        key.modifiers.insert(self.active);
        key
    }
}

fn key_modifiers_for_modifier(modifier: ModifierKeyCode) -> Option<KeyModifiers> {
    match modifier {
        ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift => Some(KeyModifiers::SHIFT),
        ModifierKeyCode::LeftControl | ModifierKeyCode::RightControl => Some(KeyModifiers::CONTROL),
        ModifierKeyCode::LeftAlt | ModifierKeyCode::RightAlt => Some(KeyModifiers::ALT),
        ModifierKeyCode::LeftSuper | ModifierKeyCode::RightSuper => Some(KeyModifiers::SUPER),
        ModifierKeyCode::LeftHyper | ModifierKeyCode::RightHyper => Some(KeyModifiers::HYPER),
        ModifierKeyCode::LeftMeta | ModifierKeyCode::RightMeta => Some(KeyModifiers::META),
        ModifierKeyCode::IsoLevel3Shift | ModifierKeyCode::IsoLevel5Shift => None,
    }
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
        KeyCode::Left if plain_movement_modifiers(key.modifiers) => {
            input.move_left();
            ComposerAction::Continue
        }
        KeyCode::Char('b' | 'B') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.move_left();
            ComposerAction::Continue
        }
        KeyCode::Right if plain_movement_modifiers(key.modifiers) => {
            input.move_right();
            ComposerAction::Continue
        }
        KeyCode::Char('f' | 'F') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.move_right();
            ComposerAction::Continue
        }
        KeyCode::Up if plain_movement_modifiers(key.modifiers) => {
            input.move_up();
            ComposerAction::Continue
        }
        KeyCode::Char('p' | 'P') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.move_up();
            ComposerAction::Continue
        }
        KeyCode::Down if plain_movement_modifiers(key.modifiers) => {
            input.move_down();
            ComposerAction::Continue
        }
        KeyCode::Char('n' | 'N') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.move_down();
            ComposerAction::Continue
        }
        KeyCode::Home if plain_movement_modifiers(key.modifiers) => {
            input.move_line_start();
            ComposerAction::Continue
        }
        KeyCode::Char('a' | 'A') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.move_line_start();
            ComposerAction::Continue
        }
        KeyCode::End if plain_movement_modifiers(key.modifiers) => {
            input.move_line_end();
            ComposerAction::Continue
        }
        KeyCode::Char('e' | 'E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.move_line_end();
            ComposerAction::Continue
        }
        KeyCode::Backspace => {
            input.backspace();
            ComposerAction::Continue
        }
        KeyCode::Delete => {
            input.delete();
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
            input.insert_char(normalize_char(value, key.modifiers));
            ComposerAction::Continue
        }
        _ => ComposerAction::Continue,
    }
}

fn plain_movement_modifiers(modifiers: KeyModifiers) -> bool {
    !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn normalize_char(value: char, modifiers: KeyModifiers) -> char {
    if modifiers.contains(KeyModifiers::SHIFT) {
        value.to_ascii_uppercase()
    } else {
        value
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

    move_cursor_to_input(stdout, input)?;
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

fn move_cursor_to_input(stdout: &mut impl Write, input: &ComposerInput) -> std::io::Result<()> {
    let cursor_row = input.text[..input.cursor]
        .chars()
        .filter(|value| *value == '\n')
        .count();
    let total_rows = row_count(input);
    let rows_up = total_rows.saturating_sub(cursor_row + 1);
    if rows_up > 0 {
        queue!(stdout, cursor::MoveUp(rows_up as u16))?;
    }

    let line_start = input.line_start();
    let column = 2 + input.text[line_start..input.cursor].chars().count();
    queue!(stdout, cursor::MoveToColumn(column as u16))
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

        let bracket_result = execute!(
            stdout,
            PopKeyboardEnhancementFlags,
            DisableModifyOtherKeys,
            DisableBracketedPaste,
        )
        .map_err(|source| Error::io("<stdout>", source));
        let raw_result =
            terminal::disable_raw_mode().map_err(|source| Error::io("<terminal>", source));
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
    fn cursor_movement_edits_inside_input() {
        let mut input = ComposerInput::default();
        input.insert_paste("abcd");

        assert_eq!(
            key(&mut input, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            ComposerAction::Continue
        );
        assert_eq!(
            key(&mut input, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            ComposerAction::Continue
        );
        assert_eq!(
            key(
                &mut input,
                KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE)
            ),
            ComposerAction::Continue
        );
        assert_eq!(input.text, "abXcd");
        assert_eq!(input.cursor, 3);

        assert_eq!(
            key(
                &mut input,
                KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)
            ),
            ComposerAction::Continue
        );
        assert_eq!(
            input.submit(),
            ComposerSubmission::Answer("abcd".to_string())
        );
    }

    #[test]
    fn home_end_and_control_shortcuts_move_within_current_line() {
        let mut input = ComposerInput::default();
        input.insert_paste("one\ntwo");

        assert_eq!(
            key(
                &mut input,
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
            ),
            ComposerAction::Continue
        );
        input.insert_paste("start ");
        assert_eq!(input.text, "one\nstart two");

        assert_eq!(
            key(
                &mut input,
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)
            ),
            ComposerAction::Continue
        );
        input.insert_paste(" end");
        assert_eq!(
            input.submit(),
            ComposerSubmission::Answer("one\nstart two end".to_string())
        );
    }

    #[test]
    fn up_down_preserve_character_column_across_lines() {
        let mut input = ComposerInput::default();
        input.insert_paste("ab\ncdef\ngh");
        input.move_line_start();
        input.move_right();
        input.move_right();

        assert_eq!(
            key(&mut input, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            ComposerAction::Continue
        );
        input.insert_char('X');
        assert_eq!(input.text, "ab\ncdXef\ngh");

        assert_eq!(
            key(&mut input, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ComposerAction::Continue
        );
        input.insert_char('Y');
        assert_eq!(
            input.submit(),
            ComposerSubmission::Answer("ab\ncdXef\nghY".to_string())
        );
    }

    #[test]
    fn shifted_letter_inserts_uppercase() {
        let mut input = ComposerInput::default();

        let action = key(
            &mut input,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT),
        );

        assert_eq!(action, ComposerAction::Continue);
        assert_eq!(input.submit(), ComposerSubmission::Answer("A".to_string()));
    }

    #[test]
    fn separate_shift_modifier_state_inserts_uppercase() {
        let mut state = ModifierState::default();
        assert!(state.update(KeyEvent::new_with_kind(
            KeyCode::Modifier(ModifierKeyCode::LeftShift),
            KeyModifiers::SHIFT,
            KeyEventKind::Press,
        )));

        let mut input = ComposerInput::default();
        let action = key(
            &mut input,
            state.apply(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
        );

        assert_eq!(action, ComposerAction::Continue);
        assert_eq!(input.submit(), ComposerSubmission::Answer("A".to_string()));

        assert!(state.update(KeyEvent::new_with_kind(
            KeyCode::Modifier(ModifierKeyCode::LeftShift),
            KeyModifiers::SHIFT,
            KeyEventKind::Release,
        )));
        assert_eq!(state.active, KeyModifiers::NONE);
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
