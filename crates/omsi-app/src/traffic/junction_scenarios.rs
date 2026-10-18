//! Deterministic junction scenarios for the AI traffic, on a synthetic network the tests
//! build themselves: nothing from a map, no graphics adapter, no content of any game.
//!
//! `Crossroads` lays out two straight streets crossing at the origin, one lane each way, each
//! lane in three pieces like a stock junction: an approach, a short junction path of its own
//! object (the lanes that conflict) and an exit, followed by a stretch beyond. Lights can be
//! put on the junction paths (the stop line of a signalled junction) or on the stretch beyond
//! the exit (a second line downstream, to stand a queue in the exit). `World` holds a real
//! `Traffic` over that network with cars made from a vehicle type that only declares the
//! variables, and steps it with `Traffic::tick`, the same frame the game runs.

use super::*;
use omsi_sim::traffic::{Lane, LaneBuilder, LaneKey};

/// The frame the scenarios step by (s): 30 frames a second.
pub(super) const DT: f32 = 1.0 / 30.0;

/// Compass directions of travel, clockwise from north: the order of the arrays in `Crossroads`.
pub(super) const NORTH: usize = 0;
pub(super) const EAST: usize = 1;
pub(super) const SOUTH: usize = 2;
pub(super) const WEST: usize = 3;

/// Half the width of the junction object (m): the junction paths run from -HALF to +HALF.
const HALF: f64 = 6.0;
/// Offset of a lane from its street's middle (m), to the right of the way it goes.
const LANE_OFFSET: f64 = 1.75;

/// The network of the scenarios and where its lanes are.
pub(super) struct Crossroads {
    pub net: Network,
    /// Per direction of travel: the approach, the junction path, the exit and the stretch
    /// beyond it.
    pub approach: [usize; 4],
    pub junction: [usize; 4],
    pub exit: [usize; 4],
    pub beyond: [usize; 4],
    /// Per direction of travel: the junction path that turns left / right out of its approach
    /// (empty without `Layout::turns`), and the direction it ends up going.
    pub left: [Option<usize>; 4],
    pub right: [Option<usize>; 4],
}

/// How the crossroads is built.
#[derive(Clone, Copy)]
pub(super) struct Layout {
    /// Length of the exit piece between the junction object and the stretch beyond (m).
    pub exit_len: f64,
    /// A light (controller, lamp) per direction on its junction path: the stop line at the
    /// junction.
    pub junction_light: [Option<(usize, usize)>; 4],
    /// A light per direction at the start of the stretch beyond the exit: the line downstream.
    pub beyond_light: [Option<(usize, usize)>; 4],
    /// Junction paths that turn left and right too.
    pub turns: bool,
}

impl Default for Layout {
    fn default() -> Layout {
        Layout {
            exit_len: 30.0,
            junction_light: [None; 4],
            beyond_light: [None; 4],
            turns: false,
        }
    }
}

/// A straight lane from `from` to `to` (z 0).
fn straight(from: (f64, f64), to: (f64, f64)) -> Lane {
    let mut lane = LaneBuilder::polyline(
        vec![DVec3::new(from.0, from.1, 0.0), DVec3::new(to.0, to.1, 0.0)],
        LaneKind::Street,
        3.0,
    );
    lane.speed_limit_kmh = 50.0;
    lane
}

