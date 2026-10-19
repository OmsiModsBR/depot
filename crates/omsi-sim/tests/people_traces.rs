//! Traces of the people's bones, the safety net of their refactor (Trilha S): the pose the
//! game draws a person in (`human::Pose`, standing, walking, running, turning, stopping,
//! sitting down and getting up, paying at the cash desk, climbing a step) and OMSI's own
//! animation of the passengers (`human_omsi::OmsiAnim`, its thirty angles and thirteen
//! bones), written out frame by frame.
//!
//! The same contract as `physics_traces.rs`: with `OMSI_PHYSICS_TRACE_DIR` set, each scenario
//! writes `<dir>/<name>.trace`, and `scripts/physics-trace-diff.sh` runs them on a base commit
//! and on the change and compares the two. A change that only moves code leaves every trace
//! as it was, to the bit. Without it, each scenario runs twice and both runs must write the
//! same trace. The file uses only the crate's public API: the script copies it as it is, with
//! `support/`, into the commit it compares with.
//!
//! The person is synthetic: a `.hum` file with the stock adults' joints and no mesh, written
//! into a folder of its own, so that the scenarios run without the game's content.

mod support;

use glam::{Affine3A, DVec2, DVec3, Vec3};
use omsi_sim::human::{Activity, HumanType, Pose, PoseInput, Posed};
use omsi_sim::human_omsi::{AnimInput, OmsiAnim};
use std::fmt::Write;
use support::scratch::ScratchPack;
use support::trace::{check, dv3, n32, v3, Trace};

// ---------------------------------------------------------------------------------------
// The person

/// The stock adults' `[links]`: the joints of hip, knee, waist, shoulder, elbow, wrist,
/// neck and head as OMSI's `.hum` files give them.
const LINKS: [f32; 22] = [
    0.09, 0.0, 0.92, 0.09, -0.03, 0.53, 0.02, 1.17, 0.18, -0.05, 1.43, 0.44, -0.04, 1.41, -0.02,
    1.55, 0.69, -0.03, 1.43, 0.9, -0.03, 1.43,
];

/// A person 1.77 m tall without a mesh, loaded as the game loads one.
fn person() -> HumanType {
    let pack = ScratchPack::new("people-traces");
    let links: Vec<String> = LINKS.iter().map(|x| x.to_string()).collect();
    pack.write("model.cfg", "");
    let hum = pack.write(
        "man.hum",
        format!(
            "[model]\nmodel.cfg\n\n[humangeom]\n0.04\n1.77\n\n[seatheight]\n0.83\n\n[links]\n{}\n\n[walk_param]\n1.4\n80\n1\n1\n0\n",
            links.join("\n")
        ),
    );
    HumanType::load(&hum).unwrap()
}

// ---------------------------------------------------------------------------------------
// Writing the state

fn affine(a: &Affine3A) -> String {
    format!(
        "{} | {} | {} | {}",
        v3(a.matrix3.x_axis.into()),
        v3(a.matrix3.y_axis.into()),
        v3(a.matrix3.z_axis.into()),
        v3(a.translation.into())
    )
}

fn pair(p: &[Vec3; 2]) -> String {
    format!("{} / {}", v3(p[0]), v3(p[1]))
}

fn pair32(p: &[f32; 2]) -> String {
    format!("{} {}", n32(p[0]), n32(p[1]))
}

/// The person's pose: the flags `Pose` tells, then every bone and joint of the frame.
fn pose(t: &mut Trace, p: &Pose, posed: &Posed) {
    let _ = writeln!(
        t.text,
        "  pose sit {} settling {} planted {} {} landed {} calm {} catch_ups {} ok {}",
        n32(p.sit_amount()),
        p.settling(),
        p.planted(0),
        p.planted(1),
        p.landed(),
        p.calm(),
        p.catch_ups(),
        posed.ok,
    );
    let _ = writeln!(
        t.text,
        "    hip {} knee {} ankle {} elbow {} wrist {} neck {}",
        pair(&posed.hip),
        pair(&posed.knee),
        pair(&posed.ankle),
        pair(&posed.elbow),
        pair(&posed.wrist),
        v3(posed.neck),
    );
    let _ = writeln!(
        t.text,
        "    sole {} knee_flex {} ankle_flex {} leg_miss {}",
        pair32(&posed.sole),
        pair32(&posed.knee_flex),
        pair32(&posed.ankle_flex),
        pair32(&posed.leg_miss),
    );
    for (i, b) in posed.bones.iter().enumerate() {
        let _ = writeln!(t.text, "    bone {i} {}", affine(b));
    }
}

/// OMSI's animation of a passenger: the phase, the thirty angles and the thirteen bones.
fn anim(t: &mut Trace, a: &OmsiAnim, bones: &[Affine3A], step: bool) {
    let angles: Vec<String> = a.angles.iter().map(|x| n32(*x)).collect();
    let _ = writeln!(
        t.text,
        "  anim phase {} step {step} angles {}",
        n32(a.phase),
        angles.join(" ")
    );
    for (i, b) in bones.iter().enumerate() {
        let _ = writeln!(t.text, "    bone {i} {}", affine(b));
    }
}

