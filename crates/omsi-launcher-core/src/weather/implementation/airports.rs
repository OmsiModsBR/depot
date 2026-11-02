//! The airports a METAR weather can be fetched for.

/// The airports OMSI's METAR list (`Weather/ICAO.txt`) offers: (ICAO, "ICAO - name").
pub fn metar_airports(root: &std::path::Path) -> Vec<(String, String)> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Vec<(String, String)>>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(v) = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(root)
        .cloned()
    {
        return v;
    }
    let text = std::fs::read(omsi_cfg::resolve_path(root, "Weather/ICAO.txt"))
        .map(|b| omsi_cfg::decode_text(&b))
        .unwrap_or_default();
    let mut v: Vec<(String, String)> = text
        .lines()
        .filter_map(|l| {
            l.split_once(" - ").map(|(c, n)| {
                (
                    c.trim().to_ascii_uppercase(),
                    format!("{} - {}", c.trim(), n.trim()),
                )
            })
        })
        .filter(|(c, _)| c.len() == 4 && c.chars().all(|x| x.is_ascii_alphabetic()))
        .collect();
    if !v.iter().any(|a| a.0 == "EDDB") {
        v.push(("EDDB".into(), "EDDB - Berlin Brandenburg".into()))
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v.dedup_by(|a, b| a.0 == b.0);
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(root.to_path_buf(), v.clone());
    v
}
