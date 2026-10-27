use openomsi_game::testing::player::door_action;

/// The door actions that work on every bus (#916).
#[test]
fn door_actions_name_a_door_or_all_of_them() {
    assert_eq!(door_action("door_1"), Some(1));
    assert_eq!(door_action("Door_3"), Some(3));
    assert_eq!(door_action("doors_all"), Some(0));
    assert_eq!(door_action("door_0"), None);
    assert_eq!(door_action("bus_doorfront0"), None);
    assert_eq!(door_action("door_x"), None);
}
