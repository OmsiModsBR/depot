//! The `achse_steer_angle` and `achse_steer_centring` parameters of a `[newachse]` (spec 004,
//! `contracts/bus-key.md`): both or none, anywhere among the axle's parameters, validated and
//! never corrected.

use omsi_cfg::CfgFile;
use omsi_vehicle::{AxleSteer, SteerParamError, Vehicle};

fn load(text: &str) -> Vehicle {
    Vehicle::parse(&CfgFile::from_str("test.bus", text))
}

const FRONT: &str = "[newachse]\nachse_long\n2.9\nachse_maxwidth\n2.5\n";

fn steered(params: &str) -> Vehicle {
    load(&format!(
        "{FRONT}[newachse]\nachse_long\n-4.0\n{params}[mass]\n12\n"
    ))
}

fn steer(lock_deg: f32, centring_deg_s: f32) -> Option<AxleSteer> {
    Some(AxleSteer {
        lock_deg,
        centring_deg_s,
    })
}

#[test]
fn both_parameters_make_the_axle_steerable() {
    let v = steered("achse_maxwidth\n2.5\nachse_steer_angle\n15\nachse_steer_centring\n20\n");
    assert_eq!(v.axles.len(), 2);
    assert_eq!(v.axles[0].steer, None);
    assert_eq!(v.axles[1].steer, steer(15.0, 20.0));
    assert_eq!(v.axles[1].long, -4.0);
    assert_eq!(v.axles[1].max_width, 2.5);
    assert_eq!(v.mass, 12.0);
}

#[test]
fn the_parameters_stand_anywhere_in_any_order() {
    let first = load(
        "[newachse]\nachse_steer_centring\n20\nachse_steer_angle\n15\nachse_long\n-4.0\nachse_feder\n300\n",
    );
    assert_eq!(first.axles[0].steer, steer(15.0, 20.0));
    assert_eq!(first.axles[0].long, -4.0);
    assert_eq!(first.axles[0].spring, 300.0);
    let split = load(
        "[newachse]\nachse_steer_angle\n15\nachse_long\n-4.0\nachse_steer_centring\n20\nachse_feder\n300\n",
    );
    assert_eq!(split.axles[0].steer, steer(15.0, 20.0));
    assert_eq!(split.axles[0].spring, 300.0);
}

#[test]
fn the_parameters_belong_to_their_own_axle() {
    let v = load(&format!(
        "{FRONT}achse_steer_angle\n10\nachse_steer_centring\n5\n[newachse]\nachse_long\n-4.0\n"
    ));
    assert_eq!(v.axles[0].steer, steer(10.0, 5.0));
    assert_eq!(v.axles[1].steer, None);
}

#[test]
fn a_partial_set_is_not_steerable_and_names_the_missing_one() {
    assert_eq!(steered("achse_steer_angle\n15\n").axles[1].steer, None);
    assert_eq!(steered("achse_steer_centring\n20\n").axles[1].steer, None);
    assert_eq!(
        AxleSteer::from_params(Some("15"), None),
        Err(SteerParamError::Missing("achse_steer_centring"))
    );
    assert_eq!(
        AxleSteer::from_params(None, Some("20")),
        Err(SteerParamError::Missing("achse_steer_angle"))
    );
    let msg = SteerParamError::Missing("achse_steer_centring").to_string();
    assert!(msg.contains("achse_steer_centring"), "{msg}");
}

#[test]
fn the_lock_must_be_above_0_and_below_90() {
    for bad in ["0", "-5", "90", "120"] {
        let v = steered(&format!(
            "achse_steer_angle\n{bad}\nachse_steer_centring\n20\n"
        ));
        assert_eq!(v.axles[1].steer, None, "{bad}");
        assert!(matches!(
            AxleSteer::from_params(Some(bad), Some("20")),
            Err(SteerParamError::OutOfRange("achse_steer_angle", _))
        ));
    }
    assert_eq!(
        AxleSteer::from_params(Some("89.5"), Some("20")),
        Ok(steer(89.5, 20.0))
    );
    assert_eq!(
        AxleSteer::from_params(Some("0.1"), Some("20")),
        Ok(steer(0.1, 20.0))
    );
}

#[test]
fn the_centring_rate_must_be_above_0() {
    for bad in ["0", "-1"] {
        assert!(matches!(
            AxleSteer::from_params(Some("15"), Some(bad)),
            Err(SteerParamError::OutOfRange("achse_steer_centring", _))
        ));
    }
    assert_eq!(
        AxleSteer::from_params(Some("15"), Some("1000")),
        Ok(steer(15.0, 1000.0))
    );
}

#[test]
fn a_value_that_is_not_a_number_is_named_not_read_as_0() {
    for bad in ["abc", "", "nan", "inf", "1e99"] {
        assert!(
            matches!(
                AxleSteer::from_params(Some(bad), Some("20")),
                Err(SteerParamError::NotANumber("achse_steer_angle", _))
            ),
            "{bad:?}"
        );
        assert!(matches!(
            AxleSteer::from_params(Some("15"), Some(bad)),
            Err(SteerParamError::NotANumber("achse_steer_centring", _))
        ));
    }
    assert_eq!(
        steered("achse_steer_angle\nabc\nachse_steer_centring\n20\n").axles[1].steer,
        None
    );
}

#[test]
fn a_decimal_comma_and_a_trailing_comment_read_as_elsewhere() {
    assert_eq!(
        AxleSteer::from_params(Some("12,5"), Some("20 deg/s")),
        Ok(steer(12.5, 20.0))
    );
}

#[test]
fn a_parameter_given_twice_keeps_the_first() {
    let v = steered(
        "achse_steer_angle\n15\nachse_steer_centring\n20\nachse_steer_angle\n30\nachse_long\n-5.0\n",
    );
    assert_eq!(v.axles[1].steer, steer(15.0, 20.0));
    // the second value line is the parameter's, not read as anything else
    assert_eq!(v.axles[1].long, -5.0);
}

#[test]
fn the_old_form_without_keywords_never_carries_them() {
    let v = load(
        "[newachse]\n-4.0\n2.5\n2.0\n1.0\n300\n100\n20\n1\n0\nachse_steer_angle\n15\nachse_steer_centring\n20\n[mass]\n12\n",
    );
    assert_eq!(v.axles.len(), 1);
    assert_eq!(v.axles[0].long, -4.0);
    assert!(v.axles[0].driven);
    assert_eq!(v.axles[0].steer, None);
    assert_eq!(v.mass, 12.0);
}

#[test]
fn a_file_without_them_parses_as_before() {
    let v = load(&format!(
        "{FRONT}[newachse]\nachse_long\n-4.0\n[mass]\n12\n"
    ));
    assert!(v.axles.iter().all(|a| a.steer.is_none()));
    assert_eq!(v.axles[0].max_width, 2.5);
    assert_eq!(v.mass, 12.0);
    assert!(v.unknown_keywords.is_empty());
}
