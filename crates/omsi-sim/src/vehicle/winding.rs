//! How a vehicle pack's exporter wound its triangles, judged pack by pack from its meshes.

use std::path::Path;

// A vehicle pack may contain many tiny trim meshes and only a few large body/interior
// meshes. When the mesh vote is close, use capped triangle counts as a tie-breaker instead
// of letting every tiny mesh carry the same weight.
pub const WINDING_EVIDENCE_CAP: usize = 4096;

pub fn keep_authored_winding(
    forward_meshes: usize,
    backward_meshes: usize,
    forward_weight: usize,
    backward_weight: usize,
) -> bool {
    if forward_meshes > backward_meshes {
        return true;
    }
    if forward_meshes == 0 || backward_meshes == 0 {
        return false;
    }
    let counts_close = forward_meshes.saturating_mul(5) >= backward_meshes.saturating_mul(4);
    let forward_clearly_heavier =
        forward_weight.saturating_mul(10) >= backward_weight.saturating_mul(11);
    counts_close && forward_clearly_heavier
}

#[derive(Default, Debug)]
pub struct WindingVotes {
    forward: usize,
    backward: usize,
    pub forward_weight: usize,
    backward_weight: usize,
}

impl WindingVotes {
    pub fn record(&mut self, forward: bool, triangles: usize) {
        let weight = triangles.min(WINDING_EVIDENCE_CAP);
        if forward {
            self.forward += 1;
            self.forward_weight += weight;
        } else {
            self.backward += 1;
            self.backward_weight += weight;
        }
    }

    pub fn keep_authored(&self) -> bool {
        keep_authored_winding(
            self.forward,
            self.backward,
            self.forward_weight,
            self.backward_weight,
        )
    }
}

/// The vehicle pack a mesh file belongs to (the folder under `Vehicles`, lower case), the
/// unit its exporter's winding is judged by; a file elsewhere is judged with its folder.
pub fn winding_pack(p: &Path) -> String {
    // (a part borrowed as `..\..\Other\model\x.o3d`: the folders it climbs out of are not its own)
    let mut comps: Vec<String> = Vec::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                comps.pop();
            }
            std::path::Component::CurDir => {}
            c => comps.push(c.as_os_str().to_string_lossy().to_ascii_lowercase()),
        }
    }
    match comps.iter().rposition(|c| c == "vehicles") {
        Some(i) if i + 2 < comps.len() => comps[i + 1].clone(),
        _ => comps[..comps.len().saturating_sub(1)].join("/"),
    }
}
