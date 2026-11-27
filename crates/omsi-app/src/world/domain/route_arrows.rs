//! OMSI 2's dynamic route arrows (the `nav_arrows` setting): the scenery object of each kind
//! of arrow and the state of the arrows that stand. OMSI puts the scenery objects
//! `Sceneryobjects\Generic\routearrow_{L,R,dn}_dyn.sco` over the junctions of the player's
//! route and `routearrows_busstop.sco` at its stops, each with a text on it. The navigator
//! says where the route goes (`Navigator::arrow_spots`); the placing into the world is in
//! `implementation/route_arrows.rs`.

use crate::world::scene::TileGpu;

#[derive(Default)]
pub(crate) struct RouteArrows {
    /// What stands now: its key and the object.
    pub(crate) placed: Vec<(u64, TileGpu)>,
    /// Seconds to the next look at the route.
    pub(crate) wait: f32,
}

pub(crate) fn sco(kind: &str) -> &'static str {
    match kind {
        "L" => "Sceneryobjects\\Generic\\routearrow_L_dyn.sco",
        "R" => "Sceneryobjects\\Generic\\routearrow_R_dyn.sco",
        "busstop" => "Sceneryobjects\\Generic\\routearrows_busstop.sco",
        _ => "Sceneryobjects\\Generic\\routearrow_dn_dyn.sco",
    }
}

impl RouteArrows {
    /// Whether any arrow stands.
    pub(crate) fn any(&self) -> bool {
        !self.placed.is_empty()
    }
}
