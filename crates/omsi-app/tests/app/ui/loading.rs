use openomsi_game::testing::ui::loading::*;
use std::path::PathBuf;

#[test]
fn the_heading_and_the_stage_read_as_in_the_mockup() {
    assert_eq!(
        heading("Example map", Some("50"), "08:00"),
        "Example map \u{b7} Line 50 \u{b7} 08:00"
    );
    assert_eq!(
        heading("Example map", None, "08:00"),
        "Example map \u{b7} 08:00"
    );
    assert_eq!(heading("", Some(" "), "08:00"), "08:00");
    assert_eq!(heading("Example map", None, " "), "Example map");
    assert_eq!(activity(0), "Reading the map");
    assert_eq!(activity(62), "Preparing the world");
    assert_eq!(
        stage(62, 100),
        "Loading map tiles (first area) \u{b7} 62 of 100"
    );
}

#[test]
fn the_bus_block_reads_as_in_the_mockup() {
    let v = omsi_vehicle::Vehicle {
        manufacturer: "Example Maker".into(),
        type_name: "Example bus A".into(),
        default_paint: "Default paint".into(),
        description: "Author's description,\nfirst lines.".into(),
        ..Default::default()
    };
    let c = bus_card(&v, "EXAMPLE_BUS", None);
    assert_eq!(c.title, "Example bus A");
    assert_eq!(c.sub, "Example Maker");
    assert_eq!(c.detail, "Livery: Default paint");
    // a paint asked for by name is shown, by number the default one
    assert_eq!(bus_card(&v, "", Some("Red")).detail, "Livery: Red");
    assert_eq!(bus_card(&v, "", Some("3")).detail, "Livery: Default paint");
    // a file with no name: the folder's
    let bare = omsi_vehicle::Vehicle::default();
    let c = bus_card(&bare, "EXAMPLE_BUS", None);
    assert_eq!(
        (c.title.as_str(), c.sub.as_str(), c.detail.as_str()),
        ("EXAMPLE_BUS", "", "")
    );
}

#[test]
fn the_slideshow_keeps_the_latest_screenshots_in_order() {
    let p = |n: u32| PathBuf::from(format!("omsi_{n}.png"));
    let all: Vec<PathBuf> = (1..=10).map(p).collect();
    assert_eq!(recent(all.clone(), 3), vec![p(8), p(9), p(10)]);
    assert_eq!(recent(all[..2].to_vec(), 6), all[..2].to_vec());
    assert!(recent(Vec::new(), 6).is_empty());
}

#[test]
fn a_wide_screenshot_is_shrunk_and_a_bad_file_is_none() {
    let dir = std::env::temp_dir().join(format!("depot-loading-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wide = dir.join("wide.png");
    image::RgbaImage::from_pixel(2560, 1440, image::Rgba([10, 20, 30, 255]))
        .save(&wide)
        .unwrap();
    let small = dir.join("small.png");
    image::RgbaImage::from_pixel(400, 300, image::Rgba([1, 2, 3, 255]))
        .save(&small)
        .unwrap();
    let junk = dir.join("junk.png");
    std::fs::write(&junk, b"not a picture").unwrap();
    let d = decode(&wide).unwrap();
    assert_eq!((d.w, d.h), (1280, 720));
    assert_eq!(d.rgba.len(), (d.w * d.h * 4) as usize);
    let d = decode(&small).unwrap();
    assert_eq!((d.w, d.h), (400, 300));
    assert!(decode(&junk).is_none());
    assert!(decode(&dir.join("missing.png")).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
