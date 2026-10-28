//! The buses of the timetable's lines: on the bus tab, each tour of a line of the map's own
//! `TTData` can take its buses from another AI group, and the buses of a depot's fleet lists
//! (`ailists.cfg`) can be added, changed and taken out. Same rules as the car groups: nothing
//! is written until Save, after a copy of each file.

use super::*;
use crate::launcher::ai_traffic::{status_of, table, Col};
use omsi_integrity::traffic::{AiTraffic, TrafficLine};

/// The fields of a bus of a fleet list while they are typed.
pub struct BusForm {
    group: usize,
    fleet: usize,
    /// The bus changed (its place in the list); none for a new one.
    bus: Option<usize>,
    number: String,
    plate: String,
    paint: String,
    from: String,
    until: String,
    error: Option<&'static str>,
}

fn day_text(d: Option<i32>) -> String {
    match d {
        Some(d) => format!("{:04}-{:02}-{:02}", d / 10_000, d / 100 % 100, d % 100),
        None => String::new(),
    }
}

/// The line `line` of the map's own `TTData` that `e` holds, if it is one.
fn ttl_of(e: &Editor, line: &TrafficLine) -> Option<usize> {
    let want = line.file.replace('\\', "/").to_ascii_lowercase();
    let folder = e.folder.to_ascii_lowercase();
    e.now
        .ttls
        .iter()
        .position(|t| want == format!("maps/{folder}/{}", t.file.to_ascii_lowercase()))
}

pub fn bus_lines(l: &mut Launcher, body: Rect, t: &AiTraffic) {
    let Some(mut e) = l.pages.diag.ai.edit.take() else {
        return;
    };
    show(l, &mut e, body, t);
    l.pages.diag.ai.edit = Some(e);
}

