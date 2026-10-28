use glam::Vec2;
use omsi_launcher_ui::look::CAPTION_H;
use omsi_launcher_ui::look::TOPBAR_H;
use openomsi_game::testing::launcher::chrome::*;
use winit::window::ResizeDirection;

#[test]
fn edges_and_corners_resize_and_the_inside_does_not() {
    let size = Vec2::new(1440.0, 880.0);
    assert_eq!(
        edge_at(Vec2::new(1.0, 400.0), size),
        Some(ResizeDirection::West)
    );
    assert_eq!(
        edge_at(Vec2::new(1438.0, 400.0), size),
        Some(ResizeDirection::East)
    );
    assert_eq!(
        edge_at(Vec2::new(700.0, 1.0), size),
        Some(ResizeDirection::North)
    );
    assert_eq!(
        edge_at(Vec2::new(700.0, 878.0), size),
        Some(ResizeDirection::South)
    );
    assert_eq!(
        edge_at(Vec2::new(2.0, 2.0), size),
        Some(ResizeDirection::NorthWest)
    );
    assert_eq!(
        edge_at(Vec2::new(1438.0, 878.0), size),
        Some(ResizeDirection::SouthEast)
    );
    assert_eq!(edge_at(Vec2::new(700.0, 400.0), size), None);
    assert_eq!(edge_at(Vec2::new(-10.0, 400.0), size), None);
}

#[test]
fn the_title_strip_ends_at_the_caption_buttons() {
    assert!(on_title_strip(Vec2::new(600.0, 10.0), 1440.0, CAPTION_H));
    assert!(!on_title_strip(Vec2::new(600.0, 40.0), 1440.0, CAPTION_H));
    assert!(on_title_strip(Vec2::new(600.0, 40.0), 1440.0, TOPBAR_H));
    assert!(!on_title_strip(
        Vec2::new(1440.0 - 60.0, 10.0),
        1440.0,
        TOPBAR_H
    ));
}
