use glam::{DVec2, Vec2};
use omsi_ui::{Color, Painter, Rect};
use openomsi_game::testing::map_surface::*;

/// One chunk with a triangle, a river piece and a name, written by hand in the file's layout.
fn sample() -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(1u32.to_le_bytes());
    body.push(FILL_WATER);
    body.extend(1u32.to_le_bytes());
    for v in [0.0f32, 0.0, 10.0, 0.0, 0.0, 10.0] {
        body.extend(v.to_le_bytes());
    }
    body.extend(1u32.to_le_bytes());
    body.extend([LINE_RIVER, 0]);
    body.extend(3.0f32.to_le_bytes());
    body.extend(u32::MAX.to_le_bytes());
    body.extend(2u32.to_le_bytes());
    for v in [0.0f32, 0.0, 10.0, 10.0] {
        body.extend(v.to_le_bytes());
    }
    body.extend(1u32.to_le_bytes());
    body.extend([0u8, 100]);
    body.extend(0u32.to_le_bytes());
    body.extend(5.0f32.to_le_bytes());
    body.extend(5.0f32.to_le_bytes());
    wrap(body)
}

/// A file with `body` as its one chunk.
fn wrap(body: Vec<u8>) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend(MAGIC);
    f.extend(VERSION.to_le_bytes());
    f.extend(1000.0f64.to_le_bytes());
    f.extend(2000.0f64.to_le_bytes());
    for v in [0.0f32, 100.0, 0.0, 100.0, 2000.0] {
        f.extend(v.to_le_bytes());
    }
    f.extend(1u32.to_le_bytes());
    f.extend(4u16.to_le_bytes());
    f.extend(b"Test");
    f.extend(1u32.to_le_bytes());
    let at = f.len() + 20;
    f.extend(0i32.to_le_bytes());
    f.extend(0i32.to_le_bytes());
    f.extend((at as u64).to_le_bytes());
    f.extend((body.len() as u32).to_le_bytes());
    f.extend(body);
    f
}

fn open(data: Vec<u8>) -> Option<MapSurface> {
    MapSurface::parse(
        data,
        String::new(),
        String::new(),
        Palette::dark(),
        Palette::light(),
        300.0,
    )
}

#[test]
fn a_valid_file_builds_and_names_its_places() {
    let s = open(sample()).expect("valid");
    let mut p = Painter::new();
    s.build(&mut p, DVec2::new(1000.0, 2000.0), None, false, 0.0);
    // one triangle (3 vertices) and a river ribbon
    assert!(p.verts.len() >= 3 + 6);
    let l = s.labels();
    assert_eq!(l.len(), 1);
    assert_eq!((l[0].name.as_str(), l[0].rank), ("Test", 100));
}

#[test]
fn a_chunk_out_of_range_is_left_out() {
    let s = open(sample()).unwrap();
    let mut p = Painter::new();
    s.build(
        &mut p,
        DVec2::ZERO,
        Some((DVec2::new(1.0e7, 1.0e7), 500.0)),
        true,
        0.0,
    );
    assert!(p.verts.is_empty());
}

#[test]
fn broken_files_are_refused_not_panicked_on() {
    let good = sample();
    assert!(open(Vec::new()).is_none());
    assert!(open(b"NOPE".to_vec()).is_none());
    for cut in [3, 10, 30, good.len() - 1] {
        // a truncated file either fails the header/index check or the chunk ends early
        let _ = open(good[..cut].to_vec()).map(|s| s.labels());
    }
    let mut bad = good.clone();
    bad[4] = 9; // another version
    assert!(open(bad).is_none());
}

#[test]
fn the_player_has_a_colour_by_line() {
    let mut s = open(sample()).unwrap();
    assert!(s.player_style(None).is_none());
    s.player = Some(Player {
        free: Color::hex(0xffffff),
        line: Color::hex(0x5588c7),
        brt: Color::hex(0x1e7b34),
        brt_lines: vec!["110".into()],
    });
    assert_eq!(s.player_style(None).map(|p| p.1), Some(false));
    assert_eq!(
        s.player_style(Some(" 110 ")).map(|p| p.0),
        Some(Color::hex(0x1e7b34))
    );
    assert_eq!(
        s.player_style(Some("76")).map(|p| p.0),
        Some(Color::hex(0x5588c7))
    );
}

