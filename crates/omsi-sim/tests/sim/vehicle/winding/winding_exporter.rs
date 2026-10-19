use omsi_sim::testing::vehicle::winding::{
    keep_authored_winding, winding_pack, WindingVotes, WINDING_EVIDENCE_CAP,
};
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
