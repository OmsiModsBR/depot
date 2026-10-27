use openomsi_game::testing::tr_fill::*;

#[test]
fn a_text_is_cut_at_its_values() {
    let p = parts("Loading map tiles (first area) \u{b7} %{done} of %{total}");
    assert_eq!(p, ["Loading map tiles (first area) \u{b7} ", " of ", ""]);
    assert_eq!(
        values(&p, "Loading map tiles (first area) \u{b7} 62 of 100"),
        Some(vec!["62", "100"])
    );
    assert_eq!(values(&p, "Loading map tiles (first area) \u{b7} 62"), None);
    assert_eq!(values(&parts("Gear %{n}"), "Gear 3"), Some(vec!["3"]));
    assert_eq!(names("%{done} of %{total}"), ["done", "total"]);
    assert!(telling(&parts("Gear %{n}")));
    assert!(!telling(&parts("%{a}: %{b}")));
    assert!(!telling(&parts("no values here")));
}

#[test]
fn a_formatted_text_is_found_through_its_template() {
    assert_eq!(
        fill(
            "de",
            "Kartenkacheln werden geladen (erster Bereich) \u{b7} 62 von 100"
        ),
        None
    );
    assert_eq!(
        fill("de", "Loading map tiles (first area) \u{b7} 62 of 100").as_deref(),
        Some("Kartenkacheln werden geladen (erster Bereich) \u{b7} 62 von 100")
    );
    assert_eq!(fill("de", "Back to my bus"), None);
    // (too few words to tell it apart: its caller fills it in)
    assert!(templates("de")
        .iter()
        .all(|t| t.parts != parts("Line %{line}")));
    assert_eq!(
        fill("de", "Loading map tiles (first area) \u{b7}  of 100"),
        None
    );
    // (a name without digits is still a value; a count without them is not)
    assert_eq!(
        fill("de", "Stop skipped: Hauptbahnhof").as_deref(),
        Some("Haltestelle ausgelassen: Hauptbahnhof")
    );
    assert_eq!(fill("de", "Clear all placed vehicles"), None);
}
