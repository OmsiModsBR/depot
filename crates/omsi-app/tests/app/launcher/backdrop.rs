use openomsi_game::testing::launcher::backdrop::*;
use std::sync::mpsc::channel;

#[test]
fn a_picture_is_blurred_small_keeping_its_shape() {
    let mut img = image::RgbaImage::new(1600, 900);
    for (x, _, p) in img.enumerate_pixels_mut() {
        *p = if x % 2 == 0 {
            image::Rgba([255, 255, 255, 255])
        } else {
            image::Rgba([0, 0, 0, 255])
        };
    }
    let b = blur(&image::DynamicImage::ImageRgba8(img));
    assert_eq!(b.dimensions(), (WIDTH, 180));
    // (a tall one keeps its shape)
    let tall = blur(&image::DynamicImage::ImageRgba8(image::RgbaImage::new(
        100, 1000,
    )));
    assert_eq!(tall.dimensions(), (64, 640));
    // (the stripes are gone: one grey)
    let p = b.get_pixel(160, 90).0;
    assert!((100..=155).contains(&p[0]), "{p:?}");
}

#[test]
fn what_was_blurred_is_kept_and_shown_without_reading_again() {
    let mut b = Backdrop::default();
    b.keep(Want::Latest, (Some(image::RgbaImage::new(4, 2)), true));
    b.want(Want::Latest);
    assert!(b.reading.is_empty());
    b.update();
    assert!(b.pending.is_some());
    // (a bus waits for its picture: nothing is read for it)
    b.want(Want::Bus(7));
    assert!(b.reading.is_empty());
    b.offer_bus(7, 2, 1, &[0; 8]);
    assert_eq!(b.reading.len(), 1);
    b.offer_bus(7, 2, 1, &[0; 8]);
    assert_eq!(b.reading.len(), 1);
}

#[test]
fn screenshots_are_read_again_after_a_game_and_the_last_stays_meanwhile() {
    let mut b = Backdrop::default();
    b.keep(Want::Latest, (Some(image::RgbaImage::new(4, 2)), true));
    b.keep(Want::Bus(3), (Some(image::RgbaImage::new(4, 2)), false));
    b.want(Want::Latest);
    b.update();
    // (as `upload` leaves it)
    b.shown = b.pending.take().map(|p| p.0);
    b.game_running(true);
    assert!(b.kept.contains_key(&Want::Latest));
    b.game_running(false);
    assert!(!b.kept.contains_key(&Want::Latest));
    assert!(b.kept.contains_key(&Want::Bus(3)));
    b.want(Want::Latest);
    assert_eq!(b.reading.len(), 1);
    // (the reading held open here: the real one may already be done, there being no
    // screenshots where the tests run)
    let (_still, rx) = channel();
    b.reading[0].1 = rx;
    // (still reading: the old picture stays; read: it replaces it)
    b.update();
    assert_eq!(b.shown, Some(Want::Latest));
    assert!(b.pending.is_none());
    b.reading.clear();
    b.keep(Want::Latest, (Some(image::RgbaImage::new(2, 2)), true));
    b.update();
    assert!(b.pending.is_some());
}

#[test]
fn only_the_latest_pictures_are_kept() {
    let mut b = Backdrop::default();
    for t in 0..KEPT as u64 + 5 {
        b.keep(Want::Bus(t), (None, false));
    }
    assert_eq!(b.kept.len(), KEPT);
    assert!(!b.kept.contains_key(&Want::Bus(0)));
    assert!(b.kept.contains_key(&Want::Bus(KEPT as u64 + 4)));
}
