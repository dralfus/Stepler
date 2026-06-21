pub(super) fn parse_key_value_lines(output: &str) -> std::collections::HashMap<String, String> {
    output
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

pub(super) fn byte_offset_to_utf16(text: &str, byte_offset: usize) -> usize {
    if byte_offset >= text.len() {
        return text.encode_utf16().count();
    }
    text[..byte_offset].encode_utf16().count()
}

pub(super) fn encode_utf16le_base64(value: &str) -> String {
    let bytes = value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    encode_base64(&bytes)
}

pub(super) fn decode_utf16le_base64(value: &str) -> Result<String, String> {
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

pub(super) fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;
        output.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() >= 2 {
            output.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() == 3 {
            output.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

pub(super) fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
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

pub(super) fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