#[test]
fn icon_paths_stay_inside_the_package() {
    assert_eq!(
        package_relative("icons/stop.png").as_deref(),
        Some("icons/stop.png")
    );
    assert_eq!(
        package_relative("icons\\stop.png").as_deref(),
        Some("icons/stop.png")
    );
    assert_eq!(package_relative("a/b/c.png").as_deref(), Some("a/b/c.png"));
    for bad in [
        "",
        "  ",
        "/etc/x.png",
        "\\x.png",
        "C:/a.png",
        "C:\\a.png",
        "C:a.png",
        "\\\\server\\share\\a.png",
        "icons/../../x",
        "../x.png",
        "./x.png",
        "a//b.png",
        "a/./b.png",
        "icons/",
        "a/b:c.png",
        "a/.../x.png",
        ".../x.png",
        "a/.. /x.png",
        "a/ /x.png",
    ] {
        assert_eq!(package_relative(bad), None, "{bad:?}");
    }
}

#[test]
fn icon_sheet_never_exceeds_the_gpu_safe_size() {
    let (w, h, pos) = pack_icons(&[(900, 900); 4]);
    assert_eq!((w, h), (1803, 1803));
    assert_eq!(pos[3], Some((902, 902)));
    let (w, h, pos) = pack_icons(&[(1024, 1024), (1024, 1024)]);
    assert!(w <= 2048 && h <= 2048, "{w}x{h}");
    assert_eq!(
        pos,
        vec![Some((1, 1)), None],
        "the second does not fit and is left out"
    );
    let (w, h, pos) = pack_icons(&[(10, 6), (4, 4)]);
    assert_eq!((w, h), (17, 8));
    assert_eq!(pos, vec![Some((1, 1)), Some((12, 1))]);
}

#[test]
fn an_icon_is_a_quad_inside_its_place_on_the_sheet() {
    let mut s = open(sample()).unwrap();
    s.sheet = Some(IconSheet {
        w: 10,
        h: 6,
        rgba: vec![0; 240],
        rects: vec![([0.1, 0.2, 0.9, 0.8], 0.5)],
    });
    let m = Marker {
        at: DVec2::ZERO,
        icon: 0,
        width_px: 20.0,
    };
    let mut v = Vec::new();
    s.push_icon(&m, Vec2::new(100.0, 50.0), 2.0, &mut v);
    assert_eq!(v.len(), 6);
    // 40 px wide, 20 high, centred
    assert_eq!((v[0].pos[0], v[0].pos[1]), (80.0, 40.0));
    assert_eq!((v[2].pos[0], v[2].pos[1]), (120.0, 60.0));
    assert_eq!(v[0].uv, [0.1, 0.2]);
}

#[test]
fn bus_lines_are_read_with_their_routes() {
    let text = "RLIN\t1\n# c\nORIGEM\t1000\t2000\nL\t110\tBRT\tSenador - Biblia\nR\t18.9\t35\t35\t0\tTodos\tBRT\tA - B\nP\t0 0 10 0 20 5\nS\t0 0 20 5\nL\t999\tALI\tSem rota\n";
    let lines = parse_lines(text, &|x, y| DVec2::new(x, y));
    assert_eq!(lines.len(), 1, "a line with no way is left out");
    let l = &lines[0];
    assert_eq!(
        (l.number.as_str(), l.group, l.routes.len()),
        ("110", GROUP_BRT, 1)
    );
    assert_eq!(l.routes[0].pts[2], DVec2::new(1020.0, 2005.0));
    assert_eq!(
        (
            l.routes[0].stops.len(),
            l.routes[0].minutes,
            l.routes[0].circular
        ),
        (2, 35, false)
    );
}

