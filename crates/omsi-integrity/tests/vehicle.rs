//! A small vehicle built from scratch in a temporary folder: nothing here is game content.

use omsi_integrity::{check_vehicle, Severity};
use std::fs;
use std::path::{Path, PathBuf};

fn temp_root(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("omsi-integrity-veh-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, rel: &str, content: impl AsRef<[u8]>) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn bmp(w: i32, h: i32) -> Vec<u8> {
    let mut b = vec![0u8; 54];
    b[0] = b'B';
    b[1] = b'M';
    b[18..22].copy_from_slice(&w.to_le_bytes());
    b[22..26].copy_from_slice(&h.to_le_bytes());
    b
}

const MESH: &str = "xof 0303txt 0032\nFrame F { Mesh {\n3; 0;0;0;, 1;0;0;, 0;1;0;;\n1; 3;0,1,2;;\n\
MeshMaterialList { 1; 1; 0;; Material { 1;1;1;1;; 2; 0;0;0;; 0;0;0;; TextureFilename { \"body.bmp\"; } } }\n} }\n";

const BUS: &str = "[friendlyname]\nMaker\nTest bus\nDefault\n[model]\nmodel\\model.cfg\n\
[script]\n1\nscript\\main.osc\n[varnamelist]\n1\nscript\\vars.txt\n[sound]\nsound\\sound.cfg\n";

fn complete(root: &Path) {
    write(root, "Vehicles/Test/Test.bus", BUS);
    write(root, "Vehicles/Test/model/model.cfg", "[mesh]\nbody.x\n");
    write(root, "Vehicles/Test/model/body.x", MESH);
    write(root, "Vehicles/Test/texture/body.bmp", bmp(1024, 1024));
    write(root, "Vehicles/Test/script/main.osc", "");
    write(root, "Vehicles/Test/script/vars.txt", "");
    write(
        root,
        "Vehicles/Test/sound/sound.cfg",
        "[sound]\nengine.wav\n1\n",
    );
    write(root, "Vehicles/Test/sound/engine.wav", "RIFF");
}

#[test]
fn a_complete_vehicle_has_no_findings() {
    let root = temp_root("complete");
    complete(&root);
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    assert!(r.findings.is_empty(), "{:#?}", r.findings);
    assert_eq!(r.textures, 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_script_sound_and_texture_are_found() {
    let root = temp_root("missing");
    complete(&root);
    fs::remove_file(root.join("Vehicles/Test/script/main.osc")).unwrap();
    fs::remove_file(root.join("Vehicles/Test/sound/engine.wav")).unwrap();
    fs::remove_file(root.join("Vehicles/Test/texture/body.bmp")).unwrap();
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    let codes = r.by_code();
    assert_eq!(codes.get("script-missing"), Some(&1), "{:#?}", r.findings);
    assert_eq!(codes.get("sound-missing"), Some(&1));
    assert_eq!(codes.get("texture-missing"), Some(&1));
    assert!(!r.is_complete());
    let sound = r
        .findings
        .iter()
        .find(|f| f.code == "sound-missing")
        .unwrap();
    assert_eq!(sound.severity, Severity::Warning);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_model_is_an_error_and_a_wasteful_texture_is_not() {
    let root = temp_root("model");
    complete(&root);
    write(&root, "Vehicles/Test/texture/body.bmp", bmp(1000, 600));
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    assert_eq!(r.findings.len(), 1, "{:#?}", r.findings);
    assert_eq!(r.findings[0].code, "texture-npot");
    assert!(r.is_complete());
    fs::remove_file(root.join("Vehicles/Test/model/model.cfg")).unwrap();
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    assert!(r.findings.iter().any(|f| f.code == "model-missing"));
    assert!(!r.is_complete());
    let _ = fs::remove_dir_all(&root);
}
