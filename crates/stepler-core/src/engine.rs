use crate::language::is_converted_phrase_more_likely;
use crate::layout::{convert_layout_text, convert_selected_text};
use crate::types::{CorrectionMode, ReplacementPlan, TextContext, TextRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionError {
    InvalidRange,
    NoTextToReplace,
    UnsupportedMode,
}

pub fn build_replacement_plan(
    context: &TextContext,
    mode: CorrectionMode,
) -> Result<ReplacementPlan, CorrectionError> {
    match mode {
        CorrectionMode::Pause => build_pause_plan(context),
        CorrectionMode::ScrollLock => build_scroll_lock_plan(context),
    }
}

fn build_pause_plan(context: &TextContext) -> Result<ReplacementPlan, CorrectionError> {
    let range = context
        .selection_range
        .filter(|range| !range.is_empty())
        .unwrap_or_else(|| {
            word_range_before_or_around_caret(&context.text_snapshot, context.caret_range.start)
        });

    let expected = slice_by_range(&context.text_snapshot, range)?;
    if expected.trim().is_empty() {
        return Err(CorrectionError::NoTextToReplace);
    }

    let replacement = if context.selection_range.is_some() {
        convert_selected_text(expected)
    } else {
        convert_layout_text(expected)
    };

    Ok(ReplacementPlan {
        range,
        replacement_text: replacement,
        reason: String::from("pause_layout_conversion"),
        confidence: 1.0,
        expected_before_text: expected.to_owned(),
    })
}

fn build_scroll_lock_plan(context: &TextContext) -> Result<ReplacementPlan, CorrectionError> {
    let scan_end = token_end_at_or_after_caret(&context.text_snapshot, context.caret_range.start);
    if !context.text_snapshot.is_char_boundary(scan_end) {
        return Err(CorrectionError::InvalidRange);
    }

    let left_text = &context.text_snapshot[..scan_end];
    if let Some(candidate) = best_sparse_line_candidate(left_text) {
        return Ok(ReplacementPlan {
            range: TextRange::new(candidate.start, candidate.end),
            replacement_text: candidate.replacement,
            reason: String::from("scrolllock_sparse_token_score"),
            confidence: candidate.confidence,
            expected_before_text: candidate.source,
        });
    }

    let Some(candidate) = best_trailing_candidate(left_text, 12) else {
        return Err(CorrectionError::NoTextToReplace);
    };

    Ok(ReplacementPlan {
        range: TextRange::new(candidate.start, scan_end),
        replacement_text: candidate.replacement,
        reason: String::from("scrolllock_language_score"),
        confidence: candidate.confidence,
        expected_before_text: candidate.source,
    })
}

fn slice_by_range(text: &str, range: TextRange) -> Result<&str, CorrectionError> {
    if range.start > range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err(CorrectionError::InvalidRange);
    }

    Ok(&text[range.start..range.end])
}

fn word_range_before_or_around_caret(text: &str, caret: usize) -> TextRange {
    let caret = caret.min(text.len());
    if !text.is_char_boundary(caret) {
        return TextRange::caret(caret);
    }

    let mut start = caret;
    let mut end = caret;

    while start > 0 {
        let Some((prev_index, prev_ch)) = text[..start].char_indices().next_back() else {
            break;
        };
        if prev_ch.is_whitespace() {
            break;
        }
        start = prev_index;
    }

    while end < text.len() {
        let Some(next_ch) = text[end..].chars().next() else {
            break;
        };
        if next_ch.is_whitespace() {
            break;
        }
        end += next_ch.len_utf8();
    }

    TextRange::new(start, end)
}

fn token_end_at_or_after_caret(text: &str, caret: usize) -> usize {
    let caret = caret.min(text.len());
    if !text.is_char_boundary(caret) {
        return caret;
    }

    let inside_or_after_token = text[..caret]
        .chars()
        .next_back()
        .is_some_and(|ch| !ch.is_whitespace())
        || text[caret..]
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace());

    if !inside_or_after_token {
        return caret;
    }

    let mut end = caret;
    while end < text.len() {
        let Some(ch) = text[end..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            break;
        }
        end += ch.len_utf8();
    }

    end
}