#[test]
fn lines_carry_a_kind_of_vehicle_and_the_dates_they_run() {
    let text = "RLIN\t1\nORIGEM\t0\t0\nL\t1\tALI\tBus\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\tT1\tALI\tTram\ttram\t0\t0\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\tM1\tALI\tMetro\tmetro\t0\t0\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\tR1\tALI\tTrain\trail\t0\t0\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\t7\tALI\tOld\tbus\t0\t19991231\nR\t1\t2\t3\t0\td\t\tA - B\nP\t0 0 5 5\n\
L\t7\tALI\tNew\tbus\t20000101\t0\nR\t1\t2\t3\t0\td\t\tA - C\nP\t5 5 0 0\n";
    let lines = parse_lines(text, &|x, y| DVec2::new(x, y));
    let kinds: Vec<u8> = lines.iter().map(|l| l.modal).collect();
    assert_eq!(
        kinds,
        [
            MODAL_BUS,
            MODAL_TRAM,
            MODAL_METRO,
            MODAL_RAIL,
            MODAL_BUS,
            MODAL_BUS
        ]
    );
    // a line with no kind or dates is a bus line open at both ends
    assert_eq!((lines[0].from, lines[0].to), (0, 0));
    assert_eq!((lines[4].from, lines[4].to), (0, 19991231));
    assert_eq!((lines[5].from, lines[5].to), (20000101, 0));
}

#[test]
fn regions_say_where_a_point_is() {
    let text = "# c\nregion = Alfa | 5 | 5 | 8 | 2 | 0,0 10,0 10,10 0,10\nregion = Beta | 0 | 0 | 0 | 0 | 20,0 30,0 30,10\nregion = Ruim | 0 | 0 | 0 | 0 | 1,1\n";
    let r = parse_regions(text, &|x, y| DVec2::new(x, y));
    assert_eq!(
        r.len(),
        2,
        "an outline with fewer than three points is left out"
    );
    assert!(point_in_poly(&r[0].poly, DVec2::new(5.0, 5.0)));
    assert!(!point_in_poly(&r[0].poly, DVec2::new(15.0, 5.0)));
    assert_eq!((r[0].box_w, r[1].box_w), (8.0, 0.0));
}

#[test]
fn the_nearest_named_street_of_a_chunk_is_found() {
    // no fills; one street of two points from (0,0) to (100,0), name index 7
    let mut b = Vec::new();
    b.extend(0u32.to_le_bytes());
    b.extend(1u32.to_le_bytes());
    b.extend([LINE_STREET, 2]);
    b.extend(8.0f32.to_le_bytes());
    b.extend(7u32.to_le_bytes());
    b.extend(2u32.to_le_bytes());
    for v in [0.0f32, 0.0, 100.0, 0.0] {
        b.extend(v.to_le_bytes());
    }
    b.extend(0u32.to_le_bytes());
    assert!(chunk_has_streets(&b));
    let mut best = None;
    street_in_chunk(&b, DVec2::new(40.0, 12.0), &mut best).unwrap();
    let (d, name, speed, heavy) = best.unwrap();
    assert!((d - 12.0).abs() < 1e-6 && name == 7 && speed == 0 && heavy == 0);
}

#[test]
fn a_street_with_limits_says_them() {
    // one street of two points, name index 3, limit 50 (the width), 30 for heavy vehicles (the flags)
    let mut b = Vec::new();
    b.extend(0u32.to_le_bytes());
    b.extend(1u32.to_le_bytes());
    b.extend([LINE_STREET_LIMITS, 30]);
    b.extend(50.0f32.to_le_bytes());
    b.extend(3u32.to_le_bytes());
    b.extend(2u32.to_le_bytes());
    for v in [0.0f32, 0.0, 100.0, 0.0] {
        b.extend(v.to_le_bytes());
    }
    b.extend(0u32.to_le_bytes());
    assert!(chunk_has_streets(&b));
    let mut best = None;
    street_in_chunk(&b, DVec2::new(40.0, 5.0), &mut best).unwrap();
    let (d, name, speed, heavy) = best.unwrap();
    assert!((d - 5.0).abs() < 1e-6 && name == 3 && speed == 50 && heavy == 30);
}

/// A chunk body with two contour lines (heights 100 and 120, one index) as parallel horizontal lines.
fn contour_chunk() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(0u32.to_le_bytes());
    b.extend(2u32.to_le_bytes());
    for (class, h, y) in [
        (LINE_CONTOUR, 100.0f32, 0.0f32),
        (LINE_CONTOUR_INDEX, 120.0, 100.0),
    ] {
        b.extend([class, 0]);
        b.extend(h.to_le_bytes());
        b.extend(u32::MAX.to_le_bytes());
        b.extend(2u32.to_le_bytes());
        for v in [0.0f32, y, 200.0, y] {
            b.extend(v.to_le_bytes());
        }
    }
    b.extend(0u32.to_le_bytes());
    b
}

