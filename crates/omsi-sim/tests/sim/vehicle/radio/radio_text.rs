use omsi_sim::testing::vehicle::radio::{magnitola_line, own_frequency};

#[test]
fn the_station_line_is_replaced_while_the_display_shows_it() {
    assert_eq!(
        magnitola_line("90.9 MHz@R-ZURNAL", "90.9 MHz@R-ZURNAL", "Radio 1   ", None).as_deref(),
        Some("90.9 MHz@Radio 1   ")
    );
    // its welcome and the volume are the display's own
    assert_eq!(
        magnitola_line("90.9 MHz@R-ZURNAL", " WELCOME  ", "Radio 1", None),
        None
    );
    assert_eq!(
        magnitola_line("90.9 MHz@R-ZURNAL", "VOLUME@ 15", "Radio 1", None),
        None
    );
    // stopped, and a radio that plays nothing
    assert_eq!(
        magnitola_line("STOPPED@", "STOPPED@", "Radio 1", None),
        None
    );
    assert_eq!(
        magnitola_line("90.9 MHz@R-ZURNAL", "90.9 MHz@R-ZURNAL", "", None),
        None
    );
}

#[test]
fn the_stations_own_frequency_stands_for_the_scripts() {
    assert_eq!(
        magnitola_line(
            "90.9 MHz@R-ZURNAL",
            "90.9 MHz@R-ZURNAL",
            "Radio 1   ",
            Some("94.6 MHz")
        )
        .as_deref(),
        Some("94.6 MHz@Radio 1   ")
    );
    assert_eq!(
        magnitola_line("STOPPED@", "STOPPED@", "Radio 1", Some("94.6 MHz")),
        None
    );
    // the kind that keeps its frequency apart
    assert_eq!(
        own_frequency("90.9 MHz@", "90.9 MHz@Radio 1   ", "94.6 MHz").as_deref(),
        Some("94.6 MHz@Radio 1   ")
    );
    assert_eq!(
        own_frequency("90.9 MHz@", "94.6 MHz@Radio 1   ", "94.6 MHz"),
        None
    );
    assert_eq!(own_frequency("90.9 MHz@", " WELCOME  ", "94.6 MHz"), None);
    assert_eq!(own_frequency("STOPPED@", "STOPPED@", "94.6 MHz"), None);
}
