use omsi_sim::testing::scripttex::{ScriptTexture, MAX_SCRIPT_TEXTURE_SIDE};

#[test]
fn a_huge_rect_is_clipped_to_the_texture() {
    let mut t = ScriptTexture::new(4, 3);
    t.color = [9, 8, 7, 6];
    // ~4e18 iterations if it walked the argument range
    t.rect(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
    assert!(t.rgba.chunks(4).all(|p| p == [9, 8, 7, 6]));
    let mut t = ScriptTexture::new(4, 3);
    t.color = [1, 1, 1, 1];
    t.rect(2, 1, 100, 100);
    assert_eq!(t.get(1, 1), [0; 4]);
    assert_eq!(t.get(2, 1), [1; 4]);
    assert_eq!(t.get(3, 2), [1; 4]);
    t.rect(-50, -50, -10, 2);
    t.rect(10, 0, 20, 2);
    assert_eq!(t.get(0, 0), [0; 4]);
}

#[test]
fn hostile_texture_sizes_are_clamped_not_overflowed() {
    // 65536 * 65536 * 4 wraps to 0 in u32; 30000 * 30000 * 4 is 3.6 GB
    for (w, h) in [
        (65536, 65536),
        (30000, 30000),
        (i32::MAX, i32::MAX),
        (i32::MIN, 5),
        (0, 0),
    ] {
        let mut t = ScriptTexture::new(w, h);
        assert!(
            t.width >= 1
                && t.width <= MAX_SCRIPT_TEXTURE_SIDE
                && t.height >= 1
                && t.height <= MAX_SCRIPT_TEXTURE_SIDE
        );
        assert_eq!(t.rgba.len(), t.width as usize * t.height as usize * 4);
        t.put(0, 0, [1, 2, 3, 4]);
        assert_eq!(t.get(0, 0), [1, 2, 3, 4]);
        t.renew();
    }
    let mut t = ScriptTexture::new(4, 4);
    // a picture larger than the ceiling (16384^2 * 4 wraps to 0 in u32) is ignored
    t.load(16384, 16384, &[0; 16]);
    assert_eq!((t.width, t.height), (4, 4));
    t.load(u32::MAX, 2, &[0; 16]);
    assert_eq!((t.width, t.height), (4, 4));
    assert_eq!(ScriptTexture::new(640, 480).rgba.len(), 640 * 480 * 4);
}

#[test]
fn a_loaded_bitmap_fills_the_texture_at_its_own_power_of_two_size() {
    let mut t = ScriptTexture::new(4, 2);
    // 3x1, white red blue: 4x1, stretched across
    let src = [255, 255, 255, 255, 255, 0, 0, 255, 0, 0, 255, 255];
    t.load(3, 1, &src);
    assert_eq!((t.width, t.height), (4, 1));
    assert!(t.mipmaps);
    assert_eq!(t.get(0, 0), [255, 255, 255, 255]);
    assert_eq!(t.get(3, 0), [0, 0, 255, 255]);
    // a power-of-two bitmap is taken as it is
    let px: Vec<u8> = (0..8 * 2 * 4).map(|i| i as u8).collect();
    t.load(8, 2, &px);
    assert_eq!(
        (t.width, t.height, t.rgba.as_slice()),
        (8, 2, px.as_slice())
    );
    // STNewTex: back to the declared size, empty
    t.color = [255; 4];
    t.renew();
    assert_eq!((t.width, t.height), (4, 2));
    assert!(t.rgba.iter().all(|b| *b == 0));
    assert_eq!(t.color, [0; 4]);
}

#[test]
fn locked_edits_wait_for_unlock_and_latest_release_wins() {
    let mut t = ScriptTexture::new(1, 1);
    t.locked = true;
    t.put(0, 0, [1; 4]);
    assert!(t.take_upload().is_none());
    t.unlock();
    t.locked = true;
    t.put(0, 0, [2; 4]);
    t.unlock();
    t.locked = true;
    // A distant vehicle can release several pictures before its next upload.
    assert_eq!(t.take_upload().unwrap(), vec![2; 4]);
    assert!(t.take_upload().is_none());
}

#[test]
fn unlocked_drawing_still_uploads_without_st_callbacks() {
    let mut t = ScriptTexture::new(1, 1);
    t.put(0, 0, [1; 4]);
    t.unlock();
    t.put(0, 0, [2; 4]);
    assert_eq!(t.take_upload().unwrap(), vec![2; 4]);
    assert!(t.take_upload().is_none());
}
