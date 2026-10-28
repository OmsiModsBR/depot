use openomsi_game::testing::launcher::thumbs::*;

#[test]
fn a_picture_is_cut_to_the_thumbnails_shape() {
    let t = make(&image::RgbaImage::new(1600, 400));
    assert_eq!(t.dimensions(), (W, H));
    let t = make(&image::RgbaImage::new(300, 900));
    assert_eq!(t.dimensions(), (W, H));
}

#[test]
fn a_thumbnail_is_made_once() {
    let mut t = Thumbs::default();
    t.want("a", || Some(image::RgbaImage::new(10, 10)));
    t.want("a", || panic!("asked twice"));
    let start = std::time::Instant::now();
    while !t.update() && start.elapsed().as_secs() < 5 {
        std::thread::yield_now();
    }
    assert_eq!(t.pending.len(), 1);
    assert!(t.asked.is_empty());
}
