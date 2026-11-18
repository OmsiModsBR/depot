//! A `[matl_bumpmap]` on an envmapped material in the Enhanced picture does not turn the
//! surface rough. (The specular antialiasing read the bump map's noise as curvature, and the
//! envmap of a bump-mapped body all but vanished.) And a weak `[matl_envmap]` (a bus body's
//! 0.1) is a smooth coat that mirrors faintly, not a rough one whose reflection is a flat
//! sheen.

mod common;

use common::bump_board::{board, centre, debug_renderer};
use omsi_render::Renderer;

/// The roughness the Enhanced pass gives the board (`OMSI_DEBUG_ENHANCED=17` shows it in
/// the red channel), mean over its centre.
fn roughness(renderer: &mut Renderer, factor: f32, bump: bool) -> f32 {
    let reds = centre(&board(renderer, factor, bump), 0);
    reds.iter().sum::<f32>() / reds.len() as f32
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_bump_map_keeps_an_envmapped_surface_smooth() {
    let mut renderer = debug_renderer("17");
    let smooth = roughness(&mut renderer, 1.0, false);
    let bumped = roughness(&mut renderer, 1.0, true);
    eprintln!("roughness on the screen: {smooth} without the bump map, {bumped} with it");
    assert!(
        smooth < 120.0,
        "the envmapped board is not smooth to begin with: {smooth}"
    );
    assert!(
        (bumped - smooth).abs() < 12.0,
        "the bump map makes the surface rough: {smooth} without, {bumped} with"
    );
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn a_weak_envmap_keeps_the_paint_smooth() {
    let mut renderer = debug_renderer("17");
    // (a bus body's `[matl_envmap] envmap2.bmp 0.1`, with neither mask nor transmap)
    let weak = roughness(&mut renderer, 0.1, false);
    let bumped = roughness(&mut renderer, 0.1, true);
    eprintln!("roughness on the screen at a factor of 0.1: {weak}, {bumped} with a bump map");
    assert!(weak < 120.0, "a weak envmap makes the paint rough: {weak}");
    assert!(
        bumped < 120.0,
        "a weak envmap with a bump map is rough: {bumped}"
    );
}
