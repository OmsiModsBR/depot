use omsi_launcher_lib as core;
use omsi_launcher_lib::original_engine as oe;
use omsi_launcher_ui::state::State;

#[test]
fn a_duty_becomes_the_map_and_the_start_point() {
    let d = core::Duty {
        map: "maps/Grundorf M/global.cfg".into(),
        entry: Some(3),
        ..Default::default()
    };
    assert_eq!(
        State::world_of(&d),
        oe::launch::World::New {
            map: "maps/Grundorf M/global.cfg".into(),
            entrypoint: 3
        }
    );
    // no start point chosen (or a negative one) is the first
    let d = core::Duty {
        entry: Some(-1),
        ..Default::default()
    };
    assert!(matches!(
        State::world_of(&d),
        oe::launch::World::New { entrypoint: 0, .. }
    ));
}
