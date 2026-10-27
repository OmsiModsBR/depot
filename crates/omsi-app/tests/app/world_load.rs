use clap::Parser;
use openomsi_game::testing::cli::Args;
use openomsi_game::testing::scene::World;
use openomsi_game::testing::world_load::*;

/// On foot without a bus (the launcher's "Without bus"), the player starts at the entry
/// point chosen, not under the map's editor camera; with neither a bus nor on foot there is
/// no spawn point.
#[test]
fn on_foot_without_a_bus_starts_at_the_entry_point() {
    let dir = std::env::temp_dir().join(format!("depot-spawn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("maps/Spawn")).unwrap();
    // (two entry points with an object id no object has: their recorded places count)
    let entry = |i: &str, x: &str, y: &str, name: &str| {
        [i, "99", "0", x, "0", y, "0", "0", "0", "1", "0", name].join("\n") + "\n"
    };
    std::fs::write(
        dir.join("maps/Spawn/global.cfg"),
        format!(
            "[name]\nSpawn\n\n[entrypoints]\n2\n{}{}[map]\n0\n0\ntile_0_0.map\n",
            entry("0", "10", "20", "First"),
            entry("1", "148.25", "70", "Second")
        ),
    )
    .unwrap();
    std::fs::write(dir.join("maps/Spawn/tile_0_0.map"), "[version]\n14\n").unwrap();
    let world = World::open(&dir, &dir.join("maps/Spawn/global.cfg"), 20261021).unwrap();
    let mut args = Args::parse_from(["depot"]);
    args.entry = 1;
    assert_eq!(spawn_point(&args, Some(&world)), None);
    args.on_foot = true;
    let p = spawn_point(&args, Some(&world)).expect("the entry point's place");
    assert!(
        (p.x - 148.25).abs() < 1e-6 && (p.y - 70.0).abs() < 1e-6,
        "{p:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
