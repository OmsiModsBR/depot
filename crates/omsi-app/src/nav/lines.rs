use omsi_ui::Color;
use crate::map_surface::{GROUP_BRT, GROUP_EXPRESS, GROUP_FEEDER};

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
    pub(super) you_are_on: &'static str,
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
    /// Decimals with a comma.
    pub(super) comma: bool,
}

pub(super) fn lines_text(lang: &str) -> LinesText {
    match lang.to_ascii_uppercase().as_str() {
"PTB" | "PT" | "POR" | "BRA" => LinesText { lines: "Linhas", line_data: "DADOS DA LINHA", map_lines: "LINHAS DO MAPA", all: "Todas", brt: "BRT", express: "Expressas", feeders: "Alimentadoras", length: "Extensão", stops: "Paradas", trip: "Tempo de percurso", route: "Rota", vehicles: "Veículos agora", you_are_on: "Você está em", circular: "Circular", out_back: "Ida e volta", minutes: "min", count: "linhas", timetable: "Horários", hint: ["Duplo clique numa linha", "para ver rotas, sentidos e horários"], modals: ["Ônibus", "VLT", "Metrô", "Trens"], tick_all: "Marcar todas", tick_none: "Limpar", towards: "Sentido", days: ["Seg–Sex", "Sáb", "Dom"], predicted: "Horários previstos pela tabela do mapa", heavy: "pesados", terrain: "Relevo", orient: ["Mapa", "Norte", "Rumo"], elev: "alt.", comma: true },
        _ => LinesText { lines: "Lines", line_data: "LINE DATA", map_lines: "MAP LINES", all: "All", brt: "BRT", express: "Express", feeders: "Feeders", length: "Length", stops: "Stops", trip: "Trip time", route: "Route", vehicles: "Vehicles now", you_are_on: "You are on", circular: "Circular", out_back: "Out and back", minutes: "min", count: "lines", timetable: "Timetable", hint: ["Double-click a line", "for its routes and timetable"], modals: ["Bus", "Tram", "Metro", "Rail"], tick_all: "Select all", tick_none: "Clear", towards: "Towards", days: ["Mon–Fri", "Sat", "Sun"], predicted: "Times predicted from the map's timetable", heavy: "heavy", terrain: "Terrain", orient: ["Map", "North", "Heading"], elev: "elev.", comma: false },
    }
}

/// The colour a bus line is drawn in on the city map (a fixed palette that keeps clear of the map's own colours).
pub(super) const GROUP_BRT_TAB: u8 = GROUP_BRT;
pub(super) const GROUP_EXPRESS_TAB: u8 = GROUP_EXPRESS;
pub(super) const GROUP_FEEDER_TAB: u8 = GROUP_FEEDER;

pub(super) fn line_color(i: usize) -> Color {
    const P: [u32; 10] = [0xe83f9b, 0x17b8c4, 0xff8a1f, 0xff4d4d, 0x8bd13f, 0xd46bff, 0x1fd1a0, 0xff6fb5, 0xf2c14e, 0x4da3ff];
    Color::hex(P[i % P.len()])
}