fn show(l: &mut Launcher, e: &mut Editor, body: Rect, t: &AiTraffic) {
    let (left, mid, right_r) = columns(body);
    card(&mut l.ui, left, "Lines", Some(&t.lines.len().to_string()));
    if t.lines.is_empty() {
        clipped(
            &mut l.ui,
            "This map has no timetable lines.",
            left.x + 17.0,
            left.y + 60.0,
            left.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        return;
    }
    e.line = keep(e.line, t.lines.len());
    let rows: Vec<(String, String, Status)> = t
        .lines
        .iter()
        .map(|line| {
            (
                line.name.clone(),
                many(line.tours.len(), "1 tour", "%{n} tours"),
                Status::Found,
            )
        })
        .collect();
    let list_r = Rect::new(left.x, left.y + 40.0, left.w, left.h - 96.0);
    if let Some(i) = entries(l, "ai-edit-lines", list_r, &rows, e.line) {
        e.line = i;
        e.tour = 0;
        e.bus = None;
    }
    foot(
        l,
        left,
        "The tours that run on the chosen day. A tour's AI group is written in its line's file.",
    );

    let line = &t.lines[e.line];
    let ttl = ttl_of(e, line);
    let editable = ttl.is_some_and(|i| e.now.ttls[i].doc.exact());

    // the line's tours, with the group each takes its buses from
    let tour_h = (mid.h * 0.42).max(160.0);
    let tours_r = Rect::new(mid.x, mid.y, mid.w, tour_h);
    let title = omsi_ui::tr("Line %{name} · tours").replace("%{name}", &line.name);
    let note = if editable {
        String::new()
    } else {
        omsi_ui::tr("Read only: not a line of this map's own TTData").to_string()
    };
    card(&mut l.ui, tours_r, &title, Some(&note));
    e.tour = keep(e.tour, line.tours.len());
    // (the group a tour has now: the edited file's, else the one read)
    let group_of = |e: &Editor, number: &str, read: &str| -> String {
        ttl.and_then(|i| {
            e.now.ttls[i]
                .doc
                .tours()
                .iter()
                .find(|d| d.number.trim() == number.trim())
                .map(|d| d.ai_group.trim().to_string())
        })
        .unwrap_or_else(|| read.trim().to_string())
    };
    let rows: Vec<(Vec<String>, Status, bool)> = line
        .tours
        .iter()
        .map(|x| {
            let g = group_of(e, &x.number, &x.ai_group);
            let known = e.now.ailists.find(&g).is_some() || x.group_known;
            (
                vec![x.number.clone(), g, x.trips.to_string()],
                if known {
                    Status::Found
                } else {
                    Status::Warning
                },
                false,
            )
        })
        .collect();
    let cols = [
        Col("Tour", 1.0, Align::Left),
        Col("AI group", 2.0, Align::Left),
        Col("Trips", 0.7, Align::Right),
    ];
    let table_r = Rect::new(tours_r.x, tours_r.y + 40.0, tours_r.w, tours_r.h - 90.0);
    if let Some(i) = table(l, "ai-edit-tours", table_r, &cols, &rows, Some(e.tour)) {
        e.tour = i;
        e.bus = None;
    }
    let change_r = Rect::new(
        tours_r.right() - 17.0 - 190.0,
        tours_r.bottom() - 44.0,
        190.0,
        32.0,
    );
    let tour = line.tours.get(e.tour);
    if button(
        l,
        "ai-edit:tour-group",
        change_r,
        "Change AI group",
        Some("tune"),
        ButtonKind::Normal,
        editable && tour.is_some(),
        if editable {
            "Makes the chosen tour take its buses from another AI group. The line's file is changed on Save."
        } else {
            "Only the lines of this map's own TTData can be changed here."
        },
    ) {
        if let (Some(i), Some(tour)) = (ttl, tour) {
            let file = e.now.ttls[i].file.clone();
            let at = e.now.ttls[i]
                .doc
                .tours()
                .iter()
                .position(|d| d.number.trim() == tour.number.trim());
            if let Some(at) = at {
                e.dialog = Some(Dialog::TourGroup {
                    file,
                    tour: at,
                    sel: None,
                });
            }
        }
    }

    // the fleet behind the chosen tour
    let fleet_r = Rect::new(
        mid.x,
        tours_r.bottom() + 12.0,
        mid.w,
        mid.bottom() - tours_r.bottom() - 12.0,
    );
    let Some(tour) = line.tours.get(e.tour) else {
        card(&mut l.ui, fleet_r, "Depot fleet", None);
        details(l, right_r, &[]);
        return;
    };
    let gname = group_of(e, &tour.number, &tour.ai_group);
    let Some(gi) = e.now.ailists.find(&gname) else {
        card(&mut l.ui, fleet_r, "Depot fleet", None);
        l.ui.paragraph(
            &omsi_ui::tr("The map's own ailists.cfg has no group \"%{g}\": change the tour's AI group, or add the group under Car groups.")
                .replace("%{g}", &gname),
            glam::Vec2::new(fleet_r.x + 17.0, fleet_r.y + 46.0),
            fleet_r.w - 34.0,
            12.5,
            Weight::Regular,
            WARN,
        );
        details(
            l,
            right_r,
            &[
                ("Tour", format!("{}/{}", line.name, tour.number)),
                ("AI group", gname),
            ],
        );
        return;
    };
    let group = e.now.ailists.groups()[gi].clone();
    if group.kind != GroupKind::Depot {
        card(
            &mut l.ui,
            fleet_r,
            &omsi_ui::tr("Group %{g}").replace("%{g}", group.name.trim()),
            None,
        );
        l.ui.paragraph(
            "This group is a list of cars, not a depot's fleet: its vehicles are edited under Car groups.",
            glam::Vec2::new(fleet_r.x + 17.0, fleet_r.y + 46.0),
            fleet_r.w - 34.0,
            12.5,
            Weight::Regular,
            MUTED,
        );
        details(
            l,
            right_r,
            &[
                ("Tour", format!("{}/{}", line.name, tour.number)),
                ("AI group", group.name.trim().to_string()),
            ],
        );
        return;
    }
    let all: usize = group.fleets.iter().map(|f| f.buses.len()).sum();
    let title = omsi_ui::tr("Depot fleet · %{g}").replace("%{g}", group.name.trim());
    card(
        &mut l.ui,
        fleet_r,
        &title,
        Some(&many(all, "1 bus", "%{n} buses")),
    );
    let root = PathBuf::from(l.state.config.root.trim());
    let mut index: Vec<(usize, usize)> = Vec::new();
    let mut rows: Vec<(Vec<String>, Status, bool)> = Vec::new();
    for (fi, f) in group.fleets.iter().enumerate() {
        let known = e.exists(&root, &f.file);
        for (bi, b) in f.buses.iter().enumerate() {
            index.push((fi, bi));
            rows.push((
                vec![
                    b.entry.number.clone(),
                    b.entry.registration.clone(),
                    b.entry.paint.clone(),
                    shown_name("", &f.file),
                    day_text(b.entry.from),
                    day_text(b.entry.to),
                ],
                status_of(known),
                false,
            ));
        }
    }
    let cols = [
        Col("Number", 0.8, Align::Left),
        Col("Plate", 1.1, Align::Left),
        Col("Repaint", 1.6, Align::Left),
        Col("Bus", 1.8, Align::Left),
        Col("From", 1.0, Align::Left),
        Col("Until", 1.0, Align::Left),
    ];
    let r = Rect::new(fleet_r.x, fleet_r.y + 40.0, fleet_r.w, fleet_r.h - 92.0);
    let bsel = e.bus.and_then(|b| index.iter().position(|x| *x == b));
    if let Some(i) = table(l, "ai-edit-fleet", r, &cols, &rows, bsel) {
        e.bus = index.get(i).copied();
    }
    // Add bus, Change, Remove
    let by = fleet_r.bottom() - 44.0;
    let can_add = !group.fleets.is_empty() && group.fleets.iter().any(|f| f.end_known());
    let chosen = e.bus.and_then(|(fi, bi)| {
        let f = group.fleets.get(fi)?;
        Some((fi, bi, f.buses.get(bi)?.entry.clone()))
    });
    let add_r = Rect::new(fleet_r.x + 17.0, by, 130.0, 32.0);
    let edit_r = Rect::new(add_r.right() + 8.0, by, 110.0, 32.0);
    let rm_r = Rect::new(edit_r.right() + 8.0, by, 110.0, 32.0);
    if button(
        l,
        "ai-edit:bus-add",
        add_r,
        "Add bus",
        Some("add"),
        ButtonKind::Primary,
        can_add,
        if can_add {
            "Adds a bus to the depot's fleet: its number, plate, repaint and the days it is in service."
        } else {
            "This depot has no list of a bus type to add a bus to."
        },
    ) {
        let fleet = chosen.as_ref().map_or(0, |c| c.0);
        e.dialog = Some(Dialog::Bus(BusForm {
            group: gi,
            fleet,
            bus: None,
            number: String::new(),
            plate: String::new(),
            paint: String::new(),
            from: String::new(),
            until: String::new(),
            error: None,
        }));
    }
    if button(
        l,
        "ai-edit:bus-edit",
        edit_r,
        "Change",
        Some("edit"),
        ButtonKind::Normal,
        chosen.is_some(),
        "Changes the chosen bus's number, plate, repaint or days in service.",
    ) {
        if let Some((fleet, bi, c)) = chosen.clone() {
            e.dialog = Some(Dialog::Bus(BusForm {
                group: gi,
                fleet,
                bus: Some(bi),
                number: c.number.clone(),
                plate: c.registration.clone(),
                paint: c.paint.clone(),
                from: day_text(c.from),
                until: day_text(c.to),
                error: None,
            }));
        }
    }
    if button(
        l,
        "ai-edit:bus-remove",
        rm_r,
        "Remove",
        Some("delete"),
        ButtonKind::Normal,
        chosen.is_some(),
        "Takes the chosen bus out of the depot's fleet.",
    ) {
        if let Some((fleet, bi, _)) = chosen.clone() {
            match e.act(|x| x.ailists.remove_bus(gi, fleet, bi)) {
                Ok(()) => e.bus = None,
                Err(err) => l.state.set_status(error_text(&err), true),
            }
        }
    }
    // the details of the chosen bus
    let tours = t
        .lines
        .iter()
        .flat_map(|ln| {
            ln.tours
                .iter()
                .filter(|x| x.ai_group.trim().eq_ignore_ascii_case(group.name.trim()))
                .map(move |x| format!("{}/{}", ln.name, x.number))
        })
        .collect::<Vec<_>>();
    let tours_text = if tours.len() > 4 {
        format!("{} … ({})", tours[..4].join(", "), tours.len())
    } else {
        tours.join(", ")
    };
    match chosen {
        Some((fi, _, c)) => {
            let file = group.fleets[fi].file.clone();
            let known = e.exists(&root, &file);
            let rest = vehicle_view(l, right_r, &file, &c.paint, known);
            details(
                l,
                rest,
                &[
                    ("Bus", shown_name("", &file)),
                    ("File", file.clone()),
                    ("Number", format!("{} · {}", c.number, c.registration)),
                    ("Repaint", c.paint.clone()),
                    ("From", day_text(c.from)),
                    ("Until", day_text(c.to)),
                    ("Tours", tours_text),
                ],
            );
        }
        None => details(
            l,
            right_r,
            &[
                ("Tour", format!("{}/{}", line.name, tour.number)),
                ("AI group", group.name.trim().to_string()),
                ("Buses", all.to_string()),
                ("Tours", tours_text),
            ],
        ),
    }
}

/// The window for the fields of a bus.
pub fn bus_dialog(l: &mut Launcher, e: &mut Editor, mut f: BusForm) -> Option<Dialog> {
    let group = e.now.ailists.groups().get(f.group)?.clone();
    let fleets: Vec<String> = group
        .fleets
        .iter()
        .map(|x| shown_name("", &x.file))
        .collect();
    let detailed = group.fleets.get(f.fleet)?.detailed;
    let title = if f.bus.is_some() {
        "Change bus"
    } else {
        "Add bus"
    };
    let inside = super::super::start::dialog(l, 600.0, 470.0, title, group.name.trim());
    let (buttons, body) = super::super::start::dialog_foot(l, inside);
    let mut y = body.y;
    let label_w = 130.0;
    let mut row = |l: &mut Launcher, label: &str| -> Rect {
        l.ui.text_in(
            label,
            Rect::new(body.x, y, label_w, 34.0),
            12.5,
            Weight::Regular,
            MUTED,
            Align::Left,
        );
        let r = Rect::new(body.x + label_w, y, body.w - label_w, 34.0);
        y += 44.0;
        r
    };
    // (the bus type: only a new bus picks it; a changed one stays in its list)
    let r = row(l, "Bus");
    if f.bus.is_some() {
        l.ui.p().rounded(r, omsi_ui::theme::CONTROL_RADIUS, PANEL_2);
        l.ui.text_in_as_is(
            &fleets[f.fleet],
            Rect::new(r.x + 12.0, r.y, r.w - 24.0, r.h),
            13.0,
            Weight::Regular,
            MUTED,
        );
    } else {
        let mut i = f.fleet;
        if l.ui.select_as_is("ai-edit-bus-type", r, &mut i, &fleets) {
            f.fleet = i;
        }
    }
    let mut touched = false;
    let r = row(l, "Number");
    touched |= l.ui.text_input(
        "ai-edit-bus-number",
        r,
        &mut f.number,
        "The fleet number, as the timetable and the plate name it",
        None,
    );
    if detailed {
        let r = row(l, "Plate");
        touched |=
            l.ui.text_input("ai-edit-bus-plate", r, &mut f.plate, "", None);
        let r = row(l, "Repaint");
        touched |= l.ui.text_input(
            "ai-edit-bus-paint",
            r,
            &mut f.paint,
            "The repaint's name, empty for the bus's own",
            None,
        );
        let r = row(l, "From");
        touched |= l.ui.text_input(
            "ai-edit-bus-from",
            r,
            &mut f.from,
            "YYYY-MM-DD, empty: from the start",
            None,
        );
        let r = row(l, "Until");
        touched |= l.ui.text_input(
            "ai-edit-bus-until",
            r,
            &mut f.until,
            "YYYY-MM-DD, empty: with no end",
            None,
        );
    } else {
        l.ui.paragraph(
            "This list names a bus by its number alone: it has no plate, repaint or days.",
            glam::Vec2::new(body.x, y),
            body.w,
            12.0,
            Weight::Regular,
            MUTED,
        );
    }
    if touched {
        f.error = None;
    }
    if let Some(err) = f.error {
        l.ui.paragraph(
            err,
            glam::Vec2::new(body.x, body.bottom() - 34.0),
            body.w,
            12.0,
            Weight::Regular,
            omsi_ui::theme::DANGER,
        );
    }
    let ok_r = Rect::new(buttons.right() - 140.0, buttons.y, 140.0, buttons.h);
    let cancel_r = Rect::new(ok_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        l,
        "ai-edit:bus-cancel",
        cancel_r,
        "Cancel",
        None,
        ButtonKind::Normal,
        true,
        "Closes this window without changing anything.",
    ) {
        return None;
    }
    let ready = !f.number.trim().is_empty();
    if button(
        l,
        "ai-edit:bus-ok",
        ok_r,
        if f.bus.is_some() { "Apply" } else { "Add bus" },
        Some(if f.bus.is_some() { "ok" } else { "add" }),
        ButtonKind::Primary,
        ready,
        "Puts the bus in the depot's fleet; ailists.cfg is changed on Save.",
    ) || (ready && l.ui.input.keys.contains(&Key::Enter))
    {
        // the days: empty, or a date
        let parse = |s: &str| -> Result<Option<i32>, ()> {
            if s.trim().is_empty() {
                return Ok(None);
            }
            omsi_map::ailists::date_code(s).map(Some).ok_or(())
        };
        let (from, until) = match (parse(&f.from), parse(&f.until)) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                f.error = Some("A day is a date as YYYY-MM-DD.");
                return Some(Dialog::Bus(f));
            }
        };
        let bus = DepotEntry {
            number: f.number.trim().to_string(),
            registration: f.plate.clone(),
            paint: if detailed {
                f.paint.clone()
            } else {
                f.number.trim().to_string()
            },
            from,
            to: until,
        };
        let (g, fl) = (f.group, f.fleet);
        let done = match f.bus {
            Some(b) => e.act(|x| x.ailists.set_bus(g, fl, b, &bus)),
            None => e.act(|x| x.ailists.add_bus(g, fl, &bus)),
        };
        match done {
            Ok(()) => {
                // (the new bus is the chosen one)
                e.bus = f
                    .bus
                    .or_else(|| {
                        e.now
                            .ailists
                            .groups()
                            .get(g)
                            .and_then(|x| x.fleets.get(fl))
                            .and_then(|x| x.buses.len().checked_sub(1))
                    })
                    .map(|b| (fl, b));
                return None;
            }
            Err(err) => f.error = Some(error_text(&err)),
        }
    }
    Some(Dialog::Bus(f))
}

