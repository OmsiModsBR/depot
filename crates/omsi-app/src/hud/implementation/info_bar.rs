//! The information bar's items: what each layout shows of [`InfoData`] and how each value is
//! written.

use crate::gameplay::player::Player;
use crate::hud::domain::{InfoBar, InfoCell, InfoData, InfoItem, InfoMode, InfoTone};
use crate::hud::implementation::legacy::fps_level;

impl InfoMode {
    pub fn from_setting(s: &str) -> InfoMode {
        match s {
            "compact" => InfoMode::Compact,
            "essential" => InfoMode::Essential,
            "custom" => InfoMode::Custom,
            "twotier" => InfoMode::TwoTier,
            "split" => InfoMode::Split,
            _ => InfoMode::Full,
        }
    }
}

/// OMSI's information bar: the time, the speed (and the limit of the road), the kilometres
/// driven this session and the bus's odometer, temperatures, the tank, the passengers aboard,
/// the trip with its next stop and delay, the pedals, the road's grade and the skid lamp. A
/// group of it is one thing the player reads at a time; `mode` leaves out what is not wanted,
/// `fps` adds the frame rate at the end. Each readout is an icon and its value; its name is
/// the icon's tooltip.
pub fn info_bar(d: &InfoData, mode: InfoMode, bottom: bool) -> InfoBar {
    if matches!(mode, InfoMode::TwoTier | InfoMode::Split) {
        return tiered_bar(d, mode == InfoMode::Split, bottom);
    }
    let plain = |t: String| InfoItem::new(t, InfoTone::Plain);
    // (the custom layout is the full one less what is switched off, item by item: the time,
    // the speed, the trip distance, the odometer, the temperatures, the tank, the passengers,
    // the line, the next stop, the pedals, the road's grade and the skid lamp; the groups
    // keep their order, so nothing moves when one is switched on or off)
    let full = matches!(mode, InfoMode::Full | InfoMode::Custom);
    let on = |i: usize| mode != InfoMode::Custom || d.items[i];
    let mut groups: Vec<Vec<InfoItem>> = Vec::new();
    if mode != InfoMode::Essential && on(0) {
        groups.push(vec![clock_item(d.clock.time, full)]);
    }
    if let Some(p) = d.player {
        if on(1) {
            groups.push(speed_group(p, d.limit));
        }
        // (every layout: what the driver does and what the bus does under it)
        groups.extend(driving_groups(p, &on));
        if full && on(2) {
            groups.push(vec![plain(distance_driven(d.metres))
                .with_icon("hud-trip")
                .with_tip("Trip")]);
        }
        if full {
            // the bus's whole mileage, as OMSI's Shift+Z overlay reads it ("Mileometer", the
            // `kmcounter_*` the cockpit shows, whole kilometres and the metres of the fraction)
            if let Some(km) = p
                .vehicle
                .var("kmcounter_km")
                .filter(|v| v.is_finite() && on(3))
            {
                groups.push(vec![plain(odometer_reading(
                    km as f64 + p.vehicle.var("kmcounter_m").unwrap_or(0.0) as f64 / 1000.0,
                ))
                .with_icon("hud-odometer")
                .with_tip("Odometer")]);
            }
            if on(4) {
                let (outside, inside) = p.temperatures();
                groups.push(vec![plain(temperatures(outside, inside))
                    .with_icon("hud-temperature")
                    .with_tip("Outside / cabin")]);
            }
            // the tank as the bus's script says it (OMSI's RL_TankContent: tank_percent)
            if let Some(tank) = p
                .vehicle
                .var("tank_percent")
                .filter(|v| v.is_finite() && on(5))
            {
                groups.push(vec![plain(format!("{:.0} %", (tank * 100.0).round()))
                    .with_icon("hud-fuel")
                    .with_tip("Tank")]);
            }
            // how many are aboard right now (None: the passengers are switched off for this
            // drive, so there is nothing to count)
            if let Some(n) = d.passengers.filter(|_| on(6)) {
                groups.push(vec![plain(passengers_aboard(n, d.capacity))
                    .with_icon("hud-passengers")
                    .with_tip("Passengers")]);
            }
        }
        if let Some(duty) = d.duty {
            if let Some(trip) = duty.trips.get(duty.trip_index) {
                let delay = delay_item(p.vehicle.host.tt_delay);
                if mode == InfoMode::Essential {
                    groups.push(vec![delay]);
                } else {
                    let line = if trip.line.trim().is_empty() {
                        duty.line.trim()
                    } else {
                        trip.line.trim()
                    };
                    // (names from the map are drawn as they are, not looked up as interface
                    // texts: a destination "Depot" is the map's word)
                    let mut line_group =
                        vec![InfoItem::raw(line, InfoTone::Badge).with_tip("Line")];
                    if full && !trip.terminus.trim().is_empty() {
                        line_group.push(route_item(trip.terminus.trim()));
                    }
                    let next = trip
                        .stops
                        .get(duty.next_stop)
                        .map(|s| next_stop_item(s.name.trim()));
                    if full {
                        if on(7) {
                            groups.push(line_group);
                        }
                        if on(8) {
                            groups.push(next.into_iter().chain([delay]).collect());
                        }
                    } else {
                        // (compact: the line's badge and the next stop are one group)
                        line_group.extend(next);
                        line_group.push(delay);
                        groups.push(line_group);
                    }
                }
            }
        }
    }
    if let Some((fps, low)) = d.fps {
        let level = fps_level(fps);
        let text = match (full, low) {
            (true, Some(low)) => {
                format!("{fps:.0} fps   {} {low:.0}", omsi_ui::tr("1 % low"))
            }
            (false, Some(low)) => format!("{fps:.0} / {low:.0} fps"),
            (_, None) => format!("{fps:.0} fps"),
        };
        groups.push(vec![
            InfoItem::new(text, InfoTone::Fps(level)).with_tip("Frame rate")
        ]);
    }
    InfoBar {
        groups,
        bottom,
        large: mode == InfoMode::Essential,
        ..Default::default()
    }
}

