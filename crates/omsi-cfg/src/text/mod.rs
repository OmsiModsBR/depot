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

/// How a text file was written, so that a changed copy of it can be written the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    /// UTF-16 little endian with its byte order mark, as OMSI's editor saves.
    Utf16Le,
    /// UTF-16 big endian with its byte order mark.
    Utf16Be,
    /// UTF-8, with or without its byte order mark (plain ASCII counts as UTF-8 without).
    Utf8 { bom: bool },
    /// The code page the file was written in.
    CodePage(codepage::CodePage),
    /// Bytes no code page reads back unchanged, kept one character per byte.
    Latin1,
}

/// [`decode_text`] that also says how the file was written, for a file that is written
/// back: [`encode_like`] with that encoding gives the same bytes for the same text.
pub fn decode_with_encoding(bytes: &[u8]) -> (String, TextEncoding) {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (s, _) = encoding_rs::UTF_16LE.decode_without_bom_handling(&bytes[2..]);
        return (s.into_owned(), TextEncoding::Utf16Le);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (s, _) = encoding_rs::UTF_16BE.decode_without_bom_handling(&bytes[2..]);
        return (s.into_owned(), TextEncoding::Utf16Be);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(s) = std::str::from_utf8(rest) {
            return (s.to_string(), TextEncoding::Utf8 { bom: true });
        }
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), TextEncoding::Utf8 { bom: false });
    }
    let page = codepage::detect(bytes);
    if let Some(s) = page
        .encoding()
        .decode_without_bom_handling_and_without_replacement(bytes)
    {
        let enc = TextEncoding::CodePage(page);
        if encode_like(&s, enc) == bytes {
            return (s.into_owned(), enc);
        }
    }
    (
        bytes.iter().map(|&b| b as char).collect(),
        TextEncoding::Latin1,
    )
}

/// `text` written the way [`decode_with_encoding`] found its file. A character the code
/// page has no letter for is written as a numeric character reference (`&#1078;`).
pub fn encode_like(text: &str, enc: TextEncoding) -> Vec<u8> {
    match enc {
        TextEncoding::Utf16Le => encode_utf16_bom(text),
        TextEncoding::Utf16Be => [0xFE, 0xFF]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
            .collect(),
        TextEncoding::Utf8 { bom } => {
            let mark: &[u8] = if bom { &[0xEF, 0xBB, 0xBF] } else { &[] };
            [mark, text.as_bytes()].concat()
        }
        TextEncoding::CodePage(page) => page.encoding().encode(text).0.into_owned(),
        TextEncoding::Latin1 => text.chars().map(|c| c as u32 as u8).collect(),
    }
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

    #[test]
    fn a_text_is_written_back_the_way_it_was_read() {
        let samples: Vec<Vec<u8>> = vec![
            encode_utf16_bom("[object]\r\nGrünau.sco\r\n"),
            [0xFE, 0xFF, 0, b'a', 0x00, 0xFC].to_vec(),
            b"\xEF\xBB\xBF[object]\r\nGr\xC3\xBCnau.sco\r\n".to_vec(),
            b"[object]\r\nplain.sco\r\n".to_vec(),
            // Russian and Polish authors' names, and 1252 with its quotation marks
            b"[object]\r\n\xCA\xF0\xE0\xF1\xED\xFB\xE9.sco\r\n".to_vec(),
            b"[object]\r\n\xA3\xF3d\x9F \xA3\xF3d\x9F.sco\r\n".to_vec(),
            b"[object]\r\n\x84Gr\xFCn\x93.sco\r\n".to_vec(),
        ];
        for bytes in samples {
            let (text, enc) = decode_with_encoding(&bytes);
            assert_eq!(
                text,
                decode_text(&bytes).trim_start_matches('\u{feff}'),
                "{enc:?}"
            );
            assert_eq!(encode_like(&text, enc), bytes, "{enc:?}");
        }
    }

    #[test]
    fn a_new_name_is_written_in_the_code_page_of_the_file() {
        let bytes = b"[object]\r\n\xCA\xF0\xE0\xF1\xED\xFB\xE9.sco\r\n";
        let (text, enc) = decode_with_encoding(bytes);
        assert_eq!(enc, TextEncoding::CodePage(codepage::CodePage::Windows1251));
        let more = format!("{text}[object]\r\nЖёлтый.sco\r\n");
        let written = encode_like(&more, enc);
        assert_eq!(decode_with_encoding(&written).0, more);
    }
}
