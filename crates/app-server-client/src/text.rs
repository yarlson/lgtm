pub(crate) fn non_empty(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn preview(text: &str, max_lines: usize, max_chars: usize) -> String {
    let mut result = String::new();
    let mut truncated = false;

    for (index, line) in text.lines().enumerate() {
        if index >= max_lines {
            truncated = true;
            break;
        }

        if !result.is_empty() {
            result.push('\n');
        }

        let remaining = max_chars.saturating_sub(result.len());
        let line_preview = prefix_by_char_boundary(line, remaining);
        if line_preview.len() < line.len() {
            result.push_str(line_preview);
            truncated = true;
            break;
        }
        result.push_str(line_preview);
    }

    if result.is_empty() && !text.is_empty() {
        let text_preview = prefix_by_char_boundary(text, max_chars);
        result.push_str(text_preview);
        truncated = text_preview.len() < text.len();
    }

    if truncated {
        result.push_str("\n...");
    }

    result
}

fn prefix_by_char_boundary(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }

    let mut end = 0;
    for (index, _) in text.char_indices() {
        if index > max_bytes {
            break;
        }
        end = index;
    }
    &text[..end]
}
