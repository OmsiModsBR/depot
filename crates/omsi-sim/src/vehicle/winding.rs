//! How a vehicle pack's exporter wound its triangles, judged pack by pack from its meshes.

use std::path::Path;

// A vehicle pack may contain many tiny trim meshes and only a few large body/interior
// meshes. When the mesh vote is close, use capped triangle counts as a tie-breaker instead
// of letting every tiny mesh carry the same weight.
const WINDING_EVIDENCE_CAP: usize = 4096;

fn keep_authored_winding(
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
pub(super) struct WindingVotes {
    forward: usize,
    backward: usize,
    forward_weight: usize,
    backward_weight: usize,
}

impl WindingVotes {
    pub(super) fn record(&mut self, forward: bool, triangles: usize) {
        let weight = triangles.min(WINDING_EVIDENCE_CAP);
        if forward {
            self.forward += 1;
            self.forward_weight += weight;
        } else {
            self.backward += 1;
            self.backward_weight += weight;
        }
    }

    pub(super) fn keep_authored(&self) -> bool {
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
pub(super) fn winding_pack(p: &Path) -> String {
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

#[cfg(test)]
mod winding_exporter_tests {
    use super::{keep_authored_winding, winding_pack, WindingVotes, WINDING_EVIDENCE_CAP};
    use std::path::Path;

    #[test]
    fn borrowed_pack_evidence_stays_separate_and_capped() {
        let body = winding_pack(Path::new("/omsi/Vehicles/Bus/model/body.o3d"));
        let borrowed = winding_pack(Path::new(
            "/omsi/Vehicles/Bus/model/../../Display/model/display.o3d",
        ));
        let mut votes: std::collections::HashMap<String, WindingVotes> =
            std::collections::HashMap::new();
        for _ in 0..4 {
            votes
                .entry(body.clone())
                .or_default()
                .record(true, usize::MAX);
        }
        for _ in 0..5 {
            votes.entry(body.clone()).or_default().record(false, 2);
        }
        votes
            .entry(borrowed.clone())
            .or_default()
            .record(false, 100);
        assert!(votes[&body].keep_authored());
        assert_eq!(votes[&body].forward_weight, 4 * WINDING_EVIDENCE_CAP);
        assert!(!votes[&borrowed].keep_authored());
        let local_display = winding_pack(Path::new("/omsi/Vehicles/Display/model/display.o3d"));
        assert_eq!(borrowed, local_display);
    }

    #[test]
    fn empty_or_inconclusive_votes_keep_default() {
        assert!(!WindingVotes::default().keep_authored());
        assert!(!keep_authored_winding(0, 5, 0, 10));
        assert!(!keep_authored_winding(5, 5, 10, 10));
    }

    #[test]
    fn close_mesh_count_can_use_triangle_evidence() {
        assert!(keep_authored_winding(64, 74, 91_597, 79_544));
    }

    #[test]
    fn clear_backward_majority_is_not_overridden() {
        assert!(!keep_authored_winding(40, 80, 120_000, 60_000));
    }

    #[test]
    fn forward_majority_keeps_existing_behaviour() {
        assert!(keep_authored_winding(80, 40, 1, 1));
    }
}
