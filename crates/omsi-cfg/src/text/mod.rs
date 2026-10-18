//! Text as OMSI writes it: the code page of a file, its lines and the numbers in them.

pub mod codepage;
pub mod number;

/// Decode raw file bytes with OMSI's encoding rules.
pub fn decode_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let (s, _) = encoding_rs::UTF_16LE.decode_without_bom_handling(&bytes[2..]);
        s.into_owned()
    } else if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let (s, _) = encoding_rs::UTF_16BE.decode_without_bom_handling(&bytes[2..]);
        s.into_owned()
    } else if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else {
        codepage::decode(bytes)
    }
}

/// Encode a string the way OMSI writes it (Windows-1252, CR LF).
pub fn encode_text(s: &str) -> Vec<u8> {
    let (b, _, _) = encoding_rs::WINDOWS_1252.encode(s);
    b.into_owned()
}

/// Encode a string as OMSI's editor saves it: UTF-16 little endian, with its byte order mark.
pub fn encode_utf16_bom(s: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 + 2 * s.len());
    bytes.extend_from_slice(&[0xFF, 0xFE]);
    for u in s.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    bytes
}

/// Split into lines. Handles CR LF, LF and bare CR.
pub fn split_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            '\n' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_with_its_mark_reads_back() {
        let bytes = encode_utf16_bom("[name]\r\nGrünau");
        assert_eq!(&bytes[..4], &[0xFF, 0xFE, b'[', 0]);
        assert_eq!(decode_text(&bytes), "[name]\r\nGrünau");
        let big_endian: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("Grünau".encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(decode_text(&big_endian), "Grünau");
    }
}
