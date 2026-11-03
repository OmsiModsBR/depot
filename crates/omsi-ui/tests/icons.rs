//! The HUD's line-icon family (`assets/icons/custom/hud-*.svg`): the information bar, the
//! minimap, the city map and their buttons draw from it.

use omsi_ui::icons;

/// Every icon of the family, by the group the information bar and the maps use them in.
const FAMILY: &[&str] = &[
    // vehicles: the standard bus (side view) and the variations of its body
    "hud-bus",
    "hud-bus-front",
    "hud-articulated",
    "hud-tram",
    "hud-metro",
    "hud-train",
    // line and stops
    "hud-line",
    "hud-stop",
    "hud-next-stop",
    "hud-destination",
    "hud-route",
    "hud-lines",
    "hud-timetable",
    "hud-skip-stop",
    // map
    "hud-follow",
    "hud-view-top",
    "hud-view-perspective",
    "hud-zoom-in",
    "hud-zoom-out",
    "hud-city-map",
    "hud-minimap",
    "hud-terrain",
    "hud-compass",
    // trip data
    "hud-time",
    "hud-trip",
    "hud-odometer",
    "hud-temperature",
    "hud-fuel",
    "hud-passengers",
    "hud-doors-closed",
    "hud-doors-open",
    "hud-frame-rate",
    // controls
    "hud-close",
    "hud-collapse",
    "hud-expand",
    "hud-plus",
    "hud-minus",
    "hud-show",
];

#[test]
fn the_family_is_complete_and_nothing_else_is_named_like_it() {
    let built: Vec<&str> = icons::names().filter(|n| n.starts_with("hud-")).collect();
    for name in FAMILY {
        assert!(built.contains(name), "{name} is missing");
    }
    for name in &built {
        assert!(FAMILY.contains(name), "{name} is not listed in the family");
    }
}

/// One grid, one stroke: a 16 x 16 drawing of round 1.5 px lines, never filled, so the
/// icons read as one set next to each other and take the colour they are drawn in.
#[test]
fn every_icon_is_a_line_drawing_on_the_16_px_grid() {
    for name in FAMILY {
        let svg = icons::svg(name).unwrap();
        assert!(svg.contains(r#"viewBox="0 0 16 16""#), "{name}");
        assert!(svg.contains(r#"fill="none""#), "{name} must not be filled");
        assert!(svg.contains(r#"stroke-width="1.5""#), "{name}");
        assert!(svg.contains(r#"stroke-linecap="round""#), "{name}");
        // (no colour of its own, no text: the line number stays a badge, nothing to translate)
        assert!(!svg.contains("<text"), "{name} carries letters");
        assert!(!svg.contains("fill=\"#"), "{name} has a fill colour");
    }
}

#[test]
fn every_icon_draws_at_the_sizes_the_bars_use() {
    for name in FAMILY {
        for size in [12u32, 14, 16, 20, 24] {
            let a = icons::rasterize(name, size).unwrap();
            assert_eq!(a.len(), (size * size) as usize);
            let covered = a.iter().filter(|&&v| v > 96).count() as f32 / a.len() as f32;
            // (lines: neither a speck nor a blot, at the smallest size too)
            assert!(
                (0.04..0.62).contains(&covered),
                "{name} at {size} px covers {covered:.2}"
            );
            assert!(
                a.iter().any(|&v| v > 0 && v < 255),
                "{name} at {size} px has hard edges"
            );
        }
    }
}

/// The drawing keeps clear of the edge of its square, so an icon in a button or next to a
/// value is not clipped and two icons side by side do not touch.
#[test]
fn every_icon_keeps_inside_its_square() {
    for name in FAMILY {
        let size = 32u32;
        let a = icons::rasterize(name, size).unwrap();
        let edge = (0..size)
            .flat_map(|k| [(k, 0), (k, size - 1), (0, k), (size - 1, k)])
            .map(|(x, y)| a[(y * size + x) as usize])
            .max()
            .unwrap();
        assert!(edge < 200, "{name} runs into the edge ({edge})");
    }
}
