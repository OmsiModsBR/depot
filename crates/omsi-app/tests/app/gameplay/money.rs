use glam::Vec3;
use hashbrown::HashMap;
use openomsi_game::testing::money::*;
use std::path::PathBuf;

#[test]
fn coins_lie_flat_on_the_point_not_stacked() {
    let point = Vec3::new(0.4, 5.0, 1.1);
    let mut rng = Money {
        currency: None,
        dir: PathBuf::new(),
        meshes: HashMap::new(),
        placed: Vec::new(),
        hidden: Vec::new(),
        rng: 7,
    };
    let mut top = f32::MIN;
    for _ in 0..80 {
        let r = [rng.rand_f(), rng.rand_f(), rng.rand_f()];
        let m = Money::coin_place(point, [0.1, 0.06], r);
        let p = m.transform_point3(Vec3::ZERO);
        assert!((p.x - point.x).abs() <= 0.05 + 1e-6 && (p.y - point.y).abs() <= 0.03 + 1e-6);
        top = top.max(p.z);
    }
    // 80 coins: all at the point's height (they were 80 * 3 mm = 24 cm high before)
    assert!((top - point.z).abs() < 1e-6, "{top}");
}

#[test]
fn leaving_the_bus_takes_payment_and_change_off_the_desk() {
    let mut m = Money {
        currency: None,
        dir: PathBuf::new(),
        meshes: HashMap::new(),
        // two coins paid, one handed out as change
        placed: vec![
            (10, glam::Mat4::IDENTITY, 0, false, None),
            (11, glam::Mat4::IDENTITY, 1, false, None),
            (12, glam::Mat4::IDENTITY, 2, true, Some("tray".into())),
        ],
        hidden: Vec::new(),
        rng: 1,
    };
    m.clear_all();
    // nothing left to draw in the next bus's frame; all three are hidden on the next sync
    assert!(m.placed.is_empty());
    let mut hidden = m.hidden.clone();
    hidden.sort();
    assert_eq!(hidden, vec![10, 11, 12]);
    assert_eq!(m.change_value(), 0.0);
}
