use termimad::MadSkin;

pub(super) fn markdown_to_string(markdown: &str, color: bool) -> String {
    let markdown = markdown.trim_end_matches(['\r', '\n']);
    if markdown.trim().is_empty() {
        return String::new();
    }

    let skin = if color {
        MadSkin::default_dark()
    } else {
        MadSkin::no_style()
    };

    skin.term_text(markdown)
        .to_string()
        .trim_end_matches(['\r', '\n'])
        .to_string()
}
