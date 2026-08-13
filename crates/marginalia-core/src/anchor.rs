use crate::Comment;

/// Where a comment's quoted lines sit in the file as it reads now, as 1-based
/// row numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    Anchored { start_row: usize, end_row: usize },
    Drifted { start_row_when_written: usize },
}

/// A pending comment together with the rows its quote occupies today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchoredComment {
    pub comment: Comment,
    pub anchor: Anchor,
}

/// The rows `quote` occupies in `file_text`, or drift when it is gone.
///
/// Lines match with whitespace trimmed at both ends, so reindentation alone is
/// not drift. Of several occurrences the one nearest `hint_row` wins; of none,
/// the answer is drift rather than a guess.
pub fn locate(quote: &str, file_text: &str, hint_row: usize) -> Anchor {
    let drifted = Anchor::Drifted {
        start_row_when_written: hint_row,
    };
    let quote_lines: Vec<&str> = quote.lines().map(str::trim).collect();
    if quote_lines.is_empty() {
        return drifted;
    }
    let file_lines: Vec<&str> = file_text.lines().map(str::trim).collect();
    if quote_lines.len() > file_lines.len() {
        return drifted;
    }
    let nearest = file_lines
        .windows(quote_lines.len())
        .enumerate()
        .filter(|(_, window)| *window == quote_lines.as_slice())
        .map(|(index, _)| index + 1)
        .min_by_key(|start_row| start_row.abs_diff(hint_row));
    match nearest {
        Some(start_row) => Anchor::Anchored {
            start_row,
            end_row: start_row + quote_lines.len() - 1,
        },
        None => drifted,
    }
}