/// The clock: hours, minutes and (`seconds`) seconds, after the clock icon.
fn clock_item(t: f64, seconds: bool) -> InfoItem {
    let (h, m) = (
        ((t / 3600.0) as i64).rem_euclid(24),
        ((t % 3600.0) / 60.0) as i64,
    );
    let text = if seconds {
        format!("{h:02}:{m:02}:{:02}", (t % 60.0) as i64)
    } else {
        format!("{h:02}:{m:02}")
    };
    InfoItem::new(text, InfoTone::Plain)
        .with_icon("hud-time")
        .with_tip("Current time")
}

/// The speed (as wide as three digits), its unit and the limit of the road when it has one.
fn speed_group(p: &Player, limit: Option<f32>) -> Vec<InfoItem> {
    let mut speed = vec![
        InfoItem::new(
            format!("{:.0}", p.vehicle.motion().velocity_kmh().abs()),
            InfoTone::Speed,
        ),
        InfoItem::new("km/h", InfoTone::Dim),
    ];
    if let Some(limit) = limit {
        speed.push(InfoItem::new(format!("{limit:.0}"), InfoTone::Limit).with_tip("Speed limit"));
    }
    speed
}

/// The pedals, the road's grade and the skid lamp of the player's vehicle (see
/// [`driving_items`]). The grade and the lamp read the rigid body it runs on: a vehicle moved
/// along its track has neither.
fn driving_groups(p: &Player, on: impl Fn(usize) -> bool) -> Vec<Vec<InfoItem>> {
    let body = p.vehicle.rigid();
    driving_items(
        p.vehicle.controls(),
        body.and_then(|b| b.road_grade()),
        body.map(|b| b.skidding()),
        on,
    )
}

/// The pedals, the road's grade (when known) and the skid lamp (when known), each a group of
/// its own, as `on` lets them through (the Custom layout's switches 9, 10 and 11).
pub fn driving_items(
    controls: omsi_sim::Controls,
    grade: Option<f32>,
    skidding: Option<bool>,
    on: impl Fn(usize) -> bool,
) -> Vec<Vec<InfoItem>> {
    let mut groups = Vec::new();
    if on(9) {
        groups.push(vec![pedals_item(controls)]);
    }
    if let Some(grade) = grade.filter(|_| on(10)) {
        groups.push(vec![grade_item(grade)]);
    }
    if let Some(skidding) = skidding.filter(|_| on(11)) {
        groups.push(vec![skid_item(skidding)]);
    }
    groups
}

