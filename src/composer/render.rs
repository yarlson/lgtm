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
    redraw_for_width(stdout, input, previous, terminal_width())
}

fn redraw_for_width(
    stdout: &mut impl Write,
    input: &ComposerInput,
    previous: RenderedInput,
    width: usize,
) -> std::io::Result<RenderedInput> {
    clear_previous(stdout, previous)?;

    let layout = input_layout(input, width);
    for (index, row) in layout.rows.iter().enumerate() {
        if index > 0 {
            write!(stdout, "\r\n{row}")?;
        } else {
            write!(stdout, "{row}")?;
        }
    }

    move_cursor_to_input(stdout, &layout)?;
    stdout.flush()?;
    Ok(RenderedInput::from_layout(&layout))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct RenderedInput {
    rows: usize,
    cursor_row: usize,
}

impl RenderedInput {
    fn from_layout(layout: &InputLayout) -> Self {
        Self {
            rows: layout.rows.len(),
            cursor_row: layout.cursor_row,
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(rows: usize, cursor_row: usize) -> Self {
        Self { rows, cursor_row }
    }

    #[cfg(test)]
    fn from_input_for_test(input: &ComposerInput, width: usize) -> Self {
        Self::from_layout(&input_layout(input, width))
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

fn move_cursor_to_input(stdout: &mut impl Write, layout: &InputLayout) -> std::io::Result<()> {
    let rows_up = layout.rows.len().saturating_sub(layout.cursor_row + 1);
    if rows_up > 0 {
        queue!(stdout, cursor::MoveUp(rows_up as u16))?;
    }

    queue!(stdout, cursor::MoveToColumn(layout.cursor_column as u16))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InputLayout {
    rows: Vec<String>,
    cursor_row: usize,
    cursor_column: usize,
}

fn input_layout(input: &ComposerInput, width: usize) -> InputLayout {
    let wrap_width = width.saturating_sub(1).max(1);
    let mut rows = Vec::new();
    let mut current = String::from("> ");
    let mut current_width = current.chars().count();
    let mut cursor = None;

    for (index, ch) in input.text.char_indices() {
        if cursor.is_none() && input.cursor == index {
            cursor = Some((rows.len(), current_width));
        }

        if ch == '\n' {
            rows.push(current);
            current = String::from("  ");
            current_width = current.chars().count();
            let after_newline = index + ch.len_utf8();
            if cursor.is_none() && input.cursor == after_newline {
                cursor = Some((rows.len(), current_width));
            }
            continue;
        }

        if current_width >= wrap_width {
            rows.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += 1;
    }

    let (cursor_row, cursor_column) = cursor.unwrap_or((rows.len(), current_width));
    rows.push(current);

    InputLayout {
        rows,
        cursor_row,
        cursor_column,
    }
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .ok()
        .and_then(|(columns, _rows)| (columns > 0).then_some(columns as usize))
        .unwrap_or(80)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_input_counts_soft_wrapped_rows() {
        let mut input = ComposerInput::default();
        input.insert_paste("abcdefghij");

        assert_eq!(
            RenderedInput::from_input_for_test(&input, 5),
            RenderedInput::for_test(3, 2)
        );
    }

    #[test]
    fn rendered_input_counts_explicit_and_soft_wrapped_rows() {
        let mut input = ComposerInput::default();
        input.insert_paste("abcdef\nxy");

        assert_eq!(
            RenderedInput::from_input_for_test(&input, 5),
            RenderedInput::for_test(3, 2)
        );
    }

    #[test]
    fn cursor_column_stays_inside_wrapped_terminal_width() {
        let mut input = ComposerInput::default();
        input.insert_paste("abcdef");
        input.cursor = 4;

        assert_eq!(
            input_layout(&input, 5),
            InputLayout {
                rows: vec!["> ab".to_string(), "cdef".to_string()],
                cursor_row: 1,
                cursor_column: 2,
            }
        );
    }

    #[test]
    fn layout_hard_wraps_rows_before_terminal_auto_wrap_column() {
        let mut input = ComposerInput::default();
        input.insert_paste("abcdefghij");

        assert_eq!(
            input_layout(&input, 5).rows,
            vec!["> ab".to_string(), "cdef".to_string(), "ghij".to_string()]
        );
    }

    #[test]
    fn redraw_clears_all_soft_wrapped_previous_rows() {
        let mut previous_input = ComposerInput::default();
        previous_input.insert_paste("abcdefghij");
        let mut rendered = Vec::new();

        let previous =
            redraw_for_width(&mut rendered, &previous_input, RenderedInput::default(), 5)
                .expect("initial redraw");

        assert_eq!(previous, RenderedInput::for_test(3, 2));

        let mut next_input = ComposerInput::default();
        next_input.insert_paste("a");
        rendered.clear();
        redraw_for_width(&mut rendered, &next_input, previous, 5).expect("next redraw");

        let output = String::from_utf8(rendered).expect("utf8");
        assert_eq!(output.matches("\x1b[2K").count(), 3);
    }
}
