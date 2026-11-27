//! `[matl_change]` and the material commands after it.

use omsi_cfg::CfgFile;
use omsi_model::Model;

/// The commands after a `[matl_change]` change the slot's own material, not the switch:
/// an LED matrix panel (a stray `[matl_item]` before the change, then the script texture as
/// transparency map and `[matl_alpha] 2`) is masked by the script texture.
#[test]
fn commands_after_a_material_change_modify_the_slot_material() {
    let text = "[mesh]\nvm16_front_LED.o3d\n[matl]\nvmsobol_LED_1.tga\n0\n[matl_item]\n[matl_change]\nvmsobol_LED_1.tga\n0\nMatrix_alphascale2\n[matl_nightmap]\nvmsobol_LED_1.tga\n[matl_transmap]\n\\S:1\n[matl_alpha]\n2\n";
    let m = Model::parse(&CfgFile::from_str("model.cfg", text));
    let mats = &m.meshes[0].materials;
    assert!(
        mats.iter().all(|x| !x.item),
        "a [matl_item] before any change opens nothing"
    );
    let plain: Vec<_> = mats.iter().filter(|x| x.change.is_none()).collect();
    assert_eq!(plain.len(), 1);
    let base = plain[0];
    assert_eq!((base.alpha, base.alpha_set), (2, true));
    assert_eq!(base.transmap.as_deref(), Some("\\S:1"));
    assert_eq!(base.nightmap.as_deref(), Some("vmsobol_LED_1.tga"));
    let switch = mats
        .iter()
        .find(|x| x.change.is_some())
        .expect("the switch");
    assert_eq!(
        switch.change.as_ref().map(|c| c.2.as_str()),
        Some("Matrix_alphascale2")
    );
}

/// A `[matl_item]` copies the slot's material as the commands after the `[matl_change]`
/// left it; what follows the item changes the item only.
#[test]
fn a_material_item_copies_the_slot_material_after_the_change() {
    let text = "[mesh]\nlamp.o3d\n[matl_change]\nlamp.bmp\n0\nlamp_on\n[matl_alpha]\n1\n[matl_item]\n[matl_lightmap]\nlamp_lit.bmp\n";
    let m = Model::parse(&CfgFile::from_str("model.cfg", text));
    let mats = &m.meshes[0].materials;
    let base = mats
        .iter()
        .find(|x| !x.item && x.change.is_none())
        .expect("the slot material");
    let item = mats.iter().find(|x| x.item).expect("the item");
    assert_eq!((base.alpha, item.alpha), (1, 1));
    assert!(base.lightmap.is_none());
    assert_eq!(
        item.lightmap.as_ref().map(|l| l.0.as_str()),
        Some("lamp_lit.bmp")
    );
    assert_eq!(item.change.as_ref().map(|c| c.2.as_str()), Some("lamp_on"));
}
