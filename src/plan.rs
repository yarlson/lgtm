use std::fs;
use std::path::Path;

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase {
    pub number: u32,
    pub title: String,
    pub heading: String,
}

pub fn load(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|source| Error::io(path, source))
}

pub fn require_file(path: &Path, display: &Path) -> Result<(), Error> {
    if path.is_file() {
        Ok(())
    } else {
        Err(Error::message(format!(
            "required file {} was not found",
            display.display()
        )))
    }
}

pub fn detect_end_phase(plan: &str) -> Option<u32> {
    phase_headings(plan)
        .into_iter()
        .map(|phase| phase.number)
        .max()
}

pub fn phase(plan: &str, phase: u32) -> Option<Phase> {
    phase_headings(plan)
        .into_iter()
        .find(|heading| heading.number == phase)
}

pub fn phase_headings(plan: &str) -> Vec<Phase> {
    plan.lines().filter_map(parse_phase_heading).collect()
}

fn parse_phase_heading(line: &str) -> Option<Phase> {
    let heading = line.trim_end().to_string();
    let rest = heading.strip_prefix("## Phase ")?;
    let digits_len = rest
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .map(|(idx, ch)| idx + ch.len_utf8())
        .last()?;
    let number = rest[..digits_len].parse().ok()?;
    let rest = rest[digits_len..].trim_start();
    let title = rest
        .strip_prefix('-')
        .or_else(|| rest.strip_prefix(':'))?
        .trim();
    if title.is_empty() {
        return None;
    }
    Some(Phase {
        number,
        title: title.to_string(),
        heading,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_phase_headings_with_dash_or_colon() {
        let plan = "\
# Plan

## Phase 1 - Skeleton
body
## Phase 12: Polish
";

        assert_eq!(
            phase_headings(plan),
            vec![
                Phase {
                    number: 1,
                    title: "Skeleton".to_string(),
                    heading: "## Phase 1 - Skeleton".to_string(),
                },
                Phase {
                    number: 12,
                    title: "Polish".to_string(),
                    heading: "## Phase 12: Polish".to_string(),
                },
            ]
        );
        assert_eq!(detect_end_phase(plan), Some(12));
        assert_eq!(
            phase(plan, 12).map(|phase| phase.heading),
            Some("## Phase 12: Polish".to_string())
        );
    }
}