#[test]
fn contour_lines_are_found_and_give_a_height_between_them() {
    let b = contour_chunk();
    assert!(chunk_has_contours(&b));
    assert!(!chunk_has_streets(&b));
    let mut found = Vec::new();
    contours_in_chunk(&b, DVec2::new(50.0, 25.0), &mut found).unwrap();
    found.sort_by(|x, y| x.0.total_cmp(&y.0));
    // 25 m from the 100 m line and 75 m from the 120 m one: a quarter of the way up
    assert_eq!(found.len(), 2);
    assert!((found[0].0 - 25.0).abs() < 1e-6 && found[0].1 == 100.0);
    let (d1, h1) = found[0];
    let (d2, h2) = found[1];
    let h = h1 + (h2 - h1) * (d1 / (d1 + d2)) as f32;
    assert!((h - 105.0).abs() < 1e-4);
}

#[test]
fn heights_are_marked_along_a_line_and_not_too_close() {
    // a straight line of 100 m segments, 2 km long
    let pts: Vec<DVec2> = (0..=20)
        .map(|i| DVec2::new(i as f64 * 100.0, 0.0))
        .collect();
    let idx = height_marks(&pts, 120, true);
    // first once 150 m are run (the segment 100-200 m), then 700 m of line further on (counted from where it was reached)
    assert_eq!(
        idx.iter().map(|m| m.at.x).collect::<Vec<_>>(),
        vec![150.0, 850.0, 1550.0]
    );
    assert!(idx.iter().all(|m| m.index && m.height == 120));
    let plain = height_marks(&pts, 100, false);
    assert_eq!(
        plain.iter().map(|m| m.at.x).collect::<Vec<_>>(),
        vec![50.0, 550.0, 1050.0, 1550.0]
    );
    // close up all of them, far away only the index ones, none nearer than the minimum
    let all: Vec<ContourMark> = plain.iter().chain(&idx).copied().collect();
    let scale = |p: DVec2| Vec2::new(p.x as f32 / 4.0, 0.0); // 4 m a pixel
    let label_at = |p: Vec2, _: &ContourMark| Rect::new(p.x - 20.0, p.y - 6.0, 40.0, 12.0);
    let close = marks_to_write(&all, 4.0, 1.0, scale, |_| true, &[], label_at);
    assert!(
        close.iter().any(|(_, m)| !m.index),
        "close up the plain lines are marked too"
    );
    for (i, (a, _)) in close.iter().enumerate() {
        assert!(close
            .iter()
            .skip(i + 1)
            .all(|(b, _)| (*a - *b).length() >= 64.0));
    }
    let far = marks_to_write(
        &all,
        20.0,
        1.0,
        |p| Vec2::new(p.x as f32 / 20.0, 0.0),
        |_| true,
        &[],
        label_at,
    );
    assert!(!far.is_empty() && far.iter().all(|(_, m)| m.index));
    assert!(marks_to_write(&all, 40.0, 1.0, scale, |_| true, &[], label_at).is_empty());
    assert!(
        marks_to_write(&all, 4.0, 1.0, scale, |_| false, &[], label_at).is_empty(),
        "outside the window"
    );
    // a label over a box already taken is left out; the others stay
    let first = close[0].0;
    let taken = [Rect::new(first.x - 5.0, first.y - 5.0, 10.0, 10.0)];
    let free = marks_to_write(&all, 4.0, 1.0, scale, |_| true, &taken, label_at);
    assert!(free.iter().all(|(p, _)| *p != first));
}

