use stepler_core::{TextContext, TextRange};

pub fn context_with_caret(text: &str, caret: usize) -> TextContext {
    TextContext::new(text).with_caret(TextRange::caret(caret))
}

pub fn context_with_selection(text: &str, start: usize, end: usize) -> TextContext {
    TextContext::new(text).with_selection(Some(TextRange::new(start, end)))
}
