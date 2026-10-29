use clap::Parser;
use glam::DVec3;
use omsi_render::Camera;
use openomsi_game::testing::close_app;
use openomsi_game::testing::core::app::start_centers;
use openomsi_game::testing::core::cli::Args;

#[test]
fn an_app_whose_letting_go_panics_closes_without_a_crash() {
    struct HungQueue;
    impl Drop for HungQueue {
        fn drop(&mut self) {
            panic!("We timed out while waiting on the last successful submission to complete!");
        }
    }
    assert!(!close_app(HungQueue));
    assert!(!omsi_render::catching());
    assert!(close_app(vec![1, 2, 3]));
}

#[test]
fn streaming_starts_around_the_bus() {
    let cam = Camera {
        position: DVec3::new(5000.0, 6000.0, 50.0),
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.5,
        far: 100.0,
    };
    let args = |extra: &[&str]| Args::parse_from(["omsi"].iter().chain(extra.iter()).copied());
    // the camera alone
    assert_eq!(start_centers(&args(&[]), &cam, None), vec![cam.position]);
    // a bus viewed from its seat: only where it stands, even with a camera given (the
    // view follows the bus)
    let seat = vec![DVec3::new(10.0, 20.0, 0.0)];
    assert_eq!(
        start_centers(
            &args(&["--bus", "Vehicles/x.bus", "--spawn", "10,20,90"]),
            &cam,
            None
        ),
        seat
    );
    assert_eq!(
        start_centers(
            &args(&[
                "--bus",
                "Vehicles/x.bus",
                "--spawn",
                "10,20,90",
                "--cam",
                "5000,6000,50,0,0"
            ]),
            &cam,
            None
        ),
        seat
    );
    // ... and with the free camera: both places
    let both = start_centers(
        &args(&[
            "--bus",
            "Vehicles/x.bus",
            "--spawn",
            "10,20,90",
            "--cam",
            "5000,6000,50,0,0",
            "--view",
            "free",
        ]),
        &cam,
        None,
    );
    assert_eq!(both, vec![DVec3::new(10.0, 20.0, 0.0), cam.position]);
}
