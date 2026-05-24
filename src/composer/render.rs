use std::io::Write;

use crossterm::{
    cursor, queue,
    terminal::{Clear, ClearType},
};

use crate::composer::input::ComposerInput;

pub(super) fn redraw(
    stdout: &mut impl Write,
    input: &ComposerInput,
    previous: RenderedInput,
) -> std::io::Result<RenderedInput> {
    clear_previous(stdout, previous)?;

    for (index, line) in input.text.split('\n').enumerate() {
        if index > 0 {
            write!(stdout, "\r\n  {line}")?;
        } else {
            write!(stdout, "> {line}")?;
        }
    }

    move_cursor_to_input(stdout, input)?;
    stdout.flush()?;
    Ok(RenderedInput::from_input(input))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct RenderedInput {
    rows: usize,
    cursor_row: usize,
}

impl RenderedInput {
    fn from_input(input: &ComposerInput) -> Self {
        Self {
            rows: row_count(input),
            cursor_row: input_cursor_row(input),
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(rows: usize, cursor_row: usize) -> Self {
        Self { rows, cursor_row }
    }
}

pub(super) fn clear_previous(
    stdout: &mut impl Write,
    previous: RenderedInput,
) -> std::io::Result<()> {
    if previous.rows == 0 {
        return Ok(());
    }

    let rows_below_cursor = previous.rows.saturating_sub(previous.cursor_row + 1);
    if rows_below_cursor > 0 {
        queue!(stdout, cursor::MoveDown(rows_below_cursor as u16))?;
    }

    for index in 0..previous.rows {
        queue!(
            stdout,
            cursor::MoveToColumn(0),
            Clear(ClearType::CurrentLine)
        )?;
        if index + 1 < previous.rows {
            queue!(stdout, cursor::MoveUp(1))?;
        }
    }

    Ok(())
}

fn row_count(input: &ComposerInput) -> usize {
    input.text.split('\n').count()
}

fn input_cursor_row(input: &ComposerInput) -> usize {
    input.text[..input.cursor]
        .chars()
        .filter(|value| *value == '\n')
        .count()
}

fn move_cursor_to_input(stdout: &mut impl Write, input: &ComposerInput) -> std::io::Result<()> {
    let cursor_row = input_cursor_row(input);
    let total_rows = row_count(input);
    let rows_up = total_rows.saturating_sub(cursor_row + 1);
    if rows_up > 0 {
        queue!(stdout, cursor::MoveUp(rows_up as u16))?;
    }

    let line_start = input.line_start();
    let column = 2 + input.text[line_start..input.cursor].chars().count();
    queue!(stdout, cursor::MoveToColumn(column as u16))
}
