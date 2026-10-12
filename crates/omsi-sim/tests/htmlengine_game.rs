//! The HTML engine with the rest of the game: the vehicle snapshot, the clock's date and time,
//! and pages loaded from disk with their style sheets and pictures.

use omsi_sim::htmlengine::{EngineRenderer, HtmlRenderer};

const DEMO: &str = include_str!("../../../docs/examples/htmltexture/demo.html");

fn px(frame: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * w + x) * 4) as usize;
    [frame[i], frame[i + 1], frame[i + 2], frame[i + 3]]
}

fn has_colour(frame: &[u8], rgb: [u8; 3]) -> bool {
    frame.chunks(4).any(|p| p[0] == rgb[0] && p[1] == rgb[1] && p[2] == rgb[2])
}

/// `.warn` (open door, brake held) and `.on` (engine lamp, open boarding door).
const RED: [u8; 3] = [0xe5, 0x38, 0x3b];
const GREEN: [u8; 3] = [0x1f, 0xa6, 0x4f];

fn bmp(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let row = ((w * 3 + 3) / 4 * 4) as usize;
    let data = row * h as usize;
    let mut b = Vec::with_capacity(54 + data);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&((54 + data) as u32).to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(w as i32).to_le_bytes());
    b.extend_from_slice(&(h as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&(data as u32).to_le_bytes());
    b.extend_from_slice(&2835u32.to_le_bytes());
    b.extend_from_slice(&2835u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    for _ in 0..h {
        for _ in 0..w {
            b.extend_from_slice(&[rgb[2], rgb[1], rgb[0]]);
        }
        b.resize(b.len() + row - (w * 3) as usize, 0);
    }
    b
}

#[test]
fn the_demo_shows_a_real_vehicle_snapshot() {
    use omsi_sim::vehicle_api::{snapshot, Inputs};
    let var = |n: &str| match n.to_ascii_lowercase().as_str() {
        "door_0" => Some(0.0),
        "door_1" => Some(1.0),
        "engine_n" => Some(800.0),
        _ => None,
    };
    let text = |n: &str| match n {
        "number" => "4711".to_string(),
        "act_route" => "Line 7".to_string(),
        "act_busstop" => "Hauptbahnhof".to_string(),
        _ => String::new(),
    };
    let api = snapshot(&Inputs {
        var: &var,
        text: &text,
        speed_kmh: 36.6,
        steer_deg: 0.0,
        heading: 0.0,
        pitch: 0.0,
        bank: 0.0,
        position: (0.0, 0.0, 0.0),
        engine_running: true,
        interior_light: 0.0,
        crashes: 0,
        last_impact_j: 0.0,
        dirt: 0.0,
        trailers: 0,
    });
    let mut r = EngineRenderer::new(800, 480, DEMO);
    r.set_vehicle(&api);
    r.set_vars(&[], &[]);
    assert_eq!(r.text_of("kmh").as_deref(), Some("37"));
    assert_eq!(r.text_of("num").as_deref(), Some("Bus 4711"));
    assert_eq!(r.text_of("route").as_deref(), Some("Line 7"));
    assert_eq!(r.text_of("stop").as_deref(), Some("Hauptbahnhof"));
    let f = r.poll_frame().unwrap();
    assert!(has_colour(&f, RED), "the open door is drawn red");
    assert!(has_colour(&f, GREEN), "the running engine is drawn green");
}

#[test]
fn time_date_and_locale_reach_the_page() {
    use omsi_sim::vehicle_api::environment;
    let mut c = omsi_sim::SimClock::default();
    c.set_date(2026, 9, 30);
    c.time = 13.0 * 3600.0 + 5.0 * 60.0 + 9.0;
    let mut r = EngineRenderer::new(
        200,
        100,
        "<div id='a'></div><div id='b'></div><div id='c'></div><div id='d'></div>\
         <script>\
          omsi.update = function () {\
            var t = omsi.time, d = omsi.date;\
            document.getElementById('a').textContent = t.hour + ':' + t.minute + ':' + t.second + ' ' + t.asString;\
            document.getElementById('b').textContent = d.day + '.' + d.month + '.' + d.year + ' ' + d.asString;\
            document.getElementById('c').textContent = omsi.locale;\
          };\
         </script>",
    );
    r.set_env(&environment(&c, "de"));
    r.set_vars(&[], &[]);
    assert_eq!(r.text_of("a").as_deref(), Some("13:5:9 13:05:09"));
    assert_eq!(r.text_of("b").as_deref(), Some("30.9.2026 30.09.2026"));
    assert_eq!(r.text_of("c").as_deref(), Some("de"));
}

#[test]
fn a_scenery_page_has_only_the_basic_api() {
    let mut r = EngineRenderer::with_api(
        200,
        100,
        "<div id='a'></div><div id='b'></div>         <script>          document.getElementById('a').textContent = [typeof omsi.vehicle, typeof omsi.depot, typeof omsi.setRoute, typeof omsi.setNextStop].join(',');          omsi.update = function (d) {            document.getElementById('b').textContent = [typeof d.vehicle, typeof omsi.setVar, typeof omsi.trigger, typeof omsi.time, omsi.locale].join(',');          };         </script>",
        omsi_sim::htmltex::PageApi::Scenery,
    );
    r.set_env(&omsi_sim::vehicle_api::environment(&omsi_sim::SimClock::default(), "de"));
    r.set_vars(&[], &[]);
    assert_eq!(r.text_of("a").as_deref(), Some("undefined,undefined,undefined,undefined"));
    assert_eq!(r.text_of("b").as_deref(), Some("undefined,function,function,object,de"));
}

#[test]
fn the_english_date_is_month_first() {
    use omsi_sim::vehicle_api::{environment, ApiValue};
    let mut c = omsi_sim::SimClock::default();
    c.set_date(2026, 9, 30);
    let ApiValue::Map(m) = environment(&c, "en") else { panic!() };
    let ApiValue::Map(d) = &m.iter().find(|(k, _)| k == "date").unwrap().1 else { panic!() };
    assert_eq!(d.iter().find(|(k, _)| k == "asString").unwrap().1, ApiValue::Str("09/30/2026".into()));
}

#[test]
fn linked_style_sheet_and_script_are_inlined_with_their_pictures() {
    let dir = std::env::temp_dir().join(format!("omsi_htmlimg_page_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("css")).unwrap();
    std::fs::create_dir_all(dir.join("img")).unwrap();
    std::fs::write(dir.join("img").join("red.bmp"), bmp(4, 4, [255, 0, 0])).unwrap();
    std::fs::write(dir.join("css").join("s.css"), "#a{width:6px;height:6px;background:url(../img/red.bmp)}").unwrap();
    std::fs::write(dir.join("index.html"), "<link rel='stylesheet' href='css/s.css'><body style='margin:0'><div id=a></div></body>").unwrap();
    let dirs = [dir.as_path()];
    let html = omsi_sim::htmltex::load_page(&dirs, "index.html");
    assert!(html.contains("url(css/../img/red.bmp)"));
    let mut r = EngineRenderer::new(8, 8, &html);
    r.set_asset_dirs(omsi_sim::htmltex::asset_dirs(&dirs, "index.html"));
    let f = r.poll_frame().unwrap();
    assert_eq!(px(&f, 8, 5, 5), [255, 0, 0, 255]);
}
