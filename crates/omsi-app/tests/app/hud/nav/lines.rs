use openomsi_game::testing;
use openomsi_game::testing::hud::nav::lines::*;

#[test]
fn decimals_follow_the_language() {
    for c in [
        "es", "fr", "de", "pt", "pt-pt", "ru", "pl", "it", "tr", "hu", "id", "vi",
    ] {
        assert!(decimal_comma_in(c), "{c}");
    }
    for c in ["", "en", "ja", "ko", "zh", "zh-tw", "th", "hi", "ms", "tl"] {
        assert!(!decimal_comma_in(c), "{c}");
    }
}

#[test]
fn every_text_of_the_panel_is_in_the_catalog_for_every_language() {
    let t = lines_text();
    let mut keys = vec![
        t.lines,
        t.line_data,
        t.map_lines,
        t.all,
        t.express,
        t.feeders,
        t.length,
        t.stops,
        t.trip,
        t.route,
        t.vehicles,
        t.circular,
        t.out_back,
        t.minutes,
        t.count,
        t.timetable,
        t.loading,
        t.tick_all,
        t.tick_none,
        t.towards,
        t.predicted,
        t.heavy,
        t.terrain,
        t.elev,
        t.next_stop,
        t.follow,
        t.you_are_here,
        t.city,
        t.street,
        t.distance,
        t.arrival,
        t.stops_left,
        t.driven,
        t.you,
        t.keys,
    ];
    keys.extend(t.hint);
    keys.extend(t.modals);
    keys.extend(t.days);
    keys.extend(t.orient);
    for lang in [
        "be", "ca", "cs", "de", "es", "fr", "hi", "hu", "id", "it", "ja", "kk", "ko", "ms", "nl",
        "pl", "pt", "pt-pt", "ru", "th", "tl", "tr", "uk", "vi", "zh", "zh-tw",
    ] {
        for k in &keys {
            assert!(testing::translate(lang, k).is_some(), "{lang}: {k:?}");
        }
    }
}
