use crate::composer::ComposerSubmission;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct ComposerInput {
    pub(super) text: String,
    pub(super) cursor: usize,
}

impl ComposerInput {
    pub(super) fn insert_char(&mut self, value: char) {
        self.insert_str(&value.to_string());
    }

    pub(super) fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub(super) fn insert_paste(&mut self, value: &str) {
        self.insert_str(&normalize_paste(value));
    }

    pub(super) fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let previous = self.previous_boundary();
        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
    }

    pub(super) fn delete(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }
        let next = self.next_boundary();
        self.text.replace_range(self.cursor..next, "");
    }

    pub(super) fn move_left(&mut self) {
        self.cursor = self.previous_boundary();
    }

    pub(super) fn move_right(&mut self) {
        self.cursor = self.next_boundary();
    }

    pub(super) fn move_up(&mut self) {
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

    pub(super) fn move_down(&mut self) {
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

    pub(super) fn move_line_start(&mut self) {
        self.cursor = self.line_start();
    }

    pub(super) fn move_line_end(&mut self) {
        self.cursor = self.line_end();
    }

    pub(super) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub(super) fn submit(self) -> ComposerSubmission {
        match self.text.as_str() {
            "/finish" => ComposerSubmission::Finish,
            "/quit" => ComposerSubmission::Quit,
            _ => ComposerSubmission::Answer(self.text),
        }
    }

    pub(super) fn line_start(&self) -> usize {
        self.text[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1)
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
