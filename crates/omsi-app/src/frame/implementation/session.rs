//! Setting up a session: the parts of the start that the window (`App::start_world`) and the
//! offscreen run (`run_offscreen`) share. Each function here is the code both of them ran
//! before, word for word; what only one of them does (the window's messages on the screen,
//! the offscreen run's checks and log lines) stays with it.

use crate::core::cli::{parse_time, Args};
use crate::gameplay::humans::Humans;
use crate::gameplay::money::Money;
use crate::gameplay::player::Player;
use crate::gameplay::schedule::{PlayerDuty, Schedule};
use crate::gameplay::traffic::Traffic;
use crate::settings::Settings;
use crate::situation::{start_clock, ticket_key_name};
use crate::world::scene::World;
use omsi_render::{Renderer, Scene};

/// The traffic of the session. A player who joins another's game draws the host's traffic in
/// it, whatever their own count says: without it the host's cars had nowhere to go. Without
/// traffic it still runs the light programs and switches the lamps: they stood frozen with
/// red, yellow and green all lit (#727).
pub(crate) fn new_traffic(
    args: &Args,
    world: &World,
    renderer: &Renderer,
    scene: &mut Scene,
) -> anyhow::Result<Traffic> {
    let populated = args.traffic > 0
        || args.schedule
        || crate::gameplay::rail_drive::args_rail(args)
        || args.lan_join.is_some();
    let mut t = Traffic::new(&args.root, world, args.traffic)?;
    t.lights_only = !populated;
    t.no_timetable_buses = args.no_timetable_buses;
    if args.traffic > 0 {
        t.precache_random(world, renderer, scene);
    }
    Ok(t)
}

/// The timetable of the session (`--schedule`), with its vehicles loaded ahead.
pub(crate) fn new_schedule(
    args: &Args,
    world: &World,
    renderer: &Renderer,
    scene: &mut Scene,
    traffic: Option<&mut Traffic>,
) -> Schedule {
    let mut sch = Schedule::new(&args.root, world, &start_clock(args));
    sch.precache(world, renderer, scene, traffic, parse_time(&args.time));
    sch
}

/// The player's duty on `line` (`--line`, `--tour`, `--trip`, `--duty-trip`): started at the
/// trip asked for and, for a saved situation, resumed where it was left.
pub(crate) fn player_duty(
    sch: &mut Schedule,
    args: &Args,
    world: &World,
    line: &str,
    p: &mut Player,
) -> Result<PlayerDuty, String> {
    let mut d = sch.player_duty(
        world,
        line,
        args.tour.as_deref().unwrap_or(""),
        parse_time(&args.time),
        args.trip.as_deref(),
        args.whole_tour,
    )?;
    if let Some(k) = args.duty_trip {
        d.start_at(k, args.duty_first_stop);
    }
    if args.is_resuming() {
        d.resume(
            &mut p.vehicle,
            parse_time(&args.time),
            args.situation_next_stop,
        );
        // the IBIS keeps its saved trip; with --autostart the duty's next trips are typed
        // into it again
        p.duty_typed = args.autostart;
    }
    Ok(d)
}

/// The driver's timetable paper in the cab, written for the duty.
pub(crate) fn timetable_paper(p: &mut Player, d: &PlayerDuty, world: &World) {
    let mut fonts = world.fonts.lock();
    if let Err(e) = crate::gameplay::schedule_paper::update_vehicle(&mut p.vehicle, d, &mut fonts) {
        log::warn!("driver timetable paper: {e:#}");
    }
}

/// The people on foot and the passengers (`--passengers`), with the settings' fares,
/// boarding and voices and, with a player's bus, its cabin, ticket key, tickets and money.
pub(crate) fn new_humans(
    args: &Args,
    settings: &Settings,
    world: &World,
    player: Option<&mut Player>,
) -> Humans {
    let mut h = Humans::new(&args.root);
    h.exact_fare = settings.exact_fare;
    h.boarding = settings.boarding.clone();
    h.voices = match settings.pax_voices.as_str() {
        "off" => 2,
        "tickets" => 1,
        _ => 0,
    };
    if let Some(p) = player {
        h.set_cabin(&mut p.vehicle);
        h.ticket_key = ticket_key_name(&args.root, &p.bindings);
        h.tickets = p.vehicle.host.tickets.clone();
        if !world.global.money_system.trim().is_empty() {
            h.money = Some(Money::new(&args.root, &world.global.money_system));
        }
    }
    h
}

/// The traffic's step of a frame, the same in the window and the offscreen run: the
/// player's priority and indicator for the AI, the AI's tick against the player's outline,
/// then the points and the signals the cars asked for.
///
/// `player_rail` is the track the player drives on (`(lane, along)`). The window passes it,
/// so a signal clears for a player's train; the offscreen run passes `None`, so there the
/// same signal stays at stop for a scripted player's train and only AI trains clear it.
/// The difference is kept on purpose until it is changed deliberately (see the test
/// `window_and_offscreen_differ_in_the_rail_given_to_signal_aspects`).
pub(crate) fn traffic_step(
    t: &mut Traffic,
    world: &World,
    player: Option<&Player>,
    dt: f32,
    player_rail: Option<(usize, bool)>,
) {
    t.player_priority = player
        .and_then(|p| p.vehicle.var("TrafficPriority"))
        .is_some_and(|v| v > 0.5);
    t.player_blinker = player
        .map(|p| crate::gameplay::traffic_link::indicator(&p.vehicle))
        .unwrap_or(0);
    t.tick(
        dt,
        player.map(crate::gameplay::traffic_link::player_outline),
    );
    world.set_switches(&t.switch_requests());
    world.set_signals(&t.signal_aspects(&world.signal_routes, player_rail));
}