// ---------------------------------------------------------------------------------------
// Running the people

/// What the person does at a moment: the activity, the velocity (m/s, floor frame) and
/// heading, and a seat or a place to reach to.
#[derive(Clone, Copy)]
struct Doing {
    activity: Activity,
    velocity: DVec2,
    heading: f64,
    seat: Option<Vec3>,
    reach: Option<Vec3>,
}

impl Doing {
    fn stand(heading: f64) -> Doing {
        Doing {
            activity: Activity::Stand,
            velocity: DVec2::ZERO,
            heading,
            seat: None,
            reach: None,
        }
    }

    /// Walking towards `heading` (OMSI's: 0 = +y, clockwise) at `speed`.
    fn walk(speed: f64, heading: f64) -> Doing {
        let h = heading.to_radians();
        Doing {
            activity: Activity::Walk,
            velocity: DVec2::new(h.sin(), h.cos()) * speed,
            heading,
            seat: None,
            reach: None,
        }
    }
}

/// `secs` seconds at 30 frames a second (a frame in seven a third longer), doing what
/// `plan` says at each moment, on the floor `floor` gives.
fn walk_through(
    t: &mut Trace,
    plan: impl Fn(f32) -> Doing,
    secs: f32,
    floor: Option<&dyn Fn(DVec2) -> Option<f64>>,
) {
    let ty = person();
    let mut p = Pose::new(7);
    let mut origin = DVec3::ZERO;
    let (mut time, mut f) = (0.0f32, 0usize);
    while time < secs {
        let dt = if f % 7 == 3 { 0.04 } else { 0.03 };
        let doing = plan(time);
        origin += (doing.velocity * dt as f64).extend(0.0);
        if let Some(floor) = floor {
            origin.z = floor(origin.truncate()).unwrap_or(0.0);
        }
        let input = PoseInput {
            activity: doing.activity,
            origin,
            heading: doing.heading,
            velocity: doing.velocity,
            seat: doing.seat,
            reach: doing.reach,
            floor,
            ..Default::default()
        };
        t.frame(dt, &format!("{:?} origin {}", doing.activity, dv3(origin)));
        p.advance(&ty.rig, &input, dt);
        let posed = p.bones(&ty.rig);
        pose(t, &p, &posed);
        time += dt;
        f += 1;
    }
}

/// OMSI's animation over `secs` seconds at 30 frames a second, with the input `plan` gives.
fn animate(t: &mut Trace, plan: impl Fn(f32) -> AnimInput, secs: f32) {
    let ty = person();
    let mut a = OmsiAnim::default();
    let (mut time, mut f) = (0.0f32, 0usize);
    while time < secs {
        let dt = if f % 7 == 3 { 0.04 } else { 0.03 };
        let input = AnimInput {
            dt_ms: dt * 1000.0,
            ..plan(time)
        };
        t.frame(
            dt,
            &format!(
                "kind {} speed {} moved {}",
                input.kind,
                n32(input.speed),
                n32(input.moved)
            ),
        );
        let ev = a.advance(&ty.omsi, &input);
        let bones = a.bones(&ty.omsi);
        anim(t, &a, &bones, ev.step);
        time += dt;
        f += 1;
    }
}

/// OMSI's input for a person `kind` (0 stand, 1 walk, 2 sit) at `speed`, outside a vehicle.
fn omsi(kind: u8, speed: f32, smooth: bool) -> AnimInput {
    AnimInput {
        kind,
        speed,
        moved: speed / 30.0,
        room_height: 50.0,
        seat_height: 0.45,
        reach: None,
        look: None,
        smooth,
        dt_ms: 0.0,
    }
}

macro_rules! scenarios {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check(stringify!($name), scenarios::$name);
            }
        )*
    };
}

scenarios!(
    h01_the_person_as_loaded,
    p01_stands,
    p02_walks,
    p03_runs,
    p04_walks_turns_and_stops,
    p05_sits_down_and_gets_up,
    p06_pays_at_the_cash_desk,
    p07_walks_up_a_step,
    o01_walks,
    o02_runs_and_stops,
    o03_sits_down_and_stands_up,
    o04_reaches_and_looks,
);

mod scenarios {
    use super::*;