/// The clutch, the brake and the throttle as the vehicle has them this frame, whatever
/// works them (the keys, a wheel and its pedals, a gamepad, the touch screen, the
/// automatic clutch): three bars, no text; "Pedals" under the mouse.
pub fn pedals_item(c: omsi_sim::Controls) -> InfoItem {
    let percent = |v: f32| (v.clamp(0.0, 1.0) * 100.0).round() as u8;
    InfoItem::new(
        "",
        InfoTone::Pedals([percent(c.clutch), percent(c.brake), percent(c.throttle)]),
    )
    .with_tip("Pedals")
}

/// The grade of the road under the vehicle in whole percent, a true minus downhill, after
/// the slope icon: "8 %", "−5 %".
pub fn grade_item(grade: f32) -> InfoItem {
    let percent = grade.round();
    let text = if percent < 0.0 {
        format!("−{:.0} %", -percent)
    } else {
        format!("{:.0} %", percent.abs())
    };
    InfoItem::new(text, InfoTone::Plain)
        .with_icon("hud-bus-slope")
        .with_tip("Road grade")
}

/// The skid lamp: the skidding bus lit white while the vehicle skids, dark otherwise.
pub fn skid_item(skidding: bool) -> InfoItem {
    let icon = if skidding {
        "hud-bus-skid-white"
    } else {
        "hud-bus-skid-dark"
    };
    InfoItem::new("", InfoTone::Light(skidding))
        .with_icon(icon)
        .with_tip("Skidding")
}

/// Where the trip goes: cut short when long (the next stop keeps its room), whole under the
/// mouse.
fn route_item(terminus: &str) -> InfoItem {
    let mut it = InfoItem::raw(terminus, InfoTone::Plain).with_tip(terminus);
    it.clip = true;
    it
}

/// The next stop: its name after the stop icon (the icon says what it is; "Next stop" under
/// the mouse).
fn next_stop_item(name: &str) -> InfoItem {
    InfoItem::raw(name, InfoTone::Plain)
        .with_icon("hud-stop")
        .with_tip("Next stop")
}

/// The delay to the timetable (`delay` seconds, negative ahead) as a chip: red when more than
/// a minute late, green when more than a minute early, the accent in between; a true minus.
pub fn delay_item(delay: f32) -> InfoItem {
    let kind = if delay > 60.0 {
        1
    } else if delay < -60.0 {
        -1
    } else {
        0
    };
    InfoItem::new(
        format!(
            "{}{}:{:02}",
            if delay < 0.0 { "−" } else { "+" },
            (delay.abs() / 60.0) as i64,
            (delay.abs() % 60.0) as i64
        ),
        InfoTone::Delay(kind),
    )
    .with_tip("early / late")
}

/// The outside and the cabin temperature with the unit once: "15 / 19 °C".
pub fn temperatures(outside: f32, inside: f32) -> String {
    format!("{outside:.0} / {inside:.0} °C")
}

