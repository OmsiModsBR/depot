use openomsi_game::testing::headtrack::*;

#[test]
fn head_tracking_listens_on_this_computer_only_unless_told_otherwise() {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    let local = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert_eq!(bind_addr(""), local);
    assert_eq!(bind_addr("  "), local);
    assert_eq!(bind_addr("any"), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_eq!(bind_addr("ALL"), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_eq!(
        bind_addr("192.168.1.20"),
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))
    );
    assert_eq!(bind_addr("::1"), IpAddr::V6(Ipv6Addr::LOCALHOST));
    // not an address: the safe default, never every interface
    assert_eq!(bind_addr("localhost; drop"), local);
    assert_eq!(bind_addr("0.0.0.0/0"), local);
}

#[test]
fn the_default_socket_does_not_take_datagrams_from_other_hosts() {
    // bound where the setting says by default: a loopback address
    let sock = std::net::UdpSocket::bind((bind_addr(""), 0)).unwrap();
    assert!(sock.local_addr().unwrap().ip().is_loopback());
}

#[test]
fn freetrack_pose_is_in_cm_and_degrees() {
    let p = freetrack_pose(
        -0.5f32.to_radians() * 60.0,
        10f32.to_radians(),
        5f32.to_radians(),
        15.0,
        -20.0,
        100.0,
    );
    assert!(
        (p.rot[0] - 30.0).abs() < 1e-3
            && (p.rot[1] + 10.0).abs() < 1e-3
            && (p.rot[2] - 5.0).abs() < 1e-3
    );
    assert_eq!(p.pos, [1.5, -2.0, 10.0]);
}

/// The head 10 cm to the right (opentrack's x -10), 5 cm up and 20 cm back: the eye goes
/// right, up and back with it (#1157).
#[test]
fn the_eye_follows_the_head_across() {
    let p = HeadPose {
        pos: [-10.0, 5.0, 20.0],
        rot: [0.0; 3],
    };
    let o = p.seat_offset();
    assert!(
        (o.x - 0.1).abs() < 1e-6 && (o.y + 0.2).abs() < 1e-6 && (o.z - 0.05).abs() < 1e-6,
        "{o}"
    );
    // (held within 60 cm)
    assert!(
        (HeadPose {
            pos: [-200.0, 0.0, 0.0],
            rot: [0.0; 3]
        }
        .seat_offset()
        .x - 0.6)
            .abs()
            < 1e-6
    );
}
