//! The `key=value` text with `[section]` headers both convention files use.

/// One section: its name as written (without the brackets) and its `(key, value)` pairs,
/// keys in lower case, both trimmed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Section {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// Read the sections of `text`. Lines starting with `;` or `#` are comments, and so is
/// the rest of a line after ` ;` (a value never needs a semicolon after a space). Pairs
/// before the first header go into a section with an empty name. A line that is neither
/// a header nor a pair is skipped.
pub(crate) fn parse(text: &str) -> Vec<Section> {
    let mut out = vec![Section {
        name: String::new(),
        entries: Vec::new(),
    }];
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push(Section {
                name: name.trim().to_string(),
                entries: Vec::new(),
            });
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if let Some(s) = out.last_mut() {
                s.entries
                    .push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
            }
        }
    }
    out.retain(|s| !(s.name.is_empty() && s.entries.is_empty()));
    out
}

fn strip_comment(line: &str) -> &str {
    match line.find(" ;").or_else(|| line.find("\t;")) {
        Some(i) => &line[..i],
        None => line,
    }
}
