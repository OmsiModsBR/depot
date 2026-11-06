use omsi_content::font::FontAtlas;
use omsi_texture::Image;
use openomsi_game::testing::schedule::{PlannedStop, PlannedTrip, StopDir};
use openomsi_game::testing::schedule_paper::*;

const FONT_HEIGHT: u32 = 24;
use omsi_content::font::FontChar;

fn stop(id: i64, name: &str, arr: f64, dep: f64) -> PlannedStop {
    PlannedStop {
        object_id: id,
        name: name.into(),
        arr,
        dep,
        position: None,
        dir: StopDir::default(),
        stops: true,
    }
}

fn test_font() -> FontAtlas {
    let chars: Vec<_> = (32u8..=126)
        .enumerate()
        .map(|(index, byte)| FontChar {
            ch: byte as char,
            x0: index as i32 * 4,
            x1: (index as i32 + 1) * 4,
            y: 0,
        })
        .collect();
    let width = chars.len() as u32 * 4;
    let pixels = vec![255; (width * FONT_HEIGHT * 4) as usize];
    FontAtlas::new(
        omsi_content::font::Font {
            height: FONT_HEIGHT as i32,
            gap: 1,
            chars,
            ..Default::default()
        },
        width,
        FONT_HEIGHT,
        pixels.clone(),
        pixels,
    )
}

#[test]
fn a_line_is_as_high_as_gdi_makes_it() {
    // Courier New bold at 16 pt (an em of 21 px): ascent 1705, descent 615 of 2048
    let k = 21.0 / 2048.0;
    assert_eq!(gdi_line_height(1705.0 * k, -615.0 * k), 23);
}

#[test]
fn paper_rows_end_with_arrival_and_departure_of_the_last_stop() {
    let t = |h: f64, m: f64| h * 3600.0 + m * 60.0;
    let mut passed = stop(2, "Feld", t(11.0, 54.0), t(11.0, 54.0));
    passed.stops = false;
    let current = PlannedTrip {
        name: "76_Kk-BH".into(),
        line: "76".into(),
        terminus: "Bauernhof".into(),
        departure: t(11.0, 52.0),
        end: t(11.0, 59.0),
        stops: vec![
            stop(1, "Krankenhaus", t(11.0, 50.0), t(11.0, 52.0)),
            passed,
            stop(3, "Dorf", t(11.0, 55.0), t(11.0, 57.0)),
            stop(4, "Bauernhof", t(11.0, 59.0), t(12.0, 1.0)),
        ],
    };
    let (title, rows) = paper_content("76", "1", &[current], 0, ("Arrival ", "Depart. "));
    assert_eq!(title, "76 - Bauernhof - 1");
    assert_eq!(
        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Krankenhaus  ", "Dorf  ", "Bauernhof  Arrival ", "Depart. "]
    );
    assert_eq!(
        rows.iter().map(|r| r.time.as_str()).collect::<Vec<_>>(),
        ["11:52", "11:57", "11:59", "12:01"]
    );
}

#[test]
fn paper_times_wrap_after_midnight() {
    assert_eq!(format_time(24.0 * 3600.0 + 7.0 * 60.0), "00:07");
}

#[test]
fn paper_text_is_two_columns_of_25_character_labels() {
    let rows: Vec<_> = (0..50)
        .map(|i| PaperRow {
            name: if i == 0 {
                "Gustav-Adolf-Str./Langhansstr.  ".into()
            } else {
                format!("Stop {i:02}  ")
            },
            time: "12:34".into(),
        })
        .collect();
    let lines = paper_lines("76 - Bauernhof - 1", &rows);
    assert_eq!(lines.len(), 2 + 24);
    assert_eq!(lines[0], "76 - Bauernhof - 1");
    assert_eq!(lines[1], "=".repeat(67));
    assert_eq!(
        lines[2],
        "Gustav-Adolf-Str./Langhan 12:34     Stop 24  ................ 12:34"
    );
    assert_eq!(
        lines[25],
        "Stop 23  ................ 12:34     Stop 47  ................ 12:34"
    );
    // rows past the second column are left out
    assert!(!lines.iter().any(|l| l.contains("Stop 48")));

    let short = paper_lines("t", &rows[1..3]);
    assert_eq!(short[2], "Stop 01  ................ 12:34");
    assert_eq!(short.len(), 4);
}

#[test]
fn paper_lines_are_drawn_from_60_90_one_font_height_apart() {
    let font = test_font();
    let mut image = Image {
        width: 1024,
        height: 1024,
        rgba: vec![255; 1024 * 1024 * 4],
        has_alpha: false,
    };
    draw_schedule(&mut image, &font, &["AB".to_string(), "CD".to_string()]);
    let ink = |x: u32, y: u32| image.rgba[((y * image.width + x) * 4) as usize] < 100;
    assert!(ink(PAPER_X, PAPER_TOP));
    assert!(ink(PAPER_X, PAPER_TOP + FONT_HEIGHT));
    assert!(!ink(PAPER_X - 1, PAPER_TOP));
    assert!(!ink(PAPER_X, PAPER_TOP - 1));
}
