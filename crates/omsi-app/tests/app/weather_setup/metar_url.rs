use openomsi_game::testing::weather_setup::*;

#[test]
fn a_station_code_becomes_one_fixed_request() {
    assert_eq!(
        metar_url("eddb").unwrap(),
        "https://aviationweather.gov/api/data/metar?ids=EDDB&format=raw"
    );
    assert_eq!(
        metar_url(" KJFK ").unwrap(),
        "https://aviationweather.gov/api/data/metar?ids=KJFK&format=raw"
    );
    assert!(metar_url("K1A0").is_some());
}

#[test]
fn a_code_cannot_change_the_request() {
    for bad in [
        "",
        "E",
        "EDDB&format=json",
        "EDDB,KJFK",
        "../x",
        "EDDB#",
        "EDDB?x=1",
        "ED DB",
        "EDDB\r\nHost: x",
        "%45DDB",
        "EDDBEDDBEDDB",
        "ÄÖÜ1",
    ] {
        assert!(metar_url(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn percent_encoding_leaves_only_unreserved_characters() {
    assert_eq!(percent_encode("EDDB"), "EDDB");
    assert_eq!(percent_encode("a&b=c d/é"), "a%26b%3Dc%20d%2F%C3%A9");
    assert_eq!(percent_encode("-._~"), "-._~");
}
