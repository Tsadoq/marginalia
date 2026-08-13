/// The 1-based row range a selection covers, given the 1-based cursor row Zed
/// reports.
///
/// Zed exposes no start and end row for a selection, only the cursor's row, so
/// the range is the selection's line count spanned from the cursor. The cursor
/// sits at the end of a downward selection, which is the common case, so it is
/// read as the last row; a selection dragged upward therefore reports the rows
/// above the cursor rather than below it. The start is clamped at row 1, since a
/// selection that begins on the first line would otherwise underflow. An empty
/// selection is the single row the cursor is on.
pub fn derive_range(cursor_row: usize, selection: &str) -> (usize, usize) {
    let rows = selection.lines().count().max(1);
    let start_row = cursor_row.saturating_sub(rows - 1).max(1);
    (start_row, start_row + rows - 1)
}
