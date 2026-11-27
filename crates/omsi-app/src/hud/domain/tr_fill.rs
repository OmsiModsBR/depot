//! The template rules of the tables: a text with `%{name}` values is cut at them, its
//! values are found in a text the game built, and a table entry is kept when it is long
//! enough to tell apart. The tables are read in `implementation/tr_fill.rs`.

/// A template cut at its values: "%{done} of %{total} tiles" is `["", " of ", " tiles"]`.
pub fn parts(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find("%{") {
        let Some(j) = rest[i..].find('}') else { break };
        out.push(rest[..i].to_string());
        rest = &rest[i + j + 1..];
    }
    out.push(rest.to_string());
    out
}

/// The values `text` puts into the template cut into `parts`, in order; None when `text` is
/// not that template.
pub fn values<'a>(parts: &[String], text: &'a str) -> Option<Vec<&'a str>> {
    let (first, last) = (parts.first()?, parts.last()?);
    if parts.len() < 2
        || text.len() < first.len() + last.len()
        || !text.starts_with(first.as_str())
        || !text.ends_with(last.as_str())
    {
        return None;
    }
    let mut mid = &text[first.len()..text.len() - last.len()];
    let mut out = Vec::with_capacity(parts.len() - 1);
    for p in &parts[1..parts.len() - 1] {
        let i = mid.find(p.as_str())?;
        out.push(&mid[..i]);
        mid = &mid[i + p.len()..];
    }
    out.push(mid);
    Some(out)
}

/// The placeholders' names, in order: "%{done} of %{total}" has `done`, `total`.
pub fn names(template: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find("%{") {
        let Some(j) = rest[i..].find('}') else { break };
        out.push(&rest[i + 2..i + j]);
        rest = &rest[i + j + 1..];
    }
    out
}

/// Placeholder names the tables use for counts and amounts.
const COUNTS: [&str; 8] = ["n", "v", "count", "total", "done", "maps", "speed", "num"];

pub struct Template {
    pub parts: Vec<String>,
    /// Words on one side only and few of them, and a value named as a count ("%{n} vehicles"):
    /// that value must have digits, or "Clear all placed vehicles" would be taken for it. A name
    /// ("Stop skipped: %{name}") takes any value.
    pub(crate) number: bool,
    pub(crate) names: Vec<String>,
    /// The translation, with the same placeholders.
    pub(crate) text: String,
    /// The template's own letters: of two that fit, the one with more is the text's.
    pub(crate) weight: usize,
}

impl Template {
    /// The template of one table entry: `key` is the English text with its values, `text`
    /// its translation. None when the entry is too short to be told apart.
    pub(crate) fn from_entry(key: String, text: String) -> Option<Template> {
        let parts = parts(&key);
        // (a short one, "Gear %{n}", is filled in by its caller: with so few words it
        // would also take texts that are not it)
        let letters = parts
            .iter()
            .flat_map(|s| s.chars())
            .filter(|c| c.is_alphabetic())
            .count();
        let one_side = parts.len() == 2 && (parts[0].is_empty() || parts[1].is_empty());
        let names: Vec<String> = names(&key).into_iter().map(str::to_string).collect();
        (letters >= 8).then(|| Template {
            number: one_side && letters < 20 && names.iter().all(|n| COUNTS.contains(&n.as_str())),
            weight: parts.iter().map(|s| s.chars().count()).sum(),
            names,
            parts,
            text,
        })
    }
}
