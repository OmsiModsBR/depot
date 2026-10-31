//! A map's picture (`omsi_launcher_ui::picture`): which file of the map's folder it is, and
//! how it is read.

use omsi_launcher_ui::picture::MapPicture;
use std::path::PathBuf;

/// A folder of its own under the system's temporary folder, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("depot-pic-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn png(path: &std::path::Path, w: u32, h: u32) {
    image::RgbaImage::from_pixel(w, h, image::Rgba([200, 120, 40, 255]))
        .save(path)
        .unwrap();
}

/// The picture is the file named "picture" in any case, of a kind the reader knows; the
/// first by name when there are several.
#[test]
fn the_picture_is_the_file_called_picture() {
    let dir = scratch("find");
    std::fs::write(dir.join("picture.txt"), b"not a picture").unwrap();
    std::fs::write(dir.join("global.cfg"), b"").unwrap();
    assert_eq!(MapPicture::find(&dir), None);
    png(&dir.join("Picture.PNG"), 4, 4);
    png(&dir.join("picture.png"), 4, 4);
    assert_eq!(MapPicture::find(&dir), Some(dir.join("Picture.PNG")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A picture is read as it is when small, made to fit 1280 x 1280 when larger, and a file that
/// is no picture gives none.
#[test]
fn a_large_picture_is_made_to_fit() {
    let dir = scratch("read");
    png(&dir.join("small.png"), 640, 480);
    png(&dir.join("large.png"), 2560, 1440);
    std::fs::write(dir.join("broken.png"), b"not a picture").unwrap();
    let small = MapPicture::read(&dir.join("small.png")).unwrap();
    assert_eq!(small.dimensions(), (640, 480));
    let large = MapPicture::read(&dir.join("large.png")).unwrap();
    assert_eq!(large.dimensions(), (1280, 720));
    assert!(MapPicture::read(&dir.join("broken.png")).is_none());
    assert!(MapPicture::read(&dir.join("missing.png")).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The folder of a map is the one its `global.cfg` lies in.
#[test]
fn a_maps_folder_holds_its_global_cfg() {
    let root = scratch("folder");
    std::fs::create_dir_all(root.join("maps").join("Town")).unwrap();
    std::fs::write(root.join("maps").join("Town").join("global.cfg"), b"").unwrap();
    assert_eq!(
        MapPicture::folder(&root, "maps/Town/global.cfg"),
        Some(root.join("maps").join("Town"))
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A new picture holder has nothing to show and is not reading.
#[test]
fn a_new_picture_shows_nothing() {
    let p = MapPicture::default();
    assert_eq!(p.shown(), None);
    assert!(!p.busy());
}
