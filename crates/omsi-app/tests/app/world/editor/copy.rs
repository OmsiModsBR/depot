use openomsi_game::testing::editor::{add_copies, NewRecord};

#[test]
fn a_copy_follows_its_template_with_its_own_id() {
    let text = "[object]\r\n0\r\nSceneryobjects\\A\\post.sco\r\n17\r\n10\r\n20\r\n0\r\n90\r\n\r\n[object]\r\n0\r\nx.sco\r\n18\r\n1\r\n1\r\n0\r\n0\r\n";
    let (out, n) = add_copies(
        text,
        &[NewRecord {
            template: 17,
            id: 99,
            file: "lamp.sco".into(),
            moved: glam::DVec3::new(2.0, 0.0, 0.0),
            turned: 10.0,
        }],
    );
    assert_eq!(n, 1);
    assert!(
        out.contains("Sceneryobjects\\A\\lamp.sco\r\n99\r\n12\r\n20\r\n0\r\n100\r\n"),
        "{out}"
    );
    assert!(out.contains("[object]\r\n0\r\nx.sco\r\n18"));
}
