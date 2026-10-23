use crate::map_surface::{GROUP_BRT, GROUP_EXPRESS, GROUP_FEEDER};
use omsi_ui::Color;

/// The texts of the lines panel and of the "you are on" chip. A language without its own gets English.
pub(super) struct LinesText {
    pub(super) lines: &'static str,
    pub(super) line_data: &'static str,
    pub(super) map_lines: &'static str,
    pub(super) all: &'static str,
    pub(super) brt: &'static str,
    pub(super) express: &'static str,
    pub(super) feeders: &'static str,
    pub(super) length: &'static str,
    pub(super) stops: &'static str,
    pub(super) trip: &'static str,
    pub(super) route: &'static str,
    pub(super) vehicles: &'static str,
    pub(super) circular: &'static str,
    pub(super) out_back: &'static str,
    pub(super) minutes: &'static str,
    pub(super) count: &'static str,
    /// The limit of heavy vehicles, when it is not the same.
    pub(super) heavy: &'static str,
    /// The compass: map, north, heading.
    pub(super) orient: [&'static str; 3],
    /// The terrain view's button, and the height in a chip.
    pub(super) terrain: &'static str,
    pub(super) elev: &'static str,
    pub(super) timetable: &'static str,
    /// Said while the timetable is being read.
    pub(super) loading: &'static str,
    /// Tick every line of the tab on show, and clear them.
    /// Said in the card while no line is open: how to open one.
    pub(super) hint: [&'static str; 2],
    /// The lists by kind of vehicle: bus, tram, metro, rail.
    pub(super) modals: [&'static str; 4],
    pub(super) tick_all: &'static str,
    pub(super) tick_none: &'static str,
    pub(super) towards: &'static str,
    /// Monday to Friday, Saturday, Sunday.
    pub(super) days: [&'static str; 3],
    pub(super) predicted: &'static str,
    /// The map's header: what it is, the next stop, the button that follows the bus.
    pub(super) city_map: &'static str,
    pub(super) next_stop: &'static str,
    pub(super) follow: &'static str,
    /// The card of where the bus is, and its rows.
    pub(super) you_are_here: &'static str,
    pub(super) city: &'static str,
    pub(super) street: &'static str,
    /// The trip's card, and its rows (the next stop's above).
    pub(super) trip_card: &'static str,
    pub(super) distance: &'static str,
    pub(super) arrival: &'static str,
    pub(super) stops_left: &'static str,
    /// The legend: the route ahead, the part driven, the traffic, the bus; and the keys.
    pub(super) driven: &'static str,
    pub(super) traffic: &'static str,
    pub(super) you: &'static str,
    pub(super) keys: &'static str,
}

/// The texts as English keys: the painter looks them up in the catalog when it draws or measures them. A text
/// that is put together with others goes through `omsi_ui::tr` first (a sentence made of English pieces is not in
/// the catalog).
pub(super) fn lines_text() -> LinesText {
    LinesText {
        lines: "Lines",
        line_data: "LINE DATA",
        map_lines: "MAP LINES",
        all: "All",
        brt: "BRT",
        express: "Express",
        feeders: "Feeders",
        length: "Length",
        stops: "Stops",
        trip: "Trip time",
        route: "Route",
        vehicles: "Vehicles now",
        circular: "Circular",
        out_back: "Out and back",
        minutes: "min",
        count: "lines",
        timetable: "Timetable",
        loading: "Loading timetable…",
        hint: ["Double-click a line", "for its routes and timetable"],
        modals: ["Bus", "Tram", "Metro", "Rail"],
        tick_all: "Select all",
        tick_none: "Clear",
        towards: "Towards",
        days: ["Mon–Fri", "Sat", "Sun"],
        predicted: "Times predicted from the map's timetable",
        heavy: "heavy",
        terrain: "Terrain",
        orient: ["Map", "North", "Heading"],
        elev: "elev.",
        city_map: "City map",
        next_stop: "Next stop",
        follow: "Follow bus",
        you_are_here: "You are here",
        city: "City",
        street: "Street",
        trip_card: "Trip",
        distance: "Distance",
        arrival: "Arrival",
        stops_left: "Stops left",
        driven: "Driven",
        traffic: "Traffic",
        you: "You",
        keys: "Wheel: zoom · Drag: pan · Esc: close",
    }
}

/// Whether the interface language writes decimals with a comma.
pub(super) fn decimal_comma() -> bool {
    decimal_comma_in(&omsi_ui::i18n::language())
}

fn decimal_comma_in(code: &str) -> bool {
    matches!(
        code,
        "be" | "ca"
            | "cs"
            | "de"
            | "es"
            | "fr"
            | "hu"
            | "id"
            | "it"
            | "kk"
            | "nl"
            | "pl"
            | "pt"
            | "pt-pt"
            | "ru"
            | "tr"
            | "uk"
            | "vi"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "be", "ca", "cs", "de", "es", "fr", "hi", "hu", "id", "it", "ja", "kk", "ko", "ms",
            "nl", "pl", "pt", "pt-pt", "ru", "th", "tl", "tr", "uk", "vi", "zh", "zh-tw",
        ] {
            for k in &keys {
                assert!(
                    crate::_rust_i18n_try_translate(lang, k).is_some(),
                    "{lang}: {k:?}"
                );
            }
        }
    }
}

/// The colour a bus line is drawn in on the city map (a fixed palette that keeps clear of the map's own colours).
pub(super) const GROUP_BRT_TAB: u8 = GROUP_BRT;
pub(super) const GROUP_EXPRESS_TAB: u8 = GROUP_EXPRESS;
pub(super) const GROUP_FEEDER_TAB: u8 = GROUP_FEEDER;

pub(super) fn line_color(i: usize) -> Color {
    const P: [u32; 10] = [
        0xe83f9b, 0x17b8c4, 0xff8a1f, 0xff4d4d, 0x8bd13f, 0xd46bff, 0x1fd1a0, 0xff6fb5, 0xf2c14e,
        0x4da3ff,
    ];
    Color::hex(P[i % P.len()])
}