/// The window that picks the AI group of a tour.
pub fn tour_group(
    l: &mut Launcher,
    e: &mut Editor,
    file: String,
    tour: usize,
    mut sel: Option<usize>,
) -> Option<Dialog> {
    let at = e.now.ttls.iter().position(|t| t.file == file)?;
    let number = e.now.ttls[at]
        .doc
        .tours()
        .get(tour)?
        .number
        .trim()
        .to_string();
    let line = Path::new(&file)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let note = omsi_ui::tr("Line %{l}, tour %{t}")
        .replace("%{l}", &line)
        .replace("%{t}", &number);
    let inside = super::super::start::dialog(l, 560.0, 520.0, "Change AI group", &note);
    let (buttons, body) = super::super::start::dialog_foot(l, inside);
    let groups = e.now.ailists.groups().to_vec();
    let now = e.now.ttls[at].doc.tours()[tour].ai_group.trim().to_string();
    let rows: Vec<(String, String, Status)> = groups
        .iter()
        .map(|g| {
            let sub = match g.kind {
                GroupKind::Depot => many(
                    g.fleets.iter().map(|f| f.buses.len()).sum(),
                    "1 bus",
                    "%{n} buses",
                ),
                _ => many(g.vehicles.len(), "1 vehicle", "%{n} vehicles"),
            };
            (g.name.trim().to_string(), sub, Status::Found)
        })
        .collect();
    let list_r = Rect::new(body.x, body.y, body.w, body.h - 6.0);
    if let Some(i) = entries(
        l,
        "ai-edit-tour-groups",
        list_r,
        &rows,
        sel.unwrap_or(usize::MAX),
    ) {
        sel = Some(i);
    }
    let ok_r = Rect::new(buttons.right() - 140.0, buttons.y, 140.0, buttons.h);
    let cancel_r = Rect::new(ok_r.x - 118.0, buttons.y, 110.0, buttons.h);
    if button(
        l,
        "ai-edit:tour-group-cancel",
        cancel_r,
        "Cancel",
        None,
        ButtonKind::Normal,
        true,
        "Closes this window without changing anything.",
    ) {
        return None;
    }
    let pick = sel
        .and_then(|i| groups.get(i))
        .map(|g| g.name.trim().to_string());
    let ready = pick.as_ref().is_some_and(|p| !p.eq_ignore_ascii_case(&now));
    if button(
        l,
        "ai-edit:tour-group-ok",
        ok_r,
        "Apply",
        Some("ok"),
        ButtonKind::Primary,
        ready,
        "The tour takes its buses from the chosen group; the line's file is changed on Save.",
    ) {
        if let Some(p) = pick {
            match e.act_ttl(|x| x.ttls[at].doc.set_group(tour, &p)) {
                Ok(()) => return None,
                Err(_) => l
                    .state
                    .set_status("This change cannot be made to the line.", true),
            }
        }
    }
    Some(Dialog::TourGroup { file, tour, sel })
}
