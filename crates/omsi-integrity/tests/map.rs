//! A small map built from scratch in a temporary folder: nothing here is game content.

use omsi_integrity::{check_map, image_size, next_pow2, Severity};
use std::fs;
use std::path::{Path, PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omsi-integrity-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, rel: &str, content: impl AsRef<[u8]>) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

/// A bmp with just a header: the check reads the size from it and nothing else.
fn bmp(w: i32, h: i32) -> Vec<u8> {
    let mut b = vec![0u8; 54];
    b[0] = b'B';
    b[1] = b'M';
    b[18..22].copy_from_slice(&w.to_le_bytes());
    b[22..26].copy_from_slice(&h.to_le_bytes());
    b
}

const MESH: &str = "xof 0303txt 0032\nFrame F { Mesh {\n3; 0;0;0;, 1;0;0;, 0;1;0;;\n1; 3;0,1,2;;\n\
MeshMaterialList { 1; 1; 0;; Material { 1;1;1;1;; 2; 0;0;0;; 0;0;0;; TextureFilename { \"wall.bmp\"; } } }\n} }\n";

fn map(root: &Path, tile: &str) {
    write(
        root,
        "Maps/Test/global.cfg",
        "[name]\nTest\n\n[map]\n0\n0\ntile_0_0.map\n",
    );
    write(root, "Maps/Test/tile_0_0.map", tile);
}

const TILE: &str = "[version]\n14\n\n\
[spline]\n0\nSplines\\road.sli\n100\n0\n101\n10\n0\n20\n0\n50\n0\n0\n0\n0\n0\n0\n0\n0\n\n\
Object Nr. 0\n[object]\n0\nSceneryobjects\\house\\house.sco\n7\n5\n6\n0.25\n90\n0\n0\n0\n";

fn complete(root: &Path) {
    write(root, "Sceneryobjects/house/house.sco", "[mesh]\nhouse.x\n");
    write(root, "Sceneryobjects/house/model/house.x", MESH);
    write(root, "Sceneryobjects/house/texture/wall.bmp", bmp(512, 512));
    write(root, "Splines/road.sli", "[texture]\nroad.bmp\n");
    write(root, "Splines/texture/road.bmp", bmp(256, 256));
}

#[test]
fn a_complete_map_has_no_findings() {
    let root = temp_root("complete");
    complete(&root);
    map(&root, TILE);
    let r = check_map(&root, &root.join("Maps/Test"));
    assert!(r.findings.is_empty(), "{:#?}", r.findings);
    assert_eq!((r.tiles, r.objects, r.splines, r.textures), (1, 1, 1, 2));
    assert!(r.is_complete());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_object_spline_and_texture_are_named_once_with_their_users() {
    let root = temp_root("missing");
    map(&root, TILE);
    write(&root, "Sceneryobjects/house/house.sco", "[mesh]\nhouse.x\n");
    write(&root, "Sceneryobjects/house/model/house.x", MESH);
    let r = check_map(&root, &root.join("Maps/Test"));
    let codes = r.by_code();
    assert_eq!(codes.get("spline-missing"), Some(&1), "{:#?}", r.findings);
    assert_eq!(codes.get("texture-missing"), Some(&1));
    assert!(!r.is_complete());
    let spline = r
        .findings
        .iter()
        .find(|f| f.code == "spline-missing")
        .unwrap();
    assert_eq!(spline.severity, Severity::Error);
    assert_eq!(spline.file, "Splines\\road.sli");
    assert_eq!(spline.referenced_by.as_ref().unwrap().1, 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_tile_is_a_warning_not_an_error() {
    let root = temp_root("tile");
    write(
        &root,
        "Maps/Test/global.cfg",
        "[name]\nTest\n\n[map]\n0\n0\ntile_0_0.map\n",
    );
    let r = check_map(&root, &root.join("Maps/Test"));
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].code, "tile-missing");
    assert_eq!(r.findings[0].severity, Severity::Warning);
    assert!(r.is_complete());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_texture_that_is_not_a_power_of_two_says_what_directx_does_with_it() {
    let root = temp_root("npot");
    complete(&root);
    write(
        &root,
        "Sceneryobjects/house/texture/wall.bmp",
        bmp(1400, 1400),
    );
    map(&root, TILE);
    let r = check_map(&root, &root.join("Maps/Test"));
    assert_eq!(r.findings.len(), 1, "{:#?}", r.findings);
    let f = &r.findings[0];
    assert_eq!(f.code, "texture-npot");
    assert_eq!(f.severity, Severity::NotOptimized);
    assert!(f.message.contains("1400x1400") && f.message.contains("2048x2048"));
    assert!(f.detail.as_ref().unwrap().contains("DirectX 9"));
    assert!(r.is_complete());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_texture_is_read_by_its_content_and_not_by_its_extension() {
    let root = temp_root("size");
    // a bmp kept under a .jpg name; a header that is none of the formats
    write(&root, "a.jpg", bmp(640, -480));
    write(&root, "b.bmp", b"not a picture at all, just text");
    assert_eq!(image_size(&root.join("a.jpg")), Some((640, 480)));
    assert_eq!(image_size(&root.join("b.bmp")), None);
    assert_eq!(next_pow2(1400), 2048);
    assert_eq!(next_pow2(1024), 1024);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_night_map_a_material_declares_must_exist() {
    let root = temp_root("night");
    complete(&root);
    write(
        &root,
        "Sceneryobjects/house/house.sco",
        "[mesh]\nhouse.x\n[matl]\nwall.bmp\n0\n[matl_nightmap]\nwall_night.bmp\n",
    );
    map(&root, TILE);
    let r = check_map(&root, &root.join("Maps/Test"));
    assert_eq!(r.findings.len(), 1, "{:#?}", r.findings);
    assert_eq!(r.findings[0].code, "texture-missing");
    assert_eq!(r.findings[0].file, "wall_night.bmp");
    write(
        &root,
        "Sceneryobjects/house/texture/wall_night.bmp",
        bmp(512, 512),
    );
    // (a texture lookup that missed is remembered until the content changes)
    omsi_cfg::content_changed();
    let again = check_map(&root, &root.join("Maps/Test"));
    assert!(again.findings.is_empty(), "{:#?}", again.findings);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn generated_and_null_textures_are_not_looked_for() {
    let root = temp_root("generated");
    complete(&root);
    fs::remove_file(root.join("Sceneryobjects/house/texture/wall.bmp")).unwrap();
    // the mesh's texture is a placeholder for a picture the game draws
    write(
        &root,
        "Sceneryobjects/house/house.sco",
        "[mesh]\nhouse.x\n[matl]\nwall.bmp\n0\n[useTextTexture]\n1\n",
    );
    map(&root, TILE);
    let r = check_map(&root, &root.join("Maps/Test"));
    assert!(r.findings.is_empty(), "{:#?}", r.findings);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_object_that_borrows_a_model_finds_its_textures_beside_the_model() {
    let root = temp_root("shared");
    complete(&root);
    // a second object of another folder uses the same mesh, with no texture of its own
    write(
        &root,
        "Sceneryobjects/other/other.sco",
        "[model]\n..\\house\\house.sco\n",
    );
    let tile = format!("{TILE}\nObject Nr. 1\n[object]\n0\nSceneryobjects\\other\\other.sco\n8\n5\n6\n0.25\n90\n0\n0\n0\n");
    map(&root, &tile);
    let r = check_map(&root, &root.join("Maps/Test"));
    assert!(r.is_complete(), "{:#?}", r.findings);
    let _ = fs::remove_dir_all(&root);
}
