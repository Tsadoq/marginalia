#[path = "../src/range.rs"]
mod range;

use range::derive_range;

const MID_FILE_ROW_WHERE_NO_CLAMPING_CAN_OCCUR: usize = 10;
const THREE_LINE_SELECTION: &str = "let x = 1;\nlet y = 2;\nlet z = 3;";
const NEAR_TOP_ROW_WHOSE_UNCLAMPED_START_FALLS_BELOW_ONE: usize = 2;
const FOUR_LINE_SELECTION: &str = "fn f() {\n    g();\n    h();\n}";

#[test]
fn derive_range_spans_selection_and_clamps_at_first_row() {
    let (start_row, end_row) = derive_range(
        MID_FILE_ROW_WHERE_NO_CLAMPING_CAN_OCCUR,
        THREE_LINE_SELECTION,
    );
    assert_eq!(
        end_row + 1 - start_row,
        3,
        "spanning: a 3 line selection must span 3 rows, got {start_row}..={end_row}"
    );
    assert!(
        (start_row..=end_row).contains(&MID_FILE_ROW_WHERE_NO_CLAMPING_CAN_OCCUR),
        "spanning: the range must contain the cursor row {MID_FILE_ROW_WHERE_NO_CLAMPING_CAN_OCCUR}, got {start_row}..={end_row}"
    );

    let (clamped_start_row, _) = derive_range(
        NEAR_TOP_ROW_WHOSE_UNCLAMPED_START_FALLS_BELOW_ONE,
        FOUR_LINE_SELECTION,
    );
    assert_eq!(
        clamped_start_row, 1,
        "clamping: a selection reaching above the first row must start at row 1"
    );
}
