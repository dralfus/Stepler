use std::io::{self, Read};
use std::process::ExitCode;

use stepler_core::{
    build_replacement_plan, Capabilities, CorrectionError, CorrectionMode, MethodBinding, MethodId,
    TextContext, TextRange,
};

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<String, String> {
    let request = Request::parse(std::env::args().skip(1))?;
    let mut line = String::new();
    io::stdin()
        .read_to_string(&mut line)
        .map_err(|error| format!("failed to read stdin: {error}"))?;

    Ok(plan_line(&line, request.mode, request.point_chars))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Request {
    mode: CorrectionMode,
    point_chars: usize,
}

impl Request {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut mode = None;
        let mut point_chars = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "bash" => {}
                "--mode" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("--mode requires a value"))?;
                    mode = Some(parse_mode(&value)?);
                }
                "--point" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("--point requires a value"))?;
                    point_chars = Some(
                        value
                            .parse::<usize>()
                            .map_err(|error| format!("invalid --point value: {error}"))?,
                    );
                }
                "--point-units" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("--point-units requires a value"))?;
                    if value != "chars" {
                        return Err(String::from("only --point-units chars is supported"));
                    }
                }
                "-h" | "--help" => return Err(usage()),
                other => return Err(format!("unknown argument: {other}\n{}", usage())),
            }
        }

        Ok(Self {
            mode: mode.ok_or_else(usage)?,
            point_chars: point_chars.ok_or_else(|| String::from("--point is required"))?,
        })
    }
}

fn usage() -> String {
    String::from(
        "usage: stepler-remote bash --mode pause|scrolllock --point <readline-point> [--point-units chars]",
    )
}

fn parse_mode(value: &str) -> Result<CorrectionMode, String> {
    match value.to_ascii_lowercase().as_str() {
        "pause" | "p" => Ok(CorrectionMode::Pause),
        "scrolllock" | "cp" | "ctrl-pause" | "control-pause" => Ok(CorrectionMode::ScrollLock),
        _ => Err(format!("unsupported mode: {value}")),
    }
}

fn plan_line(line: &str, mode: CorrectionMode, point_chars: usize) -> String {
    let point_byte = char_offset_to_byte(line, point_chars).unwrap_or(line.len());
    match convert_line(line, mode, point_byte) {
        Ok(result) => format!(
            "ok\t{}\t{}",
            byte_offset_to_char(&result.line, result.point_byte),
            encode_base64(result.line.as_bytes())
        ),
        Err(CorrectionError::NoTextToReplace) => format!(
            "noop\t{}\t{}",
            point_chars.min(line.chars().count()),
            encode_base64(line.as_bytes())
        ),
        Err(error) => format!(
            "error\t{}\t{}",
            point_chars.min(line.chars().count()),
            encode_base64(format!("{error:?}").as_bytes())
        ),
    }
}

fn convert_line(
    line: &str,
    mode: CorrectionMode,
    point_byte: usize,
) -> Result<ConversionResult, CorrectionError> {
    let context = TextContext {
        app_id: String::from("ssh-remote/bash"),
        window_id: String::from("readline"),
        control_id: String::from("readline-line"),
        text_snapshot: line.to_owned(),
        caret_range: TextRange::caret(point_byte),
        selection_range: None,
        capabilities: Capabilities {
            can_replace_directly: true,
            can_read_selection: false,
            can_read_caret: true,
            method_binding: Some(MethodBinding::new(
                MethodId::SshTerminal,
                vec![MethodId::SshTerminal],
            )),
        },
        telemetry: Default::default(),
    };
    let plan = build_replacement_plan(&context, mode)?;
    let actual = context
        .text_snapshot
        .get(plan.range.start..plan.range.end)
        .ok_or(CorrectionError::InvalidRange)?;
    if actual != plan.expected_before_text {
        return Err(CorrectionError::InvalidRange);
    }

    let mut new_line = String::with_capacity(
        context.text_snapshot.len() + plan.replacement_text.len()
            - (plan.range.end - plan.range.start),
    );
    new_line.push_str(&context.text_snapshot[..plan.range.start]);
    new_line.push_str(&plan.replacement_text);
    new_line.push_str(&context.text_snapshot[plan.range.end..]);

    let point_byte =
        adjusted_cursor_after_replacement(point_byte, plan.range, plan.replacement_text.len());
    Ok(ConversionResult {
        line: new_line,
        point_byte,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConversionResult {
    line: String,
    point_byte: usize,
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

fn char_offset_to_byte(text: &str, target_chars: usize) -> Option<usize> {
    if target_chars == text.chars().count() {
        return Some(text.len());
    }
    text.char_indices()
        .nth(target_chars)
        .map(|(byte_index, _)| byte_index)
}

fn byte_offset_to_char(text: &str, byte_offset: usize) -> usize {
    if byte_offset >= text.len() {
        return text.chars().count();
    }
    text[..byte_offset].chars().count()
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
        output.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_converts_word_before_cursor() {
        let result = convert_line("echo ghbdtn", CorrectionMode::Pause, "echo ghbdtn".len())
            .expect("pause plan");

        assert_eq!(result.line, "echo привет");
        assert_eq!(result.point_byte, "echo привет".len());
    }

    #[test]
    fn pause_accepts_trailing_space() {
        let result = convert_line("echo ltym/ ", CorrectionMode::Pause, "echo ltym/ ".len())
            .expect("pause plan");

        assert_eq!(result.line, "echo день. ");
        assert_eq!(result.point_byte, "echo день. ".len());
    }

    #[test]
    fn scrolllock_converts_likely_phrase() {
        let result = convert_line(
            "house dfkmc поле long привет мир",
            CorrectionMode::ScrollLock,
            "house dfkmc поле long привет мир".len(),
        )
        .expect("scrolllock plan");

        assert_eq!(result.line, "house вальс поле long привет мир");
        assert_eq!(result.point_byte, "house вальс поле long привет мир".len());
    }

    #[test]
    fn output_is_base64_encoded_for_bash() {
        let output = plan_line("ghbdtn", CorrectionMode::Pause, 6);

        assert_eq!(output, "ok\t6\t0L/RgNC40LLQtdGC");
    }

    #[test]
    fn readline_char_point_is_converted_to_byte_offset() {
        let byte = char_offset_to_byte("пше git", 3).unwrap();

        assert_eq!(&"пше git"[..byte], "пше");
        assert_eq!(byte_offset_to_char("пше git", byte), 3);
    }
}
