//! Reading the tables: the templates of a language, cached, and the lookup of a text the
//! game built. The rules of the templates are in `domain/tr_fill.rs`.

use super::super::domain::tr_fill::*;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub fn templates(lang: &str) -> Arc<Vec<Template>> {
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
        .filter_map(|(k, v)| Template::from_entry(k.into_owned(), v.into_owned()))
        .collect();
    let list = Arc::new(list);
    t.write().insert(lang.to_string(), list.clone());
    list
}

/// `text` in `lang` when it is a template of the tables with its values filled in. A value
/// the tables have is translated too ("Weather: Custom weather").
pub fn fill(lang: &str, text: &str) -> Option<String> {
    // (what was asked in this language, found or not: the painter asks every frame)
    static SEEN: Mutex<(String, Option<HashMap<String, Option<String>>>)> =
        Mutex::new((String::new(), None));
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
                let value = crate::_rust_i18n_try_translate(lang, value)
                    .map(|x| x.into_owned())
                    .unwrap_or_else(|| value.to_string());
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