/// A chunk with a fill, a river and a street besides two contour lines (one index) long enough to be marked.
fn relief_chunk() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(1u32.to_le_bytes());
    b.push(FILL_WATER);
    b.extend(1u32.to_le_bytes());
    for v in [0.0f32, 0.0, 10.0, 0.0, 0.0, 10.0] {
        b.extend(v.to_le_bytes());
    }
    b.extend(4u32.to_le_bytes());
    for (class, h, y) in [
        (LINE_RIVER, 3.0f32, 5.0f32),
        (LINE_CONTOUR, 100.0, 0.0),
        (LINE_STREET, 0.0, 9.0),
        (LINE_CONTOUR_INDEX, 120.0, 100.0),
    ] {
        b.extend([class, 0]);
        b.extend(h.to_le_bytes());
        b.extend(u32::MAX.to_le_bytes());
        b.extend(21u32.to_le_bytes());
        for i in 0..21 {
            b.extend((i as f32 * 100.0).to_le_bytes());
            b.extend(y.to_le_bytes());
        }
    }
    b.extend(0u32.to_le_bytes());
    b
}

fn full_walk_marks(s: &MapSurface, body: &[u8]) -> Vec<ContourMark> {
    let mut out = Vec::new();
    let _ = s.walk(
        body,
        |_, _| {},
        |(class, w), pts| {
            if class == LINE_CONTOUR || class == LINE_CONTOUR_INDEX {
                out.extend(height_marks(
                    &pts,
                    (w + s.terrain_offset).round() as i32,
                    class == LINE_CONTOUR_INDEX,
                ));
            }
        },
        |_| {},
    );
    out
}

#[test]
fn skimmed_contour_marks_equal_the_full_walks_even_on_a_cut_chunk() {
    let mut s = open(wrap(relief_chunk())).unwrap();
    s.terrain_offset = 2.0;
    let body = relief_chunk();
    let mut skim = Vec::new();
    assert!(s.skim_contours(&body, &mut skim).is_some());
    assert!(!skim.is_empty());
    assert_eq!(skim, full_walk_marks(&s, &body));
    // a chunk that ends anywhere gives what the walk gives, and never panics
    for cut in 0..body.len() {
        let mut a = Vec::new();
        let _ = s.skim_contours(&body[..cut], &mut a);
        assert_eq!(a, full_walk_marks(&s, &body[..cut]), "cut at {cut}");
    }
}

fn with_contours(body: Vec<u8>) -> std::sync::Arc<MapSurface> {
    let mut s = open(wrap(body)).unwrap();
    s.has_contours = true;
    std::sync::Arc::new(s)
}

fn wait_for(s: &MapSurface) -> bool {
    (0..500).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(10));
        s.contour_marks().is_some()
    })
}

#[test]
fn no_worker_starts_until_the_marks_are_requested() {
    let s = with_contours(relief_chunk());
    std::thread::sleep(std::time::Duration::from_millis(30));
    assert!(s.contour_marks().is_none());
    assert!(!s.contour_job.load(std::sync::atomic::Ordering::Acquire));
    s.request_contour_marks();
    assert!(wait_for(&s));
    let mut want = Vec::new();
    s.skim_contours(&relief_chunk(), &mut want).unwrap();
    assert_eq!(s.contour_marks().unwrap(), want.as_slice());
}

#[test]
fn asking_for_the_marks_never_waits_for_the_worker() {
    let s = with_contours(relief_chunk());
    let (go, wait) = std::sync::mpsc::channel::<()>();
    s.request_contour_marks_with(move |s| {
        let _ = wait.recv();
        s.skim_contour_marks()
    });
    let t = std::time::Instant::now();
    assert!(s.contour_marks().is_none());
    assert!(t.elapsed() < std::time::Duration::from_millis(500));
    go.send(()).unwrap();
    assert!(wait_for(&s));
}

#[test]
fn a_failed_worker_leaves_the_heights_off_and_is_not_started_again() {
    let s = with_contours(relief_chunk());
    s.request_contour_marks_with(|_| panic!("worker failed"));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(s.contour_marks().is_none());
    s.request_contour_marks();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(s.contour_marks().is_none(), "no retry");
}

#[test]
fn contour_lines_get_fainter_as_the_view_widens() {
    assert_eq!(
        (
            contour_strength(1.0),
            contour_strength(8.0),
            contour_strength(30.0)
        ),
        (1.0, 0.5, 0.2)
    );
}

#[test]
fn colours_read_as_hex() {
    assert!(parse_color("#173341").is_some());
    assert!(parse_color("173341").is_none());
    assert!(parse_color("#12").is_none());
}
