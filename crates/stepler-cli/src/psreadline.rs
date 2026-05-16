use stepler_core::{
    build_replacement_plan, Capabilities, CorrectionMode, MethodBinding, MethodId, TextContext,
    TextRange,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsReadLineRequest {
    pub mode: CorrectionMode,
    pub text_b64: String,
    pub cursor_utf16: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsReadLinePlan {
    pub json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsReadLineError {
    InvalidText(String),
    InvalidCursor,
    Planning(String),
    InvalidReplacementRange,
    PreflightMismatch,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PsReadLineMethod;

impl PsReadLineMethod {
    pub fn plan(&self, request: PsReadLineRequest) -> Result<PsReadLinePlan, PsReadLineError> {
        let text =
            decode_utf16le_base64(&request.text_b64).map_err(PsReadLineError::InvalidText)?;
        let cursor_byte = utf16_offset_to_byte(&text, request.cursor_utf16)
            .ok_or(PsReadLineError::InvalidCursor)?;
        let context = self.context(text, cursor_byte);

        let plan = build_replacement_plan(&context, request.mode)
            .map_err(|error| PsReadLineError::Planning(format!("{error:?}")))?;
        let Some(actual) = context.text_snapshot.get(plan.range.start..plan.range.end) else {
            return Err(PsReadLineError::InvalidReplacementRange);
        };
        if actual != plan.expected_before_text {
            return Err(PsReadLineError::PreflightMismatch);
        }

        let new_text = format!(
            "{}{}{}",
            &context.text_snapshot[..plan.range.start],
            plan.replacement_text,
            &context.text_snapshot[plan.range.end..]
        );
        let new_cursor_byte =
            adjusted_cursor_after_replacement(cursor_byte, plan.range, plan.replacement_text.len());
        let new_cursor_utf16 = byte_offset_to_utf16(&new_text, new_cursor_byte);

        Ok(PsReadLinePlan {
            json: format!(
                "{{\"applied\":true,\"mode\":{},\"range_start\":{},\"range_end\":{},\"expected\":{},\"expected_b64\":{},\"replacement\":{},\"replacement_b64\":{},\"text\":{},\"text_b64\":{},\"cursor\":{},\"confidence\":{:.3}}}",
                json_string(match request.mode {
                    CorrectionMode::Pause => "pause",
                    CorrectionMode::ScrollLock => "scrolllock",
                }),
                byte_offset_to_utf16(&context.text_snapshot, plan.range.start),
                byte_offset_to_utf16(&context.text_snapshot, plan.range.end),
                json_string(&plan.expected_before_text),
                json_string(&encode_utf16le_base64(&plan.expected_before_text)),
                json_string(&plan.replacement_text),
                json_string(&encode_utf16le_base64(&plan.replacement_text)),
                json_string(&new_text),
                json_string(&encode_utf16le_base64(&new_text)),
                new_cursor_utf16,
                plan.confidence
            ),
        })
    }

    fn context(&self, text: String, cursor_byte: usize) -> TextContext {
        TextContext {
            app_id: String::from("PowerShell/PSReadLine"),
            window_id: String::from("psreadline"),
            control_id: String::from("psreadline-buffer"),
            text_snapshot: text,
            caret_range: TextRange::caret(cursor_byte),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: false,
                can_read_caret: true,
                method_binding: Some(MethodBinding::new(
                    MethodId::PsReadLine,
                    vec![MethodId::PsReadLine],
                )),
            },
        }
    }
}

pub fn self_test_lines() -> Result<Vec<String>, PsReadLineError> {
    let cases = [
        (
            "scrolllock command",
            CorrectionMode::ScrollLock,
            "пше",
            "\"replacement\":\"git\"",
        ),
        (
            "pause word",
            CorrectionMode::Pause,
            "k.,jdm",
            "\"replacement\":\"любовь\"",
        ),
    ];
    let mut lines = Vec::with_capacity(cases.len());
    for (name, mode, text, expected_json_fragment) in cases {
        let cursor_utf16 = text.encode_utf16().count();
        let plan = PsReadLineMethod.plan(PsReadLineRequest {
            mode,
            text_b64: encode_utf16le_base64(text),
            cursor_utf16,
        })?;
        if !plan.json.contains(expected_json_fragment) {
            return Err(PsReadLineError::Planning(format!(
                "{name} produced unexpected JSON: {}",
                plan.json
            )));
        }
        lines.push(format!("{name}: ok"));
    }
    Ok(lines)
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

fn utf16_offset_to_byte(text: &str, target_utf16: usize) -> Option<usize> {
    let mut utf16_count = 0usize;
    for (byte_index, ch) in text.char_indices() {
        if utf16_count == target_utf16 {
            return Some(byte_index);
        }
        utf16_count += ch.len_utf16();
        if utf16_count > target_utf16 {
            return None;
        }
    }
    (utf16_count == target_utf16).then_some(text.len())
}

fn byte_offset_to_utf16(text: &str, byte_offset: usize) -> usize {
    if byte_offset >= text.len() {
        return text.encode_utf16().count();
    }
    text[..byte_offset].encode_utf16().count()
}

fn decode_utf16le_base64(value: &str) -> Result<String, String> {
    let bytes = decode_base64(value)?;
    if bytes.len() % 2 != 0 {
        return Err(String::from("decoded UTF-16LE byte length is odd"));
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|error| format!("invalid UTF-16LE text: {error}"))
}

fn encode_utf16le_base64(value: &str) -> String {
    let bytes = value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    encode_base64(&bytes)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0u8;
    let mut padding_seen = false;

    for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            padding_seen = true;
            continue;
        }
        if padding_seen {
            return Err(String::from("non-padding base64 byte after padding"));
        }
        let Some(value) = base64_value(byte) else {
            return Err(format!("invalid base64 byte 0x{byte:02X}"));
        };
        accumulator = (accumulator << 6) | value as u32;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            buffer.push(((accumulator >> bits) & 0xFF) as u8);
        }
    }

    Ok(buffer)
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;
        output.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_offsets_round_trip_for_cyrillic_text() {
        let text = "пше git";
        let utf16 = 3;
        let byte = utf16_offset_to_byte(text, utf16).unwrap();

        assert_eq!(&text[..byte], "пше");
        assert_eq!(byte_offset_to_utf16(text, byte), utf16);
    }

    #[test]
    fn decodes_utf16le_base64_from_powershell() {
        let text = decode_utf16le_base64("PwRIBDUE").unwrap();

        assert_eq!(text, "пше");
    }

    #[test]
    fn encodes_utf16le_base64_for_powershell() {
        assert_eq!(encode_utf16le_base64("любовь"), "OwROBDEEPgQyBEwE");
    }

    #[test]
    fn adjusts_cursor_inside_replacement_to_replacement_end() {
        let range = TextRange::new(0, "пше".len());
        let cursor = adjusted_cursor_after_replacement("пше".len(), range, "git".len());

        assert_eq!(cursor, "git".len());
    }

    #[test]
    fn psreadline_method_builds_json_plan() {
        let plan = PsReadLineMethod
            .plan(PsReadLineRequest {
                mode: CorrectionMode::ScrollLock,
                text_b64: encode_utf16le_base64("пше"),
                cursor_utf16: 3,
            })
            .unwrap();

        assert!(plan.json.contains("\"replacement\":\"git\""));
        assert!(plan.json.contains("\"text\":\"git\""));
        assert!(plan.json.contains("\"cursor\":3"));
    }

    #[test]
    fn psreadline_self_test_covers_command_and_word() {
        let lines = self_test_lines().unwrap();

        assert_eq!(lines, vec!["scrolllock command: ok", "pause word: ok"]);
    }
}
