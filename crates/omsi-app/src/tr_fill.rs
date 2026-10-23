//! Texts built with `format!` (`"Stop skipped: {name}"`) whose template the tables have
//! (`"Stop skipped: %{name}"`): the lookup finds the template, takes its translation and
//! puts the text's values back in. The code and the log keep their English; the screen
//! shows the language. The machine translation (`mt`) uses the same cut to leave a text
//! that is already a filled-in translation as it is.

use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// A template cut at its values: "%{done} of %{total} tiles" is `["", " of ", " tiles"]`.
pub(crate) fn parts(template: &str) -> Vec<String> {
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
pub(crate) fn values<'a>(parts: &[String], text: &'a str) -> Option<Vec<&'a str>> {
    let (first, last) = (parts.first()?, parts.last()?);
    if parts.len() < 2 || text.len() < first.len() + last.len() || !text.starts_with(first.as_str()) || !text.ends_with(last.as_str()) {
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

/// Enough words around the values to tell the template apart ("%{a}: %{b}" is not).
pub(crate) fn telling(parts: &[String]) -> bool {
    parts.len() >= 2 && parts.iter().map(|s| s.trim().chars().count()).sum::<usize>() >= 4
}

/// The placeholders' names, in order: "%{done} of %{total}" has `done`, `total`.
fn names(template: &str) -> Vec<&str> {
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

struct Template {
    parts: Vec<String>,
    /// Words on one side only and few of them, and a value named as a count ("%{n} vehicles"):
    /// that value must have digits, or "Clear all placed vehicles" would be taken for it. A name
    /// ("Stop skipped: %{name}") takes any value.
    number: bool,
    names: Vec<String>,
    /// The translation, with the same placeholders.
    text: String,
    /// The template's own letters: of two that fit, the one with more is the text's.
    weight: usize,
}

fn templates(lang: &str) -> Arc<Vec<Template>> {
    static T: OnceLock<RwLock<HashMap<String, Arc<Vec<Template>>>>> = OnceLock::new();
    let t = T.get_or_init(Default::default);
    if let Some(v) = t.read().get(lang) {
        return v.clone();
    }
    let list: Vec<Template> = crate::_RUST_I18N_BACKEND
        .messages_for_locale(lang)
        .unwrap_or_default()
        .into_iter()
        .filter(|(k, _)| k.contains("%{"))
        .filter_map(|(k, v)| {
            let parts = parts(&k);
            // (a short one, "Gear %{n}", is filled in by its caller: with so few words it
            // would also take texts that are not it)
            let letters = parts.iter().flat_map(|s| s.chars()).filter(|c| c.is_alphabetic()).count();
            let one_side = parts.len() == 2 && (parts[0].is_empty() || parts[1].is_empty());
            let names: Vec<String> = names(&k).into_iter().map(str::to_string).collect();
            (letters >= 8).then(|| Template {
                number: one_side && letters < 20 && names.iter().all(|n| COUNTS.contains(&n.as_str())),
                weight: parts.iter().map(|s| s.chars().count()).sum(),
                names,
                parts,
                text: v.into_owned(),
            })
        })
        .collect();
    let list = Arc::new(list);
    t.write().insert(lang.to_string(), list.clone());
    list
}

/// `text` in `lang` when it is a template of the tables with its values filled in. A value
/// the tables have is translated too ("Weather: Custom weather").
pub(crate) fn fill(lang: &str, text: &str) -> Option<String> {
    // (what was asked in this language, found or not: the painter asks every frame)
    static SEEN: Mutex<(String, Option<HashMap<String, Option<String>>>)> = Mutex::new((String::new(), None));
    {
        let mut s = SEEN.lock();
        if s.0 != lang {
            *s = (lang.to_string(), None);
        }
        if let Some(hit) = s.1.as_ref().and_then(|m| m.get(text)) {
            return hit.clone();
        }
    }
    let list = templates(lang);
    let found = list
        .iter()
        .filter_map(|t| {
            values(&t.parts, text)
                .filter(|v| v.iter().all(|x| !x.trim().is_empty()))
                .filter(|v| !t.number || v.iter().all(|x| x.chars().any(|c| c.is_ascii_digit())))
                .map(|v| (t, v))
        })
        .max_by_key(|(t, _)| t.weight)
        .map(|(t, v)| {
            let mut out = t.text.clone();
            for (name, value) in t.names.iter().zip(v) {
                let value = crate::_rust_i18n_try_translate(lang, value).map(|x| x.into_owned()).unwrap_or_else(|| value.to_string());
                out = out.replace(&format!("%{{{name}}}"), &value);
            }
            out
        });
    let mut s = SEEN.lock();
    let m = s.1.get_or_insert_with(HashMap::new);
    if m.len() > 4096 {
        m.clear();
    }
    m.insert(text.to_string(), found.clone());
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_is_cut_at_its_values() {
        let p = parts("Loading map tiles (first area) \u{b7} %{done} of %{total}");
        assert_eq!(p, ["Loading map tiles (first area) \u{b7} ", " of ", ""]);
        assert_eq!(values(&p, "Loading map tiles (first area) \u{b7} 62 of 100"), Some(vec!["62", "100"]));
        assert_eq!(values(&p, "Loading map tiles (first area) \u{b7} 62"), None);
        assert_eq!(values(&parts("Gear %{n}"), "Gear 3"), Some(vec!["3"]));
        assert_eq!(names("%{done} of %{total}"), ["done", "total"]);
        assert!(telling(&parts("Gear %{n}")));
        assert!(!telling(&parts("%{a}: %{b}")));
        assert!(!telling(&parts("no values here")));
    }

    #[test]
    fn a_formatted_text_is_found_through_its_template() {
        assert_eq!(fill("de", "Kartenkacheln werden geladen (erster Bereich) \u{b7} 62 von 100"), None);
        assert_eq!(
            fill("de", "Loading map tiles (first area) \u{b7} 62 of 100").as_deref(),
            Some("Kartenkacheln werden geladen (erster Bereich) \u{b7} 62 von 100")
        );
        assert_eq!(fill("de", "Back to my bus"), None);
        // (too few words to tell it apart: its caller fills it in)
        assert!(templates("de").iter().all(|t| t.parts != parts("Line %{line}")));
        assert_eq!(fill("de", "Loading map tiles (first area) \u{b7}  of 100"), None);
        // (a name without digits is still a value; a count without them is not)
        assert_eq!(fill("de", "Stop skipped: Hauptbahnhof").as_deref(), Some("Haltestelle ausgelassen: Hauptbahnhof"));
        assert_eq!(fill("de", "Clear all placed vehicles"), None);
    }
}
