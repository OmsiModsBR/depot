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
    assert_eq!(r.findings[0].file, "Sceneryobjects/house/texture/wall_night.bmp");
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

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b
}

fn dds(w: u32, h: u32) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..4].copy_from_slice(b"DDS ");
    b[12..16].copy_from_slice(&h.to_le_bytes());
    b[16..20].copy_from_slice(&w.to_le_bytes());
    b
}

fn tga(w: u16, h: u16) -> Vec<u8> {
    let mut b = vec![0u8; 18];
    b[2] = 2;
    b[12..14].copy_from_slice(&w.to_le_bytes());
    b[14..16].copy_from_slice(&h.to_le_bytes());
    b
}

fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut b = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 0x08,
    ];
    b.extend(h.to_be_bytes());
    b.extend(w.to_be_bytes());
    b
}

#[test]
fn every_picture_format_gives_its_size_and_a_damaged_header_is_refused() {
    let root = temp_root("formats");
    write(&root, "a.png", png(300, 200));
    write(&root, "b.dds", dds(1024, 512));
    write(&root, "c.tga", tga(64, 48));
    write(&root, "d.jpg", jpeg(1920, 1080));
    // a header that claims more than any texture can be, and a truncated jpeg
    write(&root, "e.png", png(0xFFFF_FFFF, 5));
    write(&root, "f.dds", dds(3_000_000_000, 3_000_000_000));
    write(&root, "g.jpg", &jpeg(8, 8)[..12]);
    assert_eq!(image_size(&root.join("a.png")), Some((300, 200)));
    assert_eq!(image_size(&root.join("b.dds")), Some((1024, 512)));
    assert_eq!(image_size(&root.join("c.tga")), Some((64, 48)));
    assert_eq!(image_size(&root.join("d.jpg")), Some((1920, 1080)));
    assert_eq!(image_size(&root.join("e.png")), None);
    assert_eq!(image_size(&root.join("f.dds")), None);
    assert_eq!(image_size(&root.join("g.jpg")), None);
    assert_eq!(next_pow2(u32::MAX), u32::MAX);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_mixed_size_is_enlarged_on_the_axis_that_needs_it() {
    let root = temp_root("mixed");
    complete(&root);
    write(
        &root,
        "Sceneryobjects/house/texture/wall.bmp",
        bmp(1024, 1400),
    );
    map(&root, TILE);
    let r = check_map(&root, &root.join("Maps/Test"));
    assert_eq!(r.findings.len(), 1, "{:#?}", r.findings);
    assert!(r.findings[0].message.contains("1024x2048"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_file_used_by_several_files_is_one_finding_with_the_count() {
    let root = temp_root("count");
    complete(&root);
    let tile = format!("{TILE}\nObject Nr. 1\n[object]\n0\nSceneryobjects\\gone.sco\n8\n5\n6\n0.25\n90\n0\n0\n0\nObject Nr. 2\n[object]\n0\nSceneryobjects\\gone.sco\n9\n5\n6\n0.25\n90\n0\n0\n0\n");
    map(&root, &tile);
    let r = check_map(&root, &root.join("Maps/Test"));
    let f = r
        .findings
        .iter()
        .find(|f| f.code == "object-missing")
        .unwrap();
    assert_eq!(f.referenced_by.as_ref().unwrap().1, 2);
    assert_eq!(
        r.findings
            .iter()
            .filter(|f| f.code == "object-missing")
            .count(),
        1
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_model_and_a_missing_mesh_are_errors() {
    let root = temp_root("model-mesh");
    complete(&root);
    fs::remove_file(root.join("Sceneryobjects/house/model/house.x")).unwrap();
    map(&root, TILE);
    let r = check_map(&root, &root.join("Maps/Test"));
    assert_eq!(
        r.by_code().get("mesh-missing"),
        Some(&1),
        "{:#?}",
        r.findings
    );
    write(
        &root,
        "Sceneryobjects/house/house.sco",
        "[model]\nmissing\\model.cfg\n",
    );
    let r = check_map(&root, &root.join("Maps/Test"));
    assert_eq!(
        r.by_code().get("model-missing"),
        Some(&1),
        "{:#?}",
        r.findings
    );
    assert!(!r.is_complete());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_path_over_259_characters_is_a_warning() {
    let root = temp_root("long");
    complete(&root);
    let deep = "d".repeat(120);
    write(
        &root,
        &format!("Sceneryobjects/house/texture/{deep}/{deep}/wall.bmp"),
        bmp(256, 256),
    );
    write(
        &root,
        "Sceneryobjects/house/house.sco",
        format!(
            "[mesh]\nhouse.x\n[matl]\nwall.bmp\n0\n[matl_nightmap]\n{deep}\\{deep}\\wall.bmp\n"
        ),
    );
    map(&root, TILE);
    let r = check_map(&root, &root.join("Maps/Test"));
    if root.to_string_lossy().len() + 150 + 240 > 259 {
        assert_eq!(
            r.by_code().get("path-too-long"),
            Some(&1),
            "{:#?}",
            r.findings
        );
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_ground_texture_is_looked_for_from_the_installation_folder_only() {
    let root = temp_root("ground");
    complete(&root);
    write(
        &root,
        "Maps/Test/global.cfg",
        "[name]\nTest\n\n[groundtex]\nTexture\\gras.bmp\ndetail.bmp\n8\n4\n4\n\n[map]\n0\n0\ntile_0_0.map\n",
    );
    write(&root, "Maps/Test/tile_0_0.map", TILE);
    // `detail.bmp` is bare, so the game does not find it in `Texture`
    write(&root, "Texture/detail.bmp", bmp(256, 256));
    write(&root, "Texture/gras.bmp", bmp(256, 256));
    let r = check_map(&root, &root.join("Maps/Test"));
    let missing: Vec<&str> = r
        .findings
        .iter()
        .filter(|f| f.code == "texture-missing")
        .map(|f| f.file.as_str())
        .collect();
    assert_eq!(missing, ["detail.bmp"], "{:#?}", r.findings);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn findings_the_baseline_has_too_are_marked_known() {
    let before = temp_root("base-before");
    complete(&before);
    fs::remove_file(before.join("Splines/road.sli")).unwrap();
    map(&before, TILE);
    let base = check_map(&before, &before.join("Maps/Test"));

    // the optimized copy: the same spline is still missing, and a new texture problem
    let after = temp_root("base-after");
    complete(&after);
    fs::remove_file(after.join("Splines/road.sli")).unwrap();
    write(
        &after,
        "Sceneryobjects/house/texture/wall.bmp",
        bmp(1400, 1400),
    );
    map(&after, TILE);
    let mut r = check_map(&after, &after.join("Maps/Test"));
    r.mark_known(&base);
    let spline = r
        .findings
        .iter()
        .find(|f| f.code == "spline-missing")
        .unwrap();
    let npot = r
        .findings
        .iter()
        .find(|f| f.code == "texture-npot")
        .unwrap();
    assert!(spline.known && !npot.known);
    assert_eq!(r.count_new(Severity::Error), 0);
    assert_eq!(r.count_new(Severity::NotOptimized), 1);
    let _ = fs::remove_dir_all(&before);
    let _ = fs::remove_dir_all(&after);
}

#[test]
fn two_missing_textures_of_one_name_in_different_folders_are_two_findings() {
    let root = temp_root("same-name");
    complete(&root);
    write(&root, "Sceneryobjects/other/other.sco", "[mesh]\nhouse.x\n");
    write(&root, "Sceneryobjects/other/model/house.x", MESH);
    fs::remove_file(root.join("Sceneryobjects/house/texture/wall.bmp")).unwrap();
    let tile = format!("{TILE}\nObject Nr. 1\n[object]\n0\nSceneryobjects\\other\\other.sco\n8\n5\n6\n0.25\n90\n0\n0\n0\n");
    map(&root, &tile);
    let r = check_map(&root, &root.join("Maps/Test"));
    let missing: Vec<&str> = r
        .findings
        .iter()
        .filter(|f| f.code == "texture-missing")
        .map(|f| f.file.as_str())
        .collect();
    assert_eq!(missing.len(), 2, "{missing:?}");
    let _ = fs::remove_dir_all(&root);
}
