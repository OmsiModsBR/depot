use openomsi_game::testing::admin::notice_targets;

#[test]
fn all_is_every_player_and_the_host_itself() {
    // the host is player 1, the others 2 and 5
    assert_eq!(
        notice_targets("all", [2, 5].into_iter(), 1),
        (vec![2, 5], true)
    );
    assert_eq!(notice_targets("all", [].into_iter(), 1), (vec![], true));
    // one player; the host's own number: its own screen, nothing sent
    assert_eq!(notice_targets("5", [2, 5].into_iter(), 1), (vec![5], false));
    assert_eq!(notice_targets("1", [2, 5].into_iter(), 1), (vec![], true));
    // not a number: nobody
    assert_eq!(notice_targets("x", [2, 5].into_iter(), 1), (vec![], false));
}
