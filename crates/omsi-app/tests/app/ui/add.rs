use omsi_ui::ingame::Preview;
use openomsi_game::testing::ui::add::*;

fn window() -> AddWindow {
    let mut w = AddWindow::new(vec![
        (
            "Maker".into(),
            vec![
                ("Bus A".into(), "Vehicles/A/a.bus".into()),
                ("Bus B".into(), "Vehicles/B/b.bus".into()),
            ],
        ),
        (
            "Other".into(),
            vec![("Bus C".into(), "Vehicles/C/c.bus".into())],
        ),
    ]);
    w.choose_maker(0);
    w.choose(0, vec![], vec![], vec![], String::new());
    w
}

#[test]
fn a_manufacturer_comes_with_its_first_model() {
    let mut w = window();
    assert_eq!(w.frame().makers, vec!["Maker".to_string(), "Other".into()]);
    assert_eq!(
        w.frame().vehicles,
        vec!["Bus A".to_string(), "Bus B".into()]
    );
    w.choose(1, vec!["Red".into()], vec![], vec![], "About B".into());
    assert_eq!(w.choose_maker(1).as_deref(), Some("Vehicles/C/c.bus"));
    assert_eq!(w.frame().vehicles, vec!["Bus C".to_string()]);
    assert_eq!(w.frame().vehicle, 0);
    assert_eq!(w.choose_maker(7), None);
    assert_eq!(
        w.chosen(),
        Some(("Other".into(), "Vehicles/C/c.bus".into()))
    );
}

#[test]
fn the_window_finds_the_vehicle_chosen_last() {
    let w = window();
    assert_eq!(w.find("Maker", "Vehicles/B/b.bus"), Some((0, 1)));
    assert_eq!(w.find("Other", "Vehicles/gone.bus"), Some((1, 0)));
    assert_eq!(w.find("Gone", "Vehicles/A/a.bus"), None);
}

#[test]
fn the_first_lines_mean_random_livery_and_the_maps_depot_file() {
    let mut w = window();
    assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
    w.choose(
        1,
        vec!["Red".into()],
        vec![("Depot B  (b.hof)".into(), "b.hof".into())],
        vec![],
        String::new(),
    );
    assert_eq!(w.placement(), Some(("Vehicles/B/b.bus".into(), None, None)));
    w.set_livery(1);
    w.set_depot(1);
    assert_eq!(
        w.placement(),
        Some((
            "Vehicles/B/b.bus".into(),
            Some("Red".into()),
            Some("b.hof".into())
        ))
    );
    // another vehicle starts again on the first lines
    w.choose(0, vec![], vec![], vec![], String::new());
    assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
    // a line that is not there changes nothing
    w.set_livery(5);
    w.choose(9, vec![], vec![], vec![], String::new());
    assert_eq!(w.placement(), Some(("Vehicles/A/a.bus".into(), None, None)));
}

#[test]
fn the_number_is_picked_and_the_plate_is_typed() {
    let mut w = window();
    w.choose(
        0,
        vec![],
        vec![],
        vec!["10".into(), "11".into(), "12".into()],
        String::new(),
    );
    w.set_number(2);
    assert_eq!(w.identity(), (Some("12".into()), None));
    w.set_number(9);
    assert_eq!(w.identity(), (Some("12".into()), None));
    assert!(
        !w.type_plate("ab"),
        "the field has no keyboard before it is clicked"
    );
    w.focus_plate();
    assert!(w.type_plate(" B-XY 12\n"));
    assert_eq!(w.identity(), (Some("12".into()), Some("B-XY 12".into())));
    // another vehicle forgets both
    w.choose(1, vec![], vec![], vec!["7".into()], String::new());
    assert_eq!(w.identity(), (Some("7".into()), None));
    assert!(!w.frame().plate_focused);
}

#[test]
fn the_picture_is_of_the_paint_chosen_and_the_buss_own_for_a_random_one() {
    let mut w = window();
    w.choose(1, vec![" Red ".into()], vec![], vec![], "About B".into());
    assert_eq!(
        w.picture_look(),
        Some(("Vehicles/B/b.bus".into(), String::new()))
    );
    w.set_livery(1);
    assert_eq!(
        w.picture_look(),
        Some(("Vehicles/B/b.bus".into(), "Red".into()))
    );
    assert_eq!(w.frame().description, "About B");
}

#[test]
fn the_picture_box_waits_for_its_own_picture() {
    let mut f = window().frame();
    assert_eq!(f.preview(None, None), Preview::Missing, "nothing asked for");
    f.picture = Some(7);
    assert_eq!(f.preview(None, None), Preview::Loading);
    assert_eq!(
        f.preview(Some((6, None)), None),
        Preview::Loading,
        "another's"
    );
    assert_eq!(f.preview(Some((7, None)), None), Preview::Missing);
    assert_eq!(
        f.preview(Some((7, Some((3, 1.5, [0.1, 0.2, 0.9, 0.8])))), None),
        Preview::Picture {
            tex: 3,
            aspect: 1.5,
            part: [0.1, 0.2, 0.9, 0.8]
        }
    );
    let root = std::path::Path::new("C:/OMSI 2");
    assert_ne!(
        picture_key(root, "Vehicles/A/a.bus", ""),
        picture_key(root, "Vehicles/A/a.bus", "Red")
    );
}

#[test]
fn the_live_view_shows_its_own_frame_or_says_how_it_stands() {
    let mut f = window().frame();
    f.picture = Some(7);
    let pic = Some((7, Some((3, 1.5, [0.0, 0.0, 1.0, 1.0]))));
    assert!(!f.live_on());
    assert!(
        matches!(f.preview(pic, Some((9, 4))), Preview::Picture { .. }),
        "off: the picture, whatever frame is left"
    );
    f.live = Some((9, false));
    assert!(f.live_on());
    assert_eq!(f.preview(pic, None), Preview::Starting);
    assert_eq!(
        f.preview(pic, Some((8, 4))),
        Preview::Starting,
        "a frame of the vehicle shown before"
    );
    assert_eq!(f.preview(pic, Some((9, 4))), Preview::Live { tex: 4 });
    f.live = Some((9, true));
    assert_eq!(f.preview(pic, Some((9, 4))), Preview::Failed);
}
