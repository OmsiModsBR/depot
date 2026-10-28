use omsi_content::input::gamectrler::{parse as parse_cfg, to_text as cfg_text};

/// A device taken out of the list is gone from the file Save writes, and the next one is
/// shown - the one below it, or above it when it was the last (#636).
#[test]
fn a_removed_device_leaves_the_file() {
    let mut devices = parse_cfg("[ctrl]\r\nSideWinder Joystick\r\n0\r\n\r\n[ctrl]\r\nLogitech G25 Racing Wheel USB\r\n1\r\n\r\n[ctrl]\r\nMOZA R3 Base\r\n0\r\n");
    let mut sel = 1;
    assert_eq!(
        openomsi_game::testing::launcher::pages::remove_device(&mut devices, &mut sel),
        "Logitech G25 Racing Wheel USB"
    );
    assert_eq!(sel, 1);
    let names = |text: &str| {
        parse_cfg(text)
            .into_iter()
            .map(|d| d.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&cfg_text(&devices)),
        ["SideWinder Joystick", "MOZA R3 Base"]
    );
    assert_eq!(
        openomsi_game::testing::launcher::pages::remove_device(&mut devices, &mut sel),
        "MOZA R3 Base"
    );
    assert_eq!(sel, 0);
    assert_eq!(
        openomsi_game::testing::launcher::pages::remove_device(&mut devices, &mut sel),
        "SideWinder Joystick"
    );
    assert_eq!(sel, 0);
    assert!(names(&cfg_text(&devices)).is_empty());
}
