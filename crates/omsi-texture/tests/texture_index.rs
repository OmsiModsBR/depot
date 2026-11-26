//! A texture missing from its object's folders is taken from the nearest same-named file under
//! `Sceneryobjects`, through an index of the content that can be built ahead of the first look-up.

use omsi_texture::{find_texture, prewarm_texture_index};
use std::fs;

#[test]
fn a_missing_texture_is_found_in_a_sibling_folder_after_the_index_is_prewarmed() {
    let root = std::env::temp_dir().join(format!("omsi-texture-index-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let wanted = root.join("Sceneryobjects/Alpha/texture");
    let asking = root.join("Sceneryobjects/Beta/texture");
    fs::create_dir_all(&wanted).unwrap();
    fs::create_dir_all(&asking).unwrap();
    fs::write(wanted.join("lamp.bmp"), b"x").unwrap();
    // a deeper file of another folder with the same name, farther from the asker's
    let deep = root.join("Sceneryobjects/Gamma/more/other");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("lamp.bmp"), b"x").unwrap();

    omsi_cfg::add_content_root(root.clone());
    prewarm_texture_index();
    let found = find_texture("lamp.bmp", &[asking.as_path()]).expect("found elsewhere");
    assert_eq!(found, wanted.join("lamp.bmp"));
    assert!(find_texture("nothing.bmp", &[asking.as_path()]).is_none());
    let _ = fs::remove_dir_all(&root);
}