impl Crossroads {
    pub fn new(layout: Layout) -> Crossroads {
        let mut lanes: Vec<Lane> = Vec::new();
        let (mut approach, mut junction, mut exit, mut beyond) = ([0; 4], [0; 4], [0; 4], [0; 4]);
        for d in 0..4 {
            let h = (d as f64 * 90.0).to_radians();
            // the way it goes and the right-hand side of it
            let (u, r) = ((h.sin(), h.cos()), (h.cos(), -h.sin()));
            let at = |along: f64| {
                (
                    u.0 * along + r.0 * LANE_OFFSET,
                    u.1 * along + r.1 * LANE_OFFSET,
                )
            };
            let id = d as i64;
            let key = |obj: i64, path: u16| {
                Some(LaneKey {
                    tile: (0, 0),
                    id: obj,
                    path,
                })
            };
            let mut a = straight(at(-HALF - 100.0), at(-HALF));
            a.key = key(100 + id, 0);
            a.source = 1;
            let mut j = straight(at(-HALF), at(HALF));
            // (both paths of the crossing are one object: its lanes are the ones that conflict)
            j.key = key(1, d as u16);
            j.source = 2;
            j.traffic_light = layout.junction_light[d];
            let mut e = straight(at(HALF), at(HALF + layout.exit_len));
            e.key = key(200 + id, 0);
            e.source = 1;
            let mut b = straight(
                at(HALF + layout.exit_len),
                at(HALF + layout.exit_len + 200.0),
            );
            b.key = key(300 + id, 0);
            b.source = 1;
            b.traffic_light = layout.beyond_light[d];
            approach[d] = lanes.len();
            junction[d] = lanes.len() + 1;
            exit[d] = lanes.len() + 2;
            beyond[d] = lanes.len() + 3;
            lanes.extend([a, j, e, b]);
        }
        let (mut left, mut right) = ([None; 4], [None; 4]);
        if layout.turns {
            let pos = |d: usize, along: f64| {
                let h = (d as f64 * 90.0).to_radians();
                let (u, r) = ((h.sin(), h.cos()), (h.cos(), -h.sin()));
                (
                    u.0 * along + r.0 * LANE_OFFSET,
                    u.1 * along + r.1 * LANE_OFFSET,
                )
            };
            for d in 0..4 {
                for (turn, to) in [(1, (d + 3) % 4), (2, (d + 1) % 4)] {
                    let (p0, p1) = (pos(d, -HALF), pos(to, HALF));
                    // the control point: where the two lanes' middle lines cross
                    let (h0, h1) = (
                        (d as f64 * 90.0).to_radians(),
                        (to as f64 * 90.0).to_radians(),
                    );
                    let (u0, u1) = ((h0.sin(), h0.cos()), (h1.sin(), h1.cos()));
                    let den = u0.0 * u1.1 - u0.1 * u1.0;
                    let t = ((p1.0 - p0.0) * u1.1 - (p1.1 - p0.1) * u1.0) / den;
                    let c = (p0.0 + u0.0 * t, p0.1 + u0.1 * t);
                    let points = (0..=10)
                        .map(|k| {
                            let f = k as f64 / 10.0;
                            let (a, b, e) = ((1.0 - f) * (1.0 - f), 2.0 * f * (1.0 - f), f * f);
                            DVec3::new(
                                a * p0.0 + b * c.0 + e * p1.0,
                                a * p0.1 + b * c.1 + e * p1.1,
                                0.0,
                            )
                        })
                        .collect();
                    let mut lane = LaneBuilder::polyline(points, LaneKind::Street, 3.0);
                    lane.speed_limit_kmh = 50.0;
                    lane.key = Some(LaneKey {
                        tile: (0, 0),
                        id: 1,
                        path: 4 + 2 * d as u16 + (turn as u16 - 1),
                    });
                    lane.source = 2;
                    lane.turn = turn;
                    lane.traffic_light = layout.junction_light[d];
                    let i = lanes.len();
                    lanes.push(lane);
                    if turn == 1 {
                        left[d] = Some(i)
                    } else {
                        right[d] = Some(i)
                    }
                    let _ = to;
                }
            }
        }
        let mut net = Network {
            lanes,
            ..Default::default()
        };
        net.link(1.5);
        Crossroads {
            net,
            approach,
            junction,
            exit,
            beyond,
            left,
            right,
        }
    }
}

