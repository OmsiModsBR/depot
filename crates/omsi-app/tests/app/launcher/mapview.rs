use glam::DVec2;
use openomsi_game::testing::launcher::mapview::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Three entry points are in the file, the second of which has no place on this map: the
/// drawn markers are the file's first and third, and must answer with those places.
fn map() -> MapView {
    let mut m = MapView::new();
    m.roads = Some(Arc::new(Roads {
        entries: vec![
            Entry {
                at: DVec2::new(0.0, 0.0),
                name: "Depot".into(),
                index: 0,
            },
            Entry {
                at: DVec2::new(100.0, 0.0),
                name: "Station".into(),
                index: 2,
            },
        ],
        ..Default::default()
    }));
    m
}

#[test]
fn a_marker_answers_with_its_own_place_in_the_file() {
    let mut m = map();
    // the second drawn marker is the file's third entry point, not its second
    m.clicked = Some(1);
    assert_eq!(m.take_clicked(), Some(2));
    m.clicked = Some(0);
    assert_eq!(m.take_clicked(), Some(0));
    // a click is taken once
    assert_eq!(m.take_clicked(), None);
}

#[test]
fn the_choice_finds_its_marker_by_the_files_own_place() {
    let m = map();
    assert_eq!(m.shown_of(0), Some(0));
    assert_eq!(m.shown_of(2), Some(1));
    // the entry point that could not be placed has no marker to ring
    assert_eq!(m.shown_of(1), None);
    // automatic, and anything out of the list
    assert_eq!(m.shown_of(-1), None);
    assert_eq!(m.shown_of(9), None);
}

/// The thinning a plan is built at is rounded to a power of two, so a wheel that keeps
/// turning asks for the whole map to be built again only when it crosses into another band.
#[test]
fn a_zoom_builds_the_plan_again_only_across_a_band() {
    let mut m = MapView::new();
    m.mpp = 4.0;
    let band = m.tolerance();
    assert_eq!(
        band, 2.0,
        "half of four metres a pixel, which is a power of two"
    );
    // a band is a factor of two wide: a wheel notch moves the metres a pixel by 12 %, and
    // six or seven of them, a whole gesture, stay inside the one the plan was built for
    for factor in [1.12, 1.25, 1.12 * 1.12, 0.88, 0.8, 0.88 * 0.88] {
        m.mpp = 4.0 * factor;
        assert_eq!(m.tolerance(), band, "at {factor} of four metres a pixel");
    }
    // twice as close, and twice as far out, are other bands
    m.mpp = 2.0;
    assert_eq!(m.tolerance(), 1.0);
    m.mpp = 8.0;
    assert_eq!(m.tolerance(), 4.0);
    // and the far out view is held where the clamp always held it
    m.mpp = 800.0;
    assert_eq!(m.tolerance(), 24.0);
    m.mpp = 100_000.0;
    assert_eq!(m.tolerance(), 24.0);
}

/// A zoom that keeps moving does not build the plan again: what stands on the screen is the
/// same map, thinned for the zoom before it, and the camera is a matrix over it either way.
#[test]
fn a_zoom_in_progress_keeps_the_plan_it_has() {
    let mut m = map();
    let built = Key {
        global: PathBuf::from("maps/Grundorf/global.cfg"),
        trip: String::new(),
        tolerance: 2.0f32.to_bits(),
    };
    let finer = Key {
        tolerance: 1.0f32.to_bits(),
        ..built.clone()
    };
    let plan = || {
        Some(Plan {
            key: built.clone(),
            verts: Vec::new(),
            count: 0,
            uploaded: true,
        })
    };
    m.plan = plan();
    // (whatever the zoom now asks for, while the wheel is still turning)
    m.wheeled = Some(Instant::now());
    assert!(!m.plan_is_stale(&finer));
    assert!(!m.plan_is_stale(&built));
    // (and a map that has no plan at all is always built one: it shows nothing until then)
    m.plan = None;
    assert!(m.plan_is_stale(&finer));
    // the wheel has stopped: the band it stopped in is built
    m.plan = plan();
    m.wheeled = Some(Instant::now() - std::time::Duration::from_secs(1));
    assert!(m.plan_is_stale(&finer));
    assert!(!m.plan_is_stale(&built));
}
