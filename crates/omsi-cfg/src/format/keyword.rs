//! Which lines are keyword lines, by the rule of each loader of the original.

use super::keywords;

/// How a loader of the original tells a keyword line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeywordRule {
    /// The line is exactly `[name]` with the original's spelling (almost every loader).
    #[default]
    Exact,
    /// Trailing tabs, spaces, line breaks and double quotes are cut off first (`.hof`, whose
    /// stock files are spreadsheet exports: `[infosystem_trip]` followed by twelve tabs).
    TrimEnd,
    /// The line is lower-cased first (`ailists.cfg`).
    AnyCase,
}

/// What `.hof` cuts off the end of every line.
pub(super) fn hof_trailing(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | ' ' | '"')
}

/// The original's spelling of a keyword it knows (by the name in any case).
fn omsi_spelling(name: &str) -> Option<&'static str> {
    let lower;
    let key = if name.bytes().any(|b| b.is_ascii_uppercase()) {
        lower = name.to_ascii_lowercase();
        lower.as_str()
    } else {
        name
    };
    if let Ok(i) = keywords::MIXED_CASE.binary_search_by(|k| cmp_lower(k, key)) {
        return Some(keywords::MIXED_CASE[i]);
    }
    keywords::LOWER_CASE
        .binary_search(&key)
        .ok()
        .map(|i| keywords::LOWER_CASE[i])
}

/// `a.to_ascii_lowercase().cmp(b)` without allocating.
fn cmp_lower(a: &str, b: &str) -> std::cmp::Ordering {
    a.bytes().map(|c| c.to_ascii_lowercase()).cmp(b.bytes())
}

/// Test whether a line is a keyword line and return the keyword (without brackets, as
/// written): the whole line is `[name]`, and `name` is spelled like the original spells it
/// when the original knows it.
pub fn keyword_of(line: &str) -> Option<&str> {
    keyword_with(line, KeywordRule::Exact)
}

/// [`keyword_of`] for a loader with its own rule.
pub fn keyword_with(line: &str, rule: KeywordRule) -> Option<&str> {
    let t = if rule == KeywordRule::TrimEnd {
        line.trim_end_matches(hof_trailing)
    } else {
        line
    };
    if t.len() < 2 || t.as_bytes()[0] != b'[' || !t.ends_with(']') {
        return None;
    }
    let inner = &t[1..t.len() - 1];
    // Keywords never contain a second bracket; free text like "[mesh] macht ..." is never a
    // keyword because it does not end with ']'.
    if inner.is_empty() || inner.contains('[') || inner.contains(']') {
        return None;
    }
    if rule != KeywordRule::AnyCase {
        if let Some(spelling) = omsi_spelling(inner) {
            if spelling != inner {
                return None;
            }
        }
    }
    Some(inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords() {
        assert_eq!(keyword_of("[mesh]"), Some("mesh"));
        assert_eq!(keyword_of("\t[mesh]"), None);
        assert_eq!(keyword_of("        [NightMapMode]"), None);
        assert_eq!(keyword_of("[mesh] "), None);
        assert_eq!(keyword_of("[busstop] macht aus dem Objekt"), None);
        assert_eq!(keyword_of("[coupling_front] / [coupling_back]"), None);
        // the original's spelling counts for its own keywords …
        assert_eq!(keyword_of("[matl_noZwrite]"), Some("matl_noZwrite"));
        assert_eq!(keyword_of("[matl_nozWrite]"), None);
        assert_eq!(keyword_of("[NoDistanceCheck]"), None);
        assert_eq!(keyword_of("[Mesh]"), None);
        assert_eq!(keyword_of("[LOD]"), Some("LOD"));
        assert_eq!(keyword_of("[lod]"), None);
        // … not for the rest
        assert_eq!(
            keyword_of("[Infosystem_Busstop]"),
            Some("Infosystem_Busstop")
        );
        // .hof: spreadsheet exports
        assert_eq!(
            keyword_with("[global_strings]\t\t\t", KeywordRule::TrimEnd),
            Some("global_strings")
        );
        assert_eq!(keyword_with("[end]\"\t", KeywordRule::TrimEnd), Some("end"));
        assert_eq!(keyword_with("\t[end]", KeywordRule::TrimEnd), None);
        assert_eq!(keyword_of("[global_strings]\t"), None);
        // ailists.cfg
        assert_eq!(
            keyword_with("[AIGroup_2]", KeywordRule::AnyCase),
            Some("AIGroup_2")
        );
        assert_eq!(keyword_with(" [aigroup_2]", KeywordRule::AnyCase), None);
        for k in keywords::MIXED_CASE.iter().chain(keywords::LOWER_CASE) {
            assert_eq!(omsi_spelling(&k.to_ascii_uppercase()), Some(*k));
        }
    }
}
