mod copy;

use glam::DVec3;
use hashbrown::HashMap;
use openomsi_game::testing::scene::ObjectEdit;

#[test]
fn an_unreadable_ground_is_an_error_and_a_missing_one_is_flat() {
    let dir = std::env::temp_dir().join(format!("depot-editor-ground-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.map.terrain");
    std::fs::write(&bad, [60u8, 0, 0, 0, 1, 2]).unwrap();
    assert!(testing::editor::load_ground(&bad).is_err());
    let flat = testing::editor::load_ground(&dir.join("none.map.terrain")).unwrap();
    assert_eq!(flat, omsi_map::Terrain::flat());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn writes_are_atomic_and_keep_a_backup() {
    let dir = std::env::temp_dir().join(format!("depot-editor-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("t.map.terrain");
    testing::editor::write_with_backup(&f, b"one").unwrap();
    assert!(
        !dir.join("t.map.terrain.bak").exists(),
        "nothing to back up the first time"
    );
    testing::editor::write_with_backup(&f, b"two").unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"two");
    assert_eq!(
        std::fs::read(dir.join("t.map.terrain.bak")).unwrap(),
        b"one"
    );
    assert!(!dir.join("t.map.terrain.tmp").exists());
    std::fs::remove_dir_all(&dir).ok();
}

use openomsi_game::testing;
use openomsi_game::testing::editor::*;

#[test]
fn the_brush_raises_the_middle_most_and_not_its_rim() {
    let mut t = omsi_map::Terrain::flat();
    let size = omsi_map::tile_size();
    let at = DVec3::new(size * 0.5, size * 0.5, 0.0);
    assert!(shape(
        &mut t,
        (0.0, 0.0),
        at,
        12.0,
        &Action::Ground(1.0),
        0.0
    ));
    assert!((t.sample(at.x as f32, at.y as f32) - 1.0).abs() < 1e-4);
    assert_eq!(t.sample(at.x as f32 + 18.0, at.y as f32), 0.0);
    // flattened back to 0 where the weight is full
    shape(&mut t, (0.0, 0.0), at, 12.0, &Action::Flatten, 0.0);
    assert!(t.sample(at.x as f32, at.y as f32).abs() < 1e-4);
    assert_eq!(omsi_map::Terrain::parse(&t.to_bytes()).unwrap(), t);
}

const TILE: &str = "[version]\r\n4\r\n\r\n[object]\r\n0\r\nSceneryobjects\\a.sco\r\n7\r\n5\r\n6\r\n0.25\r\n90\r\n0\r\n0\r\n0\r\n\r\nObject Nr. 1\r\n[object]\r\n0\r\nSceneryobjects\\b.sco\r\n8\r\n1\r\n2\r\n0\r\n0\r\n0\r\n0\r\n2\r\nHalt\r\nx\r\n\r\n[spline]\r\n0\r\n";

#[test]
fn a_moved_object_changes_its_lines_and_a_deleted_one_goes() {
    let mut edits = HashMap::new();
    edits.insert(
        7,
        ObjectEdit {
            moved: DVec3::new(1.5, -1.0, 0.0),
            turned: 12.5,
            deleted: false,
        },
    );
    edits.insert(
        8,
        ObjectEdit {
            deleted: true,
            ..Default::default()
        },
    );
    let (out, n) = rewrite_tile(TILE, &edits);
    assert_eq!(n, 2);
    assert!(
        out.contains(
            "[object]\r\n0\r\nSceneryobjects\\a.sco\r\n7\r\n6.5\r\n5\r\n0.25\r\n102.5\r\n0\r\n"
        ),
        "{out:?}"
    );
    assert!(!out.contains("b.sco") && !out.contains("Halt"), "{out:?}");
    assert!(out.ends_with("[spline]\r\n0\r\n"));
    // nothing to do: the same text
    assert_eq!(rewrite_tile(TILE, &HashMap::new()), (TILE.to_string(), 0));
}

#[test]
fn a_utf16_tile_is_written_back_as_utf16() {
    let enc = omsi_cfg::TextEncoding::Utf16Le;
    let bytes = omsi_cfg::encode_like(TILE, enc);
    assert_eq!(&bytes[..4], &[0xFF, 0xFE, b'[', 0]);
    let (text, read) = omsi_cfg::decode_with_encoding(&bytes);
    assert_eq!((text.as_str(), read), (TILE, enc));
}
