use stepler_core::{
    build_replacement_plan, CorrectionMode, SelectionRange, TextContext, TextRange,
};

#[derive(Debug)]
struct BehaviorRow {
    name: String,
    mode: CorrectionMode,
    context: TextContext,
    expected_range: TextRange,
    expected_replacement: String,
    expected_cursor_after: usize,
}

#[test]
fn replacement_behavior_contracts_match_cursor_and_range_expectations() {
    let rows = parse_fixture(include_str!("fixtures/replacement_behavior.tsv"));
    assert!(!rows.is_empty(), "replacement behavior fixture is empty");

    for row in rows {
        let plan = build_replacement_plan(&row.context, row.mode)
            .unwrap_or_else(|error| panic!("{}: plan failed: {:?}", row.name, error));

        assert_eq!(plan.range, row.expected_range, "{}: range", row.name);
        assert_eq!(
            plan.expected_before_text,
            slice_by_range(&row.context.text_snapshot, row.expected_range),
            "{}: expected_before_text",
            row.name
        );
        assert_eq!(
            plan.replacement_text, row.expected_replacement,
            "{}: replacement",
            row.name
        );

        let text_after = replace_range(
            &row.context.text_snapshot,
            plan.range,
            &plan.replacement_text,
        );
        let cursor_after = if row.context.selection_range.is_some() {
            plan.range.start + plan.replacement_text.len()
        } else {
            adjusted_cursor_after_replacement(
                row.context.caret_range.end,
                plan.range,
                plan.replacement_text.len(),
            )
        };

        assert_eq!(
            cursor_after, row.expected_cursor_after,
            "{}: cursor after",
            row.name
        );
        assert!(
            text_after.is_char_boundary(cursor_after),
            "{}: cursor must stay on a char boundary",
            row.name
        );
    }
}

fn parse_fixture(input: &str) -> Vec<BehaviorRow> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(parse_row)
        .collect()
}

fn parse_row(line: &str) -> BehaviorRow {
    let fields = line.split('\t').collect::<Vec<_>>();
    assert_eq!(fields.len(), 6, "bad fixture row: {line}");

    let name = fields[0].to_owned();
    let mode = parse_mode(fields[1]);
    let marked_text = unescape_field(fields[2]);
    let expected_range_marked = unescape_field(fields[3]);
    let expected_replacement = unescape_field(fields[4]);
    let expected_cursor_marked = unescape_field(fields[5]);

    let parsed_context = parse_context_markers(&marked_text);
    let expected_range = parse_range_marker(&expected_range_marked);
    let (expected_text_after, expected_cursor_after) = parse_cursor_marker(&expected_cursor_marked);

    let context = TextContext::new(parsed_context.text)
        .with_caret(TextRange::caret(parsed_context.caret))
        .with_selection(parsed_context.selection);
    let expected_text_after_from_range = replace_range(
        &context.text_snapshot,
        expected_range,
        &expected_replacement,
    );
    assert_eq!(
        expected_text_after_from_range, expected_text_after,
        "{name}: expected cursor text must match replacement"
    );

    BehaviorRow {
        name,
        mode,
        context,
        expected_range,
        expected_replacement,
        expected_cursor_after,
    }
}

#[derive(Debug)]
struct ParsedContext {
    text: String,
    caret: usize,
    selection: Option<SelectionRange>,
}

fn parse_context_markers(input: &str) -> ParsedContext {
    let mut text = String::new();
    let mut caret = None;
    let mut selection_start = None;
    let mut selection_end = None;

    for ch in input.chars() {
        match ch {
            '|' => caret = Some(text.len()),
            '[' => selection_start = Some(text.len()),
            ']' => selection_end = Some(text.len()),
            _ => text.push(ch),
        }
    }

    let selection = match (selection_start, selection_end) {
        (Some(start), Some(end)) => {
            caret = Some(end);
            Some(TextRange::new(start, end))
        }
        (None, None) => None,
        _ => panic!("selection markers must be balanced: {input:?}"),
    };

    ParsedContext {
        caret: caret.unwrap_or(text.len()),
        text,
        selection,
    }
}

fn parse_range_marker(input: &str) -> TextRange {
    let mut text = String::new();
    let mut start = None;
    let mut end = None;

    for ch in input.chars() {
        match ch {
            '{' => start = Some(text.len()),
            '}' => end = Some(text.len()),
            _ => text.push(ch),
        }
    }

    match (start, end) {
        (Some(start), Some(end)) => TextRange::new(start, end),
        _ => panic!("range markers must be present and balanced: {input:?}"),
    }
}

fn parse_cursor_marker(input: &str) -> (String, usize) {
    let mut text = String::new();
    let mut cursor = None;
    for ch in input.chars() {
        if ch == '|' {
            cursor = Some(text.len());
        } else {
            text.push(ch);
        }
    }
    (text, cursor.expect("expected cursor marker"))
}

fn parse_mode(value: &str) -> CorrectionMode {
    match value {
        "pause" => CorrectionMode::Pause,
        "scrolllock" => CorrectionMode::ScrollLock,
        _ => panic!("unknown correction mode: {value}"),
    }
}

fn unescape_field(value: &str) -> String {
    value
        .replace("\\r", "\r")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
}

fn slice_by_range(text: &str, range: TextRange) -> String {
    text[range.start..range.end].to_owned()
}

fn replace_range(text: &str, range: TextRange, replacement: &str) -> String {
    let mut result = String::new();
    result.push_str(&text[..range.start]);
    result.push_str(replacement);
    result.push_str(&text[range.end..]);
    result
}

fn adjusted_cursor_after_replacement(
    cursor: usize,
    range: TextRange,
    replacement_len: usize,
) -> usize {
    if cursor <= range.start {
        cursor
    } else if cursor <= range.end {
        range.start + replacement_len
    } else {
        cursor + replacement_len - (range.end - range.start)
    }
}