/// A vehicle type with a script set that only declares `TrafficPriority`, as the stock
/// emergency vehicles' scripts do; no model, nothing to draw.
fn car_type() -> Arc<VehicleType> {
    static TYPE: std::sync::OnceLock<Arc<VehicleType>> = std::sync::OnceLock::new();
    TYPE.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("omsi-junction-scenarios-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("test.bus"),
            "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
        )
        .unwrap();
        std::fs::write(dir.join("model.cfg"), "").unwrap();
        std::fs::write(dir.join("vars.txt"), "TrafficPriority\n").unwrap();
        std::fs::write(dir.join("main.osc"), "{init}\n{end}\n").unwrap();
        Arc::new(VehicleType::load(&dir, &dir.join("test.bus")).unwrap())
    })
    .clone()
}

fn no_render() -> VehicleRender {
    VehicleRender {
        instances: Vec::new(),
        own_materials: Vec::new(),
        set: None,
        variants: Vec::new(),
        text_textures: Vec::new(),
        script_textures: Vec::new(),
        shared_script: false,
        displays_far: false,
        display_tick: 0,
        skinned: Vec::new(),
        hidden: false,
        interior_lamps: std::cell::Cell::new(None),
        interior_blocks: std::cell::OnceCell::new(),
    }
}

/// A `Traffic` over a synthetic network, with no tiles, no map and no cars of its own.
fn traffic_over(net: Network, lights: Vec<TrafficLightController>) -> Traffic {
    let light_prev = lights.iter().map(|c| vec![-100; c.lights.len()]).collect();
    Traffic {
        net,
        street_weight: 0.0,
        parked: HashMap::new(),
        parked_waiting: Vec::new(),
        lane_tiles: Default::default(),
        lanes_generation: 0,
        types: Vec::new(),
        groups: Vec::new(),
        group_curves: false,
        group_uvg: Vec::new(),
        uvg_defaults: Arc::new(Vec::new()),
        cars: Vec::new(),
        dormant: Vec::new(),
        dormant_time: 0.0,
        rng: 0x9E37_79B9_7F4A_7C15,
        target: 0,
        lights_only: false,
        spawn_radius: 400.0,
        time: 0.0,
        released: Vec::new(),
        camera: None,
        orphan_sounds: Vec::new(),
        lights,
        controller_of_object: HashMap::new(),
        trailer_types: HashMap::new(),
        sound_cfgs: HashMap::new(),
        root: std::path::PathBuf::new(),
        held_at_red: 0,
        stop_wishes: None,
        player_still: 0.0,
        day_time: 0.0,
        time_scale: 1.0,
        weekday: 0,
        night: false,
        daylight: None,
        next_id: 1,
        last_overtaker: None,
        first_turner: None,
        first_red: None,
        first_yield: None,
        first_passer: None,
        density_curve: Vec::new(),
        unsched_factor: 1.0,
        max_scheduled: 0,
        no_timetable_buses: false,
        viewer: None,
        occluders: None,
        walkers: Vec::new(),
        people: Vec::new(),
        initial: false,
        last_dt: 0.0,
        lamp_dt: 0.0,
        trace: None,
        logged_hard: Default::default(),
        light_log: None,
        light_prev,
        debug_population: false,
        framed_spawns: Vec::new(),
        player: None,
        player_priority: false,
        player_blinker: 0,
        player_signal_age: f32::MAX,
        player_signalling: 0.0,
        way_users: Vec::new(),
        others: Vec::new(),
        drivers: HashMap::new(),
        driver_pool: Vec::new(),
        tick_split: [0.0; 3],
        others_still: HashMap::new(),
        geo_prev: Vec::new(),
        index_of: HashMap::new(),
        pull_out_rooms: HashMap::new(),
        removed_scheduled: Vec::new(),
        twinned: Default::default(),
        keep_clear: Vec::new(),
        mirror: false,
        count_near: None,
        lan_centers: Vec::new(),
    }
}

/// A `Traffic` and the crossroads it runs on.
pub(super) struct World {
    pub t: Traffic,
    pub x: Crossroads,
}

impl World {
    pub fn new(layout: Layout, lights: Vec<TrafficLightController>) -> World {
        let mut x = Crossroads::new(layout);
        let t = traffic_over(std::mem::take(&mut x.net), lights);
        World { t, x }
    }

