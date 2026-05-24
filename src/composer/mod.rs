mod input;
mod render;
mod terminal;

use std::{
    io::Write,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode,
};

use crate::composer::{
    input::ComposerInput,
    render::{RenderedInput, redraw},
};

const CTRL_C_BURST_WINDOW: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerSubmission {
    Answer(String),
    Finish,
    Quit,
}

pub fn read_inline_answer() -> Result<ComposerSubmission> {
    let mut stdout = std::io::stdout();
    let mut guard = terminal::enter(&mut stdout)?;

    let result = read_inline_answer_inner(&mut stdout);
    let restore_result = guard.restore(&mut stdout);
    match (result, restore_result) {
        (Ok(submission), Ok(())) => Ok(submission),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn read_inline_answer_inner(stdout: &mut impl Write) -> Result<ComposerSubmission> {
    let mut input = ComposerInput::default();
    let mut last_ctrl_c_at = None;
    let mut modifier_state = ModifierState::default();
    let mut rendered = RenderedInput::default();
    rendered = redraw(stdout, &input, rendered).context("failed to render composer")?;

    loop {
        match event::read().context("failed to read terminal event")? {
            Event::Key(key) if modifier_state.update(key) => continue,
            Event::Key(key) if key_is_active(key) => {
                let key = modifier_state.apply(key);
                match handle_key(&mut input, key, &mut last_ctrl_c_at, Instant::now()) {
                    ComposerAction::Continue => {}
                    ComposerAction::Submit => {
                        write!(stdout, "\r\n").context("failed to finish composer line")?;
                        stdout.flush().context("failed to flush stdout")?;
                        return Ok(input.submit());
                    }
                    ComposerAction::Quit => {
                        write!(stdout, "\r\n").context("failed to finish composer line")?;
                        stdout.flush().context("failed to flush stdout")?;
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

        rendered = redraw(stdout, &input, rendered).context("failed to render composer")?;
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
    fn vertical_cursor_movement_preserves_column_when_possible() {
        let mut input = ComposerInput::default();
        input.insert_paste("abcd\nef\nghij");
        input.cursor = 3;

        input.move_down();
        assert_eq!(input.cursor, 7);

        input.move_down();
        assert_eq!(input.cursor, 10);

        input.move_up();
        assert_eq!(input.cursor, 7);
    }

    #[test]
    fn paste_normalizes_newlines() {
        let mut input = ComposerInput::default();
        input.insert_paste("a\r\nb\rc");

        assert_eq!(
            input.submit(),
            ComposerSubmission::Answer("a\nb\nc".to_string())
        );
    }

    #[test]
    fn slash_commands_submit_as_control_actions() {
        let mut finish = ComposerInput::default();
        finish.insert_paste("/finish");
        assert_eq!(finish.submit(), ComposerSubmission::Finish);

        let mut quit = ComposerInput::default();
        quit.insert_paste("/quit");
        assert_eq!(quit.submit(), ComposerSubmission::Quit);
    }

    #[test]
    fn ctrl_c_clears_once_and_quits_on_quick_second_press() {
        let mut input = ComposerInput::default();
        input.insert_paste("draft");
        let mut last_ctrl_c_at = None;
        let now = Instant::now();

        assert_eq!(
            handle_key(
                &mut input,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut last_ctrl_c_at,
                now
            ),
            ComposerAction::Continue
        );
        assert_eq!(input.text, "");

        assert_eq!(
            handle_key(
                &mut input,
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut last_ctrl_c_at,
                now + Duration::from_millis(100)
            ),
            ComposerAction::Quit
        );
    }

    #[test]
    fn shifted_letters_are_preserved_from_keyboard_enhancement_events() {
        let mut input = ComposerInput::default();

        assert_eq!(
            key(
                &mut input,
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT)
            ),
            ComposerAction::Continue
        );

        assert_eq!(input.text, "A");
    }

    #[test]
    fn clear_previous_starts_from_bottom_when_cursor_was_above_last_row() {
        let mut rendered = Vec::new();

        render::clear_previous(&mut rendered, RenderedInput::for_test(3, 0)).expect("clear");

        let output = String::from_utf8(rendered).expect("utf8");
        assert!(output.starts_with("\x1b[2B"));
    }
}
