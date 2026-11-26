//! How a material casts into the shadow maps: the one rule the shadow lists follow
//! (`renderer/lists.rs`), row by row.

use omsi_render::testing::{shadow_caster, ShadowCaster};
use omsi_render::AlphaMode;

#[test]
fn an_opaque_material_casts_a_solid_shadow() {
    for transmap in [false, true] {
        for no_z_write in [false, true] {
            assert_eq!(
                shadow_caster(AlphaMode::Opaque, transmap, no_z_write, false),
                Some(ShadowCaster::Solid)
            );
        }
    }
}

#[test]
fn an_alpha_tested_material_casts_its_cut_out() {
    for transmap in [false, true] {
        for no_z_write in [false, true] {
            assert_eq!(
                shadow_caster(AlphaMode::Test, transmap, no_z_write, false),
                Some(ShadowCaster::CutOut)
            );
        }
    }
}

#[test]
fn a_blended_body_cut_by_its_transmap_casts_its_cut_out() {
    assert_eq!(
        shadow_caster(AlphaMode::Blend, true, false, false),
        Some(ShadowCaster::CutOut)
    );
}

#[test]
fn any_other_blended_layer_is_glass() {
    // a pane, a decal, dirt: no transmap, or a transmap on a layer that writes no depth; the
    // instance decides whether it is a pane (a road's own layers are not)
    assert_eq!(
        shadow_caster(AlphaMode::Blend, false, false, false),
        Some(ShadowCaster::Glass)
    );
    assert_eq!(
        shadow_caster(AlphaMode::Blend, false, true, false),
        Some(ShadowCaster::Glass)
    );
    assert_eq!(
        shadow_caster(AlphaMode::Blend, true, true, false),
        Some(ShadowCaster::Glass)
    );
}

#[test]
fn a_material_without_the_depth_check_casts_nothing() {
    for alpha in [AlphaMode::Opaque, AlphaMode::Test, AlphaMode::Blend] {
        for transmap in [false, true] {
            for no_z_write in [false, true] {
                assert_eq!(shadow_caster(alpha, transmap, no_z_write, true), None);
            }
        }
    }
}