/// The two-tier layouts: the driving items (the line and where it goes, the time, the speed
/// and its limit, the pedals, the road's grade, the skid lamp, the next stop with the delay)
/// in the first tier, small readouts (trip,
/// odometer, temperatures, passengers, the tank with its meter, the frame rate) in the
/// second, which the arrow folds away. `split` puts the second tier on a plate of its own.
fn tiered_bar(d: &InfoData, split: bool, bottom: bool) -> InfoBar {
    let plain = |t: String| InfoItem::new(t, InfoTone::Plain);
    let dim = |t: &str| InfoItem::new(t, InfoTone::Dim);
    let mut groups: Vec<Vec<InfoItem>> = vec![vec![clock_item(d.clock.time, true)]];
    let mut cells: Vec<InfoCell> = Vec::new();
    // (a readout: its icon, its name above the value when the names are on; the name is the
    // icon's tooltip either way)
    let cell = |name: &str, icon: &'static str, value: String| InfoCell {
        items: if d.labels {
            vec![dim(name), plain(value)]
        } else {
            vec![plain(value)]
        },
        icon: Some(icon),
        tip: Some(name.to_string()),
        ..Default::default()
    };
    if let Some(p) = d.player {
        if let Some(duty) = d.duty {
            if let Some(trip) = duty.trips.get(duty.trip_index) {
                let line = if trip.line.trim().is_empty() {
                    duty.line.trim()
                } else {
                    trip.line.trim()
                };
                let mut line_group = vec![InfoItem::raw(line, InfoTone::Badge).with_tip("Line")];
                if !trip.terminus.trim().is_empty() {
                    line_group.push(route_item(trip.terminus.trim()));
                }
                // (the line and where it goes first, as the bus's sign reads)
                groups.insert(0, line_group);
            }
        }
        groups.push(speed_group(p, d.limit));
        groups.extend(driving_groups(p, |_| true));
        if let Some(duty) = d.duty {
            if let Some(trip) = duty.trips.get(duty.trip_index) {
                let next: Vec<InfoItem> = trip
                    .stops
                    .get(duty.next_stop)
                    .map(|s| next_stop_item(s.name.trim()))
                    .into_iter()
                    .chain([delay_item(p.vehicle.host.tt_delay)])
                    .collect();
                groups.push(next);
            }
        }
        cells.push(cell("Trip", "hud-trip", distance_driven(d.metres)));
        if let Some(km) = p.vehicle.var("kmcounter_km").filter(|v| v.is_finite()) {
            let km = km as f64 + p.vehicle.var("kmcounter_m").unwrap_or(0.0) as f64 / 1000.0;
            cells.push(cell("Odometer", "hud-odometer", odometer_reading(km)));
        }
        let (outside, inside) = p.temperatures();
        cells.push(cell(
            "Outside / cabin",
            "hud-temperature",
            temperatures(outside, inside),
        ));
        if let Some(n) = d.passengers {
            cells.push(cell(
                "Passengers",
                "hud-passengers",
                passengers_aboard(n, d.capacity),
            ));
        }
        if let Some(tank) = p.vehicle.var("tank_percent").filter(|v| v.is_finite()) {
            let level = tank.clamp(0.0, 1.0);
            let low = d.low_fuel && level < 0.15;
            let mut c = cell(
                "Tank",
                "hud-fuel",
                format!("{:.0} %", (level * 100.0).round()),
            );
            if low {
                if let Some(v) = c.items.last_mut() {
                    v.tone = InfoTone::Late;
                }
            }
            c.meter = Some(level);
            c.alert = low;
            cells.push(c);
        }
    }
    if let Some((fps, low)) = d.fps {
        let text = match low {
            Some(low) => format!("{fps:.0} fps   {} {low:.0}", omsi_ui::tr("1 % low")),
            None => format!("{fps:.0} fps"),
        };
        cells.push(InfoCell {
            items: vec![InfoItem::new(text, InfoTone::Fps(fps_level(fps)))],
            tip: Some("Frame rate".into()),
            ..Default::default()
        });
    }
    if d.collapsed {
        cells.clear();
    }
    InfoBar {
        groups,
        bottom,
        tier2: cells,
        split,
        tiered: true,
        collapsed: d.collapsed,
        large: false,
    }
}

/// `v` with its whole part in groups of three, split by a thin space (the same in every
/// language: neither the English comma nor the dot of the languages that write a decimal
/// comma), to `decimals` places: "18 427.3".
pub fn grouped(v: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, v.max(0.0));
    let (whole, frac) = match text.split_once('.') {
        Some((w, f)) => (w.to_string(), Some(f.to_string())),
        None => (text.clone(), None),
    };
    let mut out = String::new();
    for (k, c) in whole.chars().enumerate() {
        if k > 0 && (whole.len() - k) % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(&f);
    }
    out
}

/// The kilometres driven this session (`Career::metres`), to a hundred metres.
pub fn distance_driven(metres: f64) -> String {
    format!("{} km", grouped(metres / 1000.0, 1))
}

/// The bus's odometer to a hundred metres, as the stock cockpits show it (km and tenths);
/// the word is the icon's tooltip.
pub fn odometer_reading(km: f64) -> String {
    format!("{} km", grouped(km, 1))
}

/// How many are aboard, of how many places when the bus has places to count: "36 / 111"
/// (the word is the icon's tooltip).
pub fn passengers_aboard(n: usize, capacity: Option<usize>) -> String {
    match capacity {
        Some(c) => format!("{n} / {c}"),
        None => n.to_string(),
    }
}