struct ScrollLockCandidate {
    start: usize,
    end: usize,
    source: String,
    replacement: String,
    confidence: f32,
}

fn best_trailing_candidate(text: &str, max_tokens: usize) -> Option<ScrollLockCandidate> {
    let tokens = token_spans(text);
    if tokens.is_empty() {
        return None;
    }

    let mut best = None;
    for token_count in 1..=max_tokens.min(tokens.len()) {
        let start = tokens[tokens.len() - token_count].0;
        let source = text[start..].trim_start();
        if source.is_empty() {
            continue;
        }
        if token_count > 1 && starts_with_ascii_acronym(source) {
            continue;
        }

        let replacement = convert_layout_text(source);
        if replacement == source {
            continue;
        }

        if let Some(confidence) = is_converted_phrase_more_likely(source, &replacement) {
            let candidate = ScrollLockCandidate {
                start,
                end: text.len(),
                source: source.to_owned(),
                replacement,
                confidence,
            };

            if best.as_ref().map_or(true, |current: &ScrollLockCandidate| {
                candidate.confidence > current.confidence
                    || (candidate.confidence == current.confidence
                        && candidate.source.len() > current.source.len())
            }) {
                best = Some(candidate);
            }
        }
    }

    best
}

fn best_sparse_line_candidate(text: &str) -> Option<ScrollLockCandidate> {
    let line_start = text
        .char_indices()
        .rev()
        .find(|(_, ch)| *ch == '\n' || *ch == '\r')
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let line = &text[line_start..];
    let tokens = token_spans(line);
    if tokens.is_empty() {
        return None;
    }

    let mut converted = Vec::new();
    for (token_start, token_end) in tokens {
        let token = &line[token_start..token_end];
        if starts_with_ascii_acronym(token) {
            continue;
        }

        let replacement = convert_layout_text(token);
        if replacement == token {
            continue;
        }

        if let Some(confidence) = is_converted_phrase_more_likely(token, &replacement) {
            converted.push(TokenConversion {
                start: token_start,
                end: token_end,
                replacement,
                confidence,
            });
        }
    }

    let first = converted.first()?;
    let last = converted.last()?;
    if converted.len() == 1 {
        return Some(ScrollLockCandidate {
            start: line_start + first.start,
            end: line_start + first.end,
            source: line[first.start..first.end].to_owned(),
            replacement: first.replacement.clone(),
            confidence: first.confidence,
        });
    }

    let source = &line[first.start..last.end];
    let mut replacement = String::with_capacity(source.len());
    let mut cursor = first.start;
    for conversion in &converted {
        replacement.push_str(&line[cursor..conversion.start]);
        replacement.push_str(&conversion.replacement);
        cursor = conversion.end;
    }
    replacement.push_str(&line[cursor..last.end]);

    Some(ScrollLockCandidate {
        start: line_start + first.start,
        end: line_start + last.end,
        source: source.to_owned(),
        replacement,
        confidence: converted
            .iter()
            .map(|conversion| conversion.confidence)
            .sum::<f32>()
            / converted.len() as f32,
    })
}

struct TokenConversion {
    start: usize,
    end: usize,
    replacement: String,
    confidence: f32,
}

fn token_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;

    for (index, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(token_start) = start.take() {
                spans.push((token_start, index));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }

    if let Some(token_start) = start {
        spans.push((token_start, text.len()));
    }

    spans
}

fn starts_with_ascii_acronym(text: &str) -> bool {
    let Some(first_token) = text.split_whitespace().next() else {
        return false;
    };

    let mut has_letter = false;
    for ch in first_token.chars() {
        if ch.is_ascii_alphabetic() {
            has_letter = true;
            if !ch.is_ascii_uppercase() {
                return false;
            }
        } else {
            return false;
        }
    }

    has_letter && first_token.chars().count() >= 2
}