    pub fn h01_the_person_as_loaded() -> String {
        let ty = person();
        let r = &ty.rig;
        let mut t = Trace::default();
        t.line(&format!(
            "rig hip {} knee {} ankle {} shoulder {} elbow {} wrist {}",
            pair(&r.hip),
            pair(&r.knee),
            pair(&r.ankle),
            pair(&r.shoulder),
            pair(&r.elbow),
            pair(&r.wrist)
        ));
        t.line(&format!(
            "rig waist {} neck {} head {} pelvis {}",
            v3(r.waist),
            v3(r.neck),
            v3(r.head_pivot),
            v3(r.pelvis)
        ));
        t.line(&format!(
            "rig thigh {} shin {} upper_arm {} forearm {} sole {} ankle_h {} heel {} ball {} toe {} ball_h {} head_top {} seat_lift {} scale {} walk_step {} arm_swing {} hip_sway {}",
            n32(r.thigh),
            n32(r.shin),
            n32(r.upper_arm),
            n32(r.forearm),
            n32(r.sole),
            n32(r.ankle_h),
            n32(r.heel),
            n32(r.ball),
            n32(r.toe),
            n32(r.ball_h),
            n32(r.head_top),
            n32(r.seat_lift),
            n32(r.scale),
            n32(r.walk_step),
            n32(r.arm_swing),
            n32(r.hip_sway),
        ));
        t.line(&format!(
            "rig leg {} seat_front {}",
            n32(r.leg()),
            n32(r.seat_front())
        ));
        for v in [0.0, 0.6, 1.35, 2.2, 3.5, 5.0] {
            t.line(&format!(
                "cadence {} {} run {}",
                n32(v),
                n32(r.cadence(v)),
                n32(omsi_sim::human::run_factor(v))
            ));
        }
        let rest = OmsiAnim::default();
        anim(&mut t, &rest, &rest.bones(&ty.omsi), false);
        t.text
    }

    pub fn p01_stands() -> String {
        let mut t = Trace::default();
        walk_through(&mut t, |_| Doing::stand(30.0), 3.0, None);
        t.text
    }

    pub fn p02_walks() -> String {
        let mut t = Trace::default();
        walk_through(&mut t, |_| Doing::walk(1.35, 0.0), 6.0, None);
        t.text
    }

    pub fn p03_runs() -> String {
        let mut t = Trace::default();
        walk_through(
            &mut t,
            |time| Doing::walk((time as f64 * 1.5).min(3.5), 0.0),
            6.0,
            None,
        );
        t.text
    }

    /// Off at walking pace, a turn of 90° to the right on the way, and a stop.
    pub fn p04_walks_turns_and_stops() -> String {
        let mut t = Trace::default();
        walk_through(
            &mut t,
            |time| match time {
                x if x < 2.5 => Doing::walk(1.35, 0.0),
                x if x < 4.0 => Doing::walk(1.2, ((x - 2.5) as f64 * 60.0).min(90.0)),
                x if x < 5.5 => Doing::walk(1.35, 90.0),
                _ => Doing::stand(90.0),
            },
            8.0,
            None,
        );
        t.text
    }

    /// The person sits down on a seat in front of them, stays, and gets up again.
    pub fn p05_sits_down_and_gets_up() -> String {
        let mut t = Trace::default();
        let seat = Vec3::new(0.0, -0.1, 0.45);
        walk_through(
            &mut t,
            |time| {
                if (0.5..4.5).contains(&time) {
                    Doing {
                        activity: Activity::Sit,
                        seat: Some(seat),
                        ..Doing::stand(0.0)
                    }
                } else {
                    Doing::stand(0.0)
                }
            },
            7.0,
            None,
        );
        t.text
    }

    /// Standing at the cash desk with the right hand held out to it.
    pub fn p06_pays_at_the_cash_desk() -> String {
        let mut t = Trace::default();
        walk_through(
            &mut t,
            |time| Doing {
                activity: if time < 3.0 {
                    Activity::Pay
                } else {
                    Activity::Stand
                },
                reach: Some(Vec3::new(0.3, 0.45, 0.95)),
                ..Doing::stand(0.0)
            },
            4.5,
            None,
        );
        t.text
    }

    /// Walking onto a kerb of 15 cm 2 m ahead.
    pub fn p07_walks_up_a_step() -> String {
        let mut t = Trace::default();
        let kerb = |p: DVec2| Some(if p.y >= 2.0 { 0.15 } else { 0.0 });
        walk_through(&mut t, |_| Doing::walk(1.2, 0.0), 4.0, Some(&kerb));
        t.text
    }

    pub fn o01_walks() -> String {
        let mut t = Trace::default();
        animate(&mut t, |_| omsi(1, 1.35, false), 4.0);
        t.text
    }

    pub fn o02_runs_and_stops() -> String {
        let mut t = Trace::default();
        animate(
            &mut t,
            |time| match time {
                x if x < 2.0 => omsi(1, 3.2, false),
                x if x < 3.0 => omsi(1, 3.2 * (3.0 - x), false),
                _ => omsi(0, 0.0, false),
            },
            4.5,
        );
        t.text
    }

    pub fn o03_sits_down_and_stands_up() -> String {
        let mut t = Trace::default();
        animate(
            &mut t,
            |time| {
                if (0.5..3.0).contains(&time) {
                    omsi(2, 0.0, true)
                } else {
                    omsi(0, 0.0, true)
                }
            },
            5.0,
        );
        t.text
    }

    /// Standing at the validator: the right hand reaches out and the head looks at it.
    pub fn o04_reaches_and_looks() -> String {
        let mut t = Trace::default();
        animate(
            &mut t,
            |time| AnimInput {
                reach: (time < 2.0).then_some(Vec3::new(0.35, 1.0, 0.4)),
                look: Some(Vec3::new(0.35, 1.0, 0.4)),
                ..omsi(0, 0.0, true)
            },
            3.0,
        );
        t.text
    }
}
