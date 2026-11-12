//! Setting up a session: the parts of the start that the window (`App::start_world`) and the
//! offscreen run (`run_offscreen`) share. Each function here is the code both of them ran
//! before, word for word; what only one of them does (the window's messages on the screen,
//! the offscreen run's checks and log lines) stays with it.

use crate::core::cli::{parse_time, Args};
use crate::gameplay::career::Career;
use crate::gameplay::humans::Humans;
use crate::gameplay::journey::Journey;
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

/// The duty's step of a frame, part 1: a stop the timetable window asked for
/// (`html_next_stop`) becomes the next one, and the IBIS is set to it.
pub(crate) fn duty_skip_asked(d: &mut PlayerDuty, p: &mut Player) {
    if let Some(stop) = p.html_next_stop.take() {
        if d.skip_to(stop) {
            let (trip, k) = d.trip_for_ibis();
            p.ibis_to_stop(trip, k);
        }
    }
}

/// The duty's step of a frame, part 2: the duty follows the bus at `now`, a stop served
/// counts in the career, and the journey's log notes it. The journey's heading takes the
/// time from `clock`, or from the bus's own clock when it is `None` (the offscreen run).
#[allow(clippy::too_many_arguments)]
pub(crate) fn duty_update(
    d: &mut PlayerDuty,
    p: &mut Player,
    now: f64,
    career: &mut Career,
    journey: &mut Option<Journey>,
    root: &std::path::Path,
    world: &World,
    clock: Option<&omsi_sim::SimClock>,
) {
    let due = (d.trip_index, d.next_stop);
    let served = d.update(&mut p.vehicle, now);
    if let Some((arrival, departure)) = served {
        career.stop_served(arrival, departure);
    }
    crate::gameplay::journey::note(journey, d, due, served, root, || {
        crate::gameplay::journey::head(
            career,
            &world.global.name,
            &p.vehicle,
            clock.unwrap_or(&p.vehicle.host.clock),
        )
    });
}

/// The duty's step of a frame, part 3: a new trip is typed into the IBIS when the duty
/// was typed there, and the driver's timetable paper follows the duty.
pub(crate) fn duty_after(d: &mut PlayerDuty, p: &mut Player, world: &World) {
    if d.take_trip_change() && p.duty_typed {
        let (trip, stop) = d.trip_for_ibis();
        p.set_duty_destination(trip, stop);
    }
    timetable_paper(p, d, world);
}

/// The people's step of a frame, the clock part: how many people the map has at this hour
/// (`people_curve`, times the settings' passengers density), the time of day and the
/// daylight they see.
pub(crate) fn people_clock(
    h: &mut Humans,
    world: &World,
    settings: &Settings,
    clock: &omsi_sim::SimClock,
    envir: Option<&omsi_content::Envir>,
) {
    h.people_curve = world.global.passenger_density((clock.time / 3600.0) as f32);
    // (OMSI's `AIPassFactor`, the passengers setting in per cent)
    h.density = h.people_curve * settings.pax_density;
    h.time_of_day = clock.time;
    h.daylight = omsi_sim::Daylight::compute(clock, envir).light_a;
}

/// The people's step of a frame, after their tick: the validators used play the bus's
/// `ev_Stamper` sound, on the player's bus or on the AI bus they were used in.
pub(crate) fn people_stamped(
    h: &mut Humans,
    mut player: Option<&mut Player>,
    mut traffic: Option<&mut Traffic>,
) {
    for bus in h.take_stamped() {
        match bus {
            None => {
                if let Some(p) = player.as_deref_mut() {
                    p.vehicle.host.fired_triggers.push("ev_Stamper".into());
                }
            }
            Some(id) => {
                if let Some(c) = traffic
                    .as_deref_mut()
                    .and_then(|t| t.cars.iter_mut().find(|c| c.id == id))
                {
                    c.vehicle.host.fired_triggers.push("ev_Stamper".into());
                }
            }
        }
    }
}

/// The people's step of a frame, what the traffic takes from them: who wants to get off
/// and who waits at which stop, the AI buses held at a stop while people board, and the
/// doors the passengers ask an AI bus to open.
pub(crate) fn people_to_traffic(h: &mut Humans, t: &mut Traffic) {
    let (alighting, waiting) = h.stop_wishes();
    t.set_stop_wishes(alighting, waiting);
    for (id, stop, secs) in h.take_holds() {
        t.hold_boarding(id, stop, secs);
    }
    for (id, doors) in h.take_ai_requests() {
        t.set_pax_requests(id, &doors);
    }
    t.set_payloads(&h.ai_payloads());
}

/// The people's step of a frame after their tick, the same in the window and the offscreen
/// run: the player's bus weighs the people aboard on the section they are in, the people
/// stamp the validators' sound, and the traffic takes what the people want of it.
pub(crate) fn people_after_tick(
    h: &mut Humans,
    player: Option<&mut Player>,
    traffic: Option<&mut Traffic>,
) {
    let mut player = player;
    let mut traffic = traffic;
    if let Some(p) = player.as_deref_mut() {
        p.vehicle.set_payload(&h.player_payload());
    }
    people_stamped(h, player.as_deref_mut(), traffic.as_deref_mut());
    if let Some(t) = traffic.as_deref_mut() {
        people_to_traffic(h, t);
    }
}

/// The people's step of a frame, the hand-over to the player's bus, the same in the window and
/// the offscreen run: the ticket, the change and the stop the player asks for by the keys reach
/// the passengers, the bus learns who rides it, and the money shows the change due.
pub(crate) fn people_handover(
    h: &mut Humans,
    world: &World,
    renderer: &Renderer,
    scene: &mut Scene,
    player: Option<&mut Player>,
    took: bool,
) {
    let stop = std::mem::take(&mut h.stop_request);
    if let Some(p) = player {
        if took {
            p.vehicle.set_var("GivenTicket", -1.0);
        }
        h.give_ticket = std::mem::take(&mut p.give_ticket);
        h.give_change_all = std::mem::take(&mut p.give_change);
        if std::mem::take(&mut p.take_change) {
            h.take_change_tray();
        }
        if stop {
            // a passenger's request is the vehicle trigger OMSI fires,
            // not the cab's stop button `door_haltewunsch`,
            // whose switch and brake sounds some buses play
            p.vehicle.trigger("int_haltewunsch");
        }
        h.write_pax_vars(&mut p.vehicle);
        p.vehicle.host.humans_on_path_link = h.path_link_counts();
        p.vehicle.host.humans_on_seat = h.seat_counts();
        let coins: Vec<usize> = std::mem::take(&mut p.vehicle.host.change_coins);
        h.give_change(world, renderer, scene, &coins);
        h.sync_money(world, renderer, scene, &p.vehicle);
    }
}
