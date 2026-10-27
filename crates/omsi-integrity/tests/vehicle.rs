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

#[test]
fn the_textures_of_the_paint_schemes_are_checked_in_the_paint_folder() {
    let root = temp_root("paint");
    complete(&root);
    write(
        &root,
        "Vehicles/Test/model/model.cfg",
        "[mesh]\nbody.x\n[CTC]\nPaint\nTexture\\paints\n0\n[CTCTexture]\nbody_ctc\nbody.bmp\n",
    );
    write(
        &root,
        "Vehicles/Test/texture/paints/red.cti",
        "[item]\nRed\nbody_ctc\nred.bmp\n[item]\nBlue\nbody_ctc\nblue.bmp\n[item]\nRed\nroof_ctc\nroof_red.bmp\n",
    );
    write(
        &root,
        "Vehicles/Test/texture/paints/red.bmp",
        bmp(1024, 1024),
    );
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    assert_eq!(r.findings.len(), 1, "{:#?}", r.findings);
    assert_eq!(r.findings[0].code, "texture-missing");
    assert!(
        r.findings[0].file.ends_with("blue.bmp"),
        "{}",
        r.findings[0].file
    );
    // (the folder is spelled as the model.cfg writes it: `Texture` on a system that ignores case)
    assert!(r.findings[0]
        .referenced_by
        .as_ref()
        .unwrap()
        .0
        .eq_ignore_ascii_case("Vehicles/Test/texture/paints/red.cti"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_model_cfg_in_a_subfolder_finds_its_mesh_and_the_vehicles_textures() {
    let root = temp_root("subfolder");
    complete(&root);
    // model.cfg in a "Configuration Files" folder; the mesh lies in the vehicle's `model`
    // folder and the texture in the vehicle's own `Texture` folder
    write(
        &root,
        "Vehicles/Test/Configuration Files/model.cfg",
        "[mesh]\nbody.x\n",
    );
    fs::remove_file(root.join("Vehicles/Test/model/model.cfg")).unwrap();
    write(
        &root,
        "Vehicles/Test/Test.bus",
        BUS.replace("model\\model.cfg", "Configuration Files\\model.cfg"),
    );
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    assert!(r.findings.is_empty(), "{:#?}", r.findings);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_reflection_camera_texture_needs_no_file() {
    let root = temp_root("mirror");
    complete(&root);
    write(
        &root,
        "Vehicles/Test/model/body.x",
        MESH.replace("body.bmp", "reflexion0.bmp"),
    );
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    assert!(r.findings.is_empty(), "{:#?}", r.findings);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_texture_change_list_is_named_from_the_vehicles_folder() {
    let root = temp_root("texchange");
    complete(&root);
    fs::remove_file(root.join("Vehicles/Test/texture/body.bmp")).unwrap();
    // two lists: one exists beside the model.cfg as well as in the vehicle's folder, the
    // other only in the vehicle's folder
    write(
        &root,
        "Vehicles/Test/model/model.cfg",
        "[mesh]\nbody.x\n[texchanges]\ntexture\\chtex.cfg\ntexture\\chtex2.cfg\n",
    );
    write(
        &root,
        "Vehicles/Test/model/texture/chtex.cfg",
        "[newtexchangemaster]\nother.bmp\nvar\n[entries]\n1\nalt.bmp\n",
    );
    write(
        &root,
        "Vehicles/Test/texture/chtex.cfg",
        "[newtexchangemaster]\nbody.bmp\nvar\n[entries]\n1\nalt.bmp\n",
    );
    write(
        &root,
        "Vehicles/Test/texture/chtex2.cfg",
        "[newtexchangemaster]\nbody.bmp\nvar\n[entries]\n1\nalt.bmp\n",
    );
    write(&root, "Vehicles/Test/texture/alt.bmp", bmp(512, 512));
    let r = check_vehicle(&root, &root.join("Vehicles/Test/Test.bus"));
    assert!(r.findings.is_empty(), "{:#?}", r.findings);
    let _ = fs::remove_dir_all(&root);
}