    /// A car of the random traffic standing on `lane` at `s` metres, going `speed` m/s. The
    /// same driver (`seed`) gives the same car: the scenarios do not depend on a random draw
    /// they did not make. `priority`: its script claims the right of way (`TrafficPriority`,
    /// the stock ambulance).
    pub fn car(&mut self, lane: usize, s: f32, speed: f32, seed: u64, priority: bool) -> u64 {
        let ty = car_type();
        let mut host = omsi_sim::VehicleHost::new(omsi_sim::SimClock::default());
        host.paint_scheme = Some(None);
        let mut vehicle = VehicleInstance::new(ty.clone(), host);
        if priority {
            assert!(vehicle.set_var("TrafficPriority", 1.0));
        }
        let mut state = AiState::new(lane, s, seed);
        state.plan_next(&self.t.net);
        personality(&mut state, seed, false);
        state.max_speed_kmh = 100.0;
        state.lat_accel = 2.4 + (seed % 7) as f32 * 0.1;
        let (front, rear, half_width) = extents(&ty, 4.5);
        state.front = front;
        state.rear = rear;
        state.length = front + rear;
        state.speed = speed;
        let body = place_body(&self.t.net, &state, &mut vehicle, MotionKind::Road);
        let id = self.t.next_id;
        self.t.next_id += 1;
        self.t.cars.push(AiCar {
            id,
            state,
            vehicle,
            render: no_render(),
            trailer_renders: Vec::new(),
            body,
            stopped: 0.0,
            lead_car: None,
            ignore_lead: None,
            crawl: 0.0,
            progress: (0.0, 0.0),
            bus: None,
            sounds: None,
            half_width,
            yielding: false,
            light_hold: false,
            reserved: Vec::new(),
            amber: None,
            passing: None,
            gone: false,
            fresh: 0.0,
            merge_after: None,
            holding: None,
            why: ("", 0.0),
            held: false,
            geo_block: None,
            lead_info: None,
            junction_why: String::new(),
            wait_at: None,
            seed,
            scheme: None,
            squeeze: None,
            pass_room: 0.0,
            pass_retry: 0.0,
            light_at: None,
            pull_out: 0.0,
            rail_trail: Default::default(),
            ai_secs: 0.0,
            consist_reversed: false,
            park: None,
        });
        id
    }

    pub fn of(&self, id: u64) -> &AiCar {
        self.t.cars.iter().find(|c| c.id == id).expect("car")
    }

    /// One frame.
    pub fn tick(&mut self) {
        self.t.tick(DT, None);
    }

    /// Run `secs` seconds, calling `watch` after every frame.
    pub fn run(&mut self, secs: f32, mut watch: impl FnMut(&World)) {
        for _ in 0..(secs / DT).round() as usize {
            self.tick();
            watch(self);
        }
    }

    /// Is the car on one of the crossroads' junction paths (its front over the line)?
    pub fn in_junction(&self, id: u64) -> bool {
        self.t.cars.iter().find(|c| c.id == id).is_some_and(|c| {
            let l = c.state.lane;
            self.x.junction.contains(&l)
                || self.x.left.contains(&Some(l))
                || self.x.right.contains(&Some(l))
        })
    }

    /// Send the car along `route` (its first lane is the one it is on).
    pub fn route(&mut self, id: u64, route: Vec<usize>) {
        let net = &self.t.net;
        let car = self.t.cars.iter_mut().find(|c| c.id == id).expect("car");
        let s = car.state.s;
        car.state.set_route(net, route, s);
    }

    /// Has the car left the junction (on an exit, the stretch beyond it, or off the road at
    /// the end of it)?
    pub fn through(&self, id: u64) -> bool {
        match self.t.cars.iter().find(|c| c.id == id) {
            None => true,
            Some(c) => {
                c.gone
                    || self.x.exit.contains(&c.state.lane)
                    || self.x.beyond.contains(&c.state.lane)
            }
        }
    }
}
