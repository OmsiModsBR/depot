use omsi_sim::EngineAction as A;
use openomsi_game::testing::player::fallback_action;
use winit::keyboard::KeyCode;

#[test]
fn plain_left_right_steer_only_with_the_arrows_preset() {
    assert_eq!(fallback_action(KeyCode::ArrowLeft, "simple"), None);
    assert_eq!(fallback_action(KeyCode::ArrowRight, "simple"), None);
    assert_eq!(
        fallback_action(KeyCode::ArrowUp, "simple"),
        Some(A::Throttle)
    );
    assert_eq!(
        fallback_action(KeyCode::KeyA, "simple"),
        Some(A::SteeringLeft)
    );
    assert_eq!(
        fallback_action(KeyCode::ArrowLeft, "arrows"),
        Some(A::SteeringLeft)
    );
    assert_eq!(
        fallback_action(KeyCode::ArrowRight, "arrows"),
        Some(A::SteeringRight)
    );
}
