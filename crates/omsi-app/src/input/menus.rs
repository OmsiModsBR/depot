//! The game menu and its lists: opening and closing them, moving through them and picking
//! an entry.

use crate::core::app::App;

/// The game menu on a server (`--lan-join https://…`): the world's clock and weather are the
/// server's, and the way out leaves the server.
pub(crate) const SERVER_GAME_MENU: [(&str, &str); 8] = [
    ("resume", "Resume"),
    ("options", "Options..."),
    ("camera", "Camera..."),
    ("vehicle", "Vehicle options..."),
    ("world", "World options..."),
    ("map", "City map"),
    ("shot", "Screenshot"),
    ("quit", "Leave the server"),
];

/// Whether the session was started on a server (`--lan-join https://…`).
/// Without LAN play the game joins nothing (`--lan-join` is ignored).
pub(crate) fn on_server(args: &crate::core::cli::Args) -> bool {
    {
        let _ = args;
        false
    }
}

/// The game menu's lines for a session with these arguments.
pub(crate) fn game_menu_for(
    args: &crate::core::cli::Args,
) -> &'static [(&'static str, &'static str)] {
    if on_server(args) {
        &SERVER_GAME_MENU
    } else {
        &GAME_MENU
    }
}

/// The lines of the game menu: (what, label). What can be set is on the pages behind
/// "Options", "Vehicle options" and "World options" (see `game_lists`).
pub const GAME_MENU: [(&str, &str); 15] = [
    ("resume", "Resume"),
    ("options", "Options..."),
    ("controls", "Controls..."),
    // (the driver's view - seat, field of view, head movement - straight from the pause
    // menu: it is what is changed most while driving, #908)
    ("camera", "Camera..."),
    ("vehicle", "Vehicle options..."),
    ("world", "World options..."),
    ("map", "City map"),
    ("duty", "Line and tour..."),
    ("skipstop", "Skip the next stop"),
    ("endduty", "End the tour"),
    ("save", "Save the situation"),
    ("saveslot", "Save to a new slot"),
    ("load", "Load the quicksave"),
    ("shot", "Screenshot"),
    ("quit", "End the session"),
];

impl App {
    /// What the cursor points at, for the HUD. Recomputed every frame: the head turns and
    /// the bus moves under a cursor that is standing still.
    /// Open the game menu: the simulation pauses (not in a LAN session, which runs on
    /// for the other players).
    pub(crate) fn open_game_menu(&mut self) {
        self.menu_prev_pause = self.paused;
        // the menu takes the keys, their key-ups too: what is held now is let go here, or a
        // steering key let go in the menu went on turning the wheel to full lock once the
        // menu closed (a throttle key went on accelerating, a door button stayed pressed)
        self.release_vehicle_keys();
        if !self.in_lan_session() {
            self.paused = true;
        }
        self.game_menu = Some(0);
        self.menu_top = None;
        self.menu_kbd = true;
        self.menu_drag = None;
    }

    pub(crate) fn close_game_menu(&mut self) {
        self.key_capture = None;
        if self.menu_edit_icao {
            if let Some(w) = self.window.as_ref() {
                w.set_ime_allowed(false);
            }
            self.menu_edit_icao = false;
            self.menu_edit = None;
        }
        self.game_menu = None;
        self.menu_top = None;
        self.paused = self.menu_prev_pause;
    }

    /// Show one of the menu's lists in the chooser (see `game_lists`).
    pub(crate) fn open_list(&mut self, kind: crate::game_lists::ListKind) {
        crate::game_lists::forget_page_titles();
        self.dropdown = None;
        self.admin_list = Some(crate::game_lists::items(self, &kind));
        self.list_kind = Some(kind);
        // (on its first line, not on a heading)
        self.chooser = Some(if self.is_heading(0) {
            self.chooser_next(0, 1)
        } else {
            0
        });
    }

    /// Line `k` of the list shown heads the lines under it (`game_lists::HEADING`).
    pub(crate) fn is_heading(&self, k: usize) -> bool {
        self.admin_list
            .as_ref()
            .and_then(|l| l.get(k))
            .is_some_and(|l| l.1 == crate::game_lists::HEADING)
    }

    /// The line `step` lines on from `sel` (round the list; `n - 1` is one back), over the
    /// headings.
    pub(crate) fn chooser_next(&self, sel: usize, step: usize) -> usize {
        let n = self
            .admin_list
            .as_ref()
            .unwrap_or(&self.vehicle_list)
            .len()
            .max(1);
        let mut k = sel;
        for _ in 0..n {
            k = (k + step) % n;
            if !self.is_heading(k) {
                break;
            }
        }
        k
    }

    /// Left or Right on line `k` of a list, or a click on the arrows round its value: its
    /// setting one step down (`-`) or up (`+`), see `game_lists::ADJUST`; other lines stay.
    pub(crate) fn chooser_adjust(&mut self, k: usize, dir: &str) {
        let Some(action) = self
            .admin_list
            .as_ref()
            .and_then(|l| l.get(k))
            .and_then(|l| l.1.strip_suffix(crate::game_lists::ADJUST))
            .map(|a| format!("{a} {dir}"))
        else {
            return;
        };
        // (run as a pick of the line, with the step in place of the mark)
        if let Some(l) = self.admin_list.as_mut().and_then(|l| l.get_mut(k)) {
            l.1 = action;
        }
        self.chooser_pick(k);
    }

    pub(crate) fn start_icao_edit(&mut self) {
        self.menu_edit = Some(String::new());
        self.menu_edit_icao = true;
        if let Some(w) = self.window.as_ref() {
            w.set_ime_allowed(true);
        }
    }

    pub(crate) fn apply_icao_edit(&mut self) {
        let code = self
            .menu_edit
            .take()
            .unwrap_or_default()
            .trim()
            .to_ascii_uppercase();
        self.menu_edit_icao = false;
        if let Some(w) = self.window.as_ref() {
            w.set_ime_allowed(false);
        }
        if code.len() == 4 && code.chars().all(|c| c.is_ascii_alphabetic()) {
            self.settings.metar_station = code.clone();
            crate::game_lists::remember_setting("metar_station", &code);
            self.metar_rx = None;
            self.metar_once = false;
            self.metar_next = 0.0;
            self.service_msg = Some((format!("METAR source: {code}"), 3.0));
        } else if !code.is_empty() {
            self.service_msg = Some(("ICAO must be exactly 4 letters".into(), 3.0));
        }
        self.refresh_list();
    }

    /// Set the clock to the time typed (digits: hh, hhmm or hhmmss; what is missing is 0).
    pub(crate) fn apply_time_edit(&mut self) {
        let Some(d) = self.menu_edit.take() else {
            return;
        };
        if !d.is_empty() {
            let mut c = d.clone();
            while c.len() < 6 {
                c.push('0');
            }
            let n = |a: usize| c[a..a + 2].parse::<i64>().unwrap_or(0);
            let (h, m, sec) = (n(0), n(2), n(4));
            if h > 23 || m > 59 || sec > 59 {
                self.service_msg = Some((
                    omsi_ui::tr("%{time} is no time of day")
                        .replace("%{time}", &format!("{:02}:{:02}:{:02}", h, m, sec)),
                    3.0,
                ));
            } else if self.is_lan_client() {
                self.service_msg = Some(("In a LAN session the host sets the clock".into(), 3.0));
            } else if self.real_time_locked() {
                self.service_msg = Some((
                    "The time cannot be changed while the real-time sync is on".into(),
                    3.0,
                ));
            } else {
                let t = self.clock.time;
                let day_start = t - t.rem_euclid(86400.0);
                self.shift_clock(day_start + (h * 3600 + m * 60 + sec) as f64 - t);
                self.service_msg = Some((
                    omsi_ui::tr("Clock: %{time}")
                        .replace("%{time}", &format!("{:02}:{:02}:{:02}", h, m, sec)),
                    3.0,
                ));
            }
        }
        self.refresh_list();
    }

    /// The open list made again from what it shows (a value changed), the chosen line kept.
    pub(crate) fn refresh_list(&mut self) {
        let Some(kind) = self.list_kind.clone() else {
            return;
        };
        let keep = self.chooser;
        self.open_list(kind);
        if let (Some(k), Some(l)) = (keep, self.admin_list.as_ref()) {
            self.chooser = Some(k.min(l.len().saturating_sub(1)));
        }
    }

    /// A settings window (options, vehicle, world) is open.
    pub(crate) fn settings_list(&self) -> bool {
        use crate::game_lists::ListKind;
        self.chooser.is_some()
            && matches!(
                self.list_kind,
                Some(
                    ListKind::Options(_)
                        | ListKind::Vehicle(_)
                        | ListKind::World(_)
                        | ListKind::Controls
                        | ListKind::Keyboard(_)
                        | ListKind::ControllerDevices(_)
                        | ListKind::Controller(..)
                        | ListKind::ControllerAxis(..)
                        | ListKind::ControllerButtonSettings(..)
                )
            )
    }

    /// The open list is closed: back to the game menu.
    pub(crate) fn close_list(&mut self) {
        self.key_capture = None;
        self.dropdown = None;
        if self.menu_edit_icao {
            if let Some(w) = self.window.as_ref() {
                w.set_ime_allowed(false);
            }
        }
        self.menu_edit_icao = false;
        self.menu_edit = None;
        self.chooser = None;
        self.admin_list = None;
        self.list_kind = None;
        self.menu_top = None;
    }

    /// Show page `i` of the open settings window.
    pub(crate) fn settings_tab(&mut self, i: usize) {
        use crate::game_lists::ListKind;
        let next = match self.list_kind.as_ref() {
            Some(ListKind::Options(_)) => ListKind::Options(i),
            Some(ListKind::Vehicle(_)) => ListKind::Vehicle(i),
            Some(ListKind::World(_)) => ListKind::World(i),
            Some(ListKind::Keyboard(_)) => ListKind::Keyboard(i.min(1)),
            Some(ListKind::ControllerDevices(_)) => ListKind::ControllerDevices(i.min(2)),
            Some(ListKind::Controller(name, _)) => ListKind::Controller(name.clone(), i.min(3)),
            _ => return,
        };
        self.menu_top = None;
        self.menu_edit = None;
        self.key_capture = None;
        self.open_list(next);
    }

    /// The next (or previous) page of the open settings window, round the ends.
    pub(crate) fn settings_tab_step(&mut self, forward: bool) {
        let Some(kind) = self.list_kind.clone() else {
            return;
        };
        let Some((titles, at)) = crate::game_lists::page_titles(self, &kind) else {
            return;
        };
        let n = titles.len().max(1);
        self.settings_tab(if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        });
    }

    /// A click on the sidebar of a settings window: page `i`, or (the last box) the way back.
    pub(crate) fn settings_side_click(&mut self, i: usize) {
        let Some(kind) = self.list_kind.clone() else {
            return;
        };
        let n = crate::game_lists::page_titles(self, &kind)
            .map(|t| t.0.len())
            .unwrap_or(0);
        if i < n {
            self.settings_tab(i);
        } else {
            self.key_capture = None;
            if let Some(parent) = crate::game_lists::run(self, &kind, "back") {
                self.menu_top = None;
                self.open_list(parent);
            } else {
                self.close_list();
            }
        }
    }

    /// Change the value of line `k` of the open settings window as `mv` says.
    pub(crate) fn list_adjust(&mut self, k: usize, mv: crate::game_lists::Move) {
        use crate::game_lists::ListKind;
        let Some(kind) = self.list_kind.clone() else {
            return;
        };
        if !matches!(
            kind,
            ListKind::Options(_)
                | ListKind::World(_)
                | ListKind::ControllerDevices(_)
                | ListKind::Controller(..)
                | ListKind::ControllerAxis(..)
                | ListKind::ControllerButtonSettings(..)
        ) {
            return;
        }
        let Some(action) = self
            .admin_list
            .as_ref()
            .and_then(|l| l.get(k))
            .map(|x| x.1.clone())
        else {
            return;
        };
        let slider = crate::game_lists::is_slider(action.split(' ').next().unwrap_or(""));
        crate::game_lists::LIST_DIRTY.store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(next) = crate::game_lists::run_move(self, &kind, &action, mv) {
            // (a slider dragged sends the same value many times over: the list stays)
            if slider
                && !crate::game_lists::LIST_DIRTY.swap(false, std::sync::atomic::Ordering::Relaxed)
            {
                return;
            }
            self.open_list(next);
            let last = self
                .admin_list
                .as_ref()
                .map(|l| l.len().saturating_sub(1))
                .unwrap_or(0);
            self.chooser = Some(k.min(last));
        }
    }

    /// A click on the control of line `k` (a slider's track, a stepper), `fx` of the way
    /// along it from the left.
    /// True when the control is a slider (which the mouse button then holds: it follows the cursor).
    pub(crate) fn list_click(&mut self, k: usize, fx: f32) -> bool {
        use crate::game_lists::Move;
        let Some(action) = self
            .admin_list
            .as_ref()
            .and_then(|l| l.get(k))
            .map(|x| x.1.clone())
        else {
            return false;
        };
        let verb = action.split(' ').next().unwrap_or("");
        let slider = crate::game_lists::is_slider(verb);
        let mv = if slider {
            Move::To(fx)
        } else if fx < 0.5 {
            Move::Dec
        } else {
            Move::Inc
        };
        self.list_adjust(k, mv);
        slider
    }

    /// Entry `i` of the open drop-down is taken: done, and the window's rows shown again
    /// with the new value.
    pub(crate) fn dropdown_pick(&mut self, i: usize) {
        let Some(d) = self.dropdown.take() else {
            return;
        };
        let Some((_, action)) = d.items.get(i).cloned() else {
            return;
        };
        crate::game_lists::dropdown_apply(self, &action);
        if let Some(kind) = self.list_kind.clone() {
            self.open_list(kind);
            let last = self
                .admin_list
                .as_ref()
                .map(|l| l.len().saturating_sub(1))
                .unwrap_or(0);
            self.chooser = Some(d.row.min(last));
        }
    }

    /// A line's tours are open, with the stops of the tour chosen beside them.
    pub(crate) fn tours_list(&self) -> bool {
        self.chooser.is_some()
            && matches!(self.list_kind, Some(crate::game_lists::ListKind::Tours(..)))
    }

    /// The trip of the chosen tour leaving before (or after) the one chosen, as OMSI's
    /// timetable steps through the times of a tour (the tour itself stays).
    pub(crate) fn trip_step(&mut self, forward: bool) {
        let k = self.chooser.unwrap_or(0);
        let (Some((line, tour)), Some((_, _, trip, trips))) = (
            crate::game_lists::tour_at(self, k),
            crate::game_lists::tour_choice(self, k),
        ) else {
            return;
        };
        let to = if forward {
            (trip + 1).min(trips.saturating_sub(1))
        } else {
            trip.saturating_sub(1)
        };
        if to != trip {
            self.pane_scroll = None;
            self.list_kind = Some(crate::game_lists::ListKind::Tours(
                line,
                Some((tour, 0, to)),
            ));
        }
    }

    /// The stop to start the chosen tour from, one on (or back).
    pub(crate) fn tour_stop_step(&mut self, forward: bool) {
        let k = self.chooser.unwrap_or(0);
        let (Some((line, tour)), Some((n, at, trip, _))) = (
            crate::game_lists::tour_at(self, k),
            crate::game_lists::tour_choice(self, k),
        ) else {
            return;
        };
        let to = if forward {
            (at + 1).min(n - 1)
        } else {
            at.saturating_sub(1)
        };
        self.pane_scroll = None;
        self.list_kind = Some(crate::game_lists::ListKind::Tours(
            line,
            Some((tour, to, trip)),
        ));
    }

    /// A click in the timetable beside the tours: stop `i` as the start, or (`usize::MAX`)
    /// the button that starts the trip; `usize::MAX - 1` / `- 2` the trip before / after.
    pub(crate) fn tour_pane_click(&mut self, i: usize) {
        let k = self.chooser.unwrap_or(0);
        // (the arrows beside the time: `usize::MAX - 1` the trip before, `- 2` the next)
        if i == usize::MAX - 1 || i == usize::MAX - 2 {
            self.trip_step(i == usize::MAX - 2);
            return;
        }
        let (Some((line, tour)), Some((n, at, trip, _))) = (
            crate::game_lists::tour_at(self, k),
            crate::game_lists::tour_choice(self, k),
        ) else {
            return;
        };
        if i < n {
            self.pane_scroll = None;
            self.list_kind = Some(crate::game_lists::ListKind::Tours(
                line,
                Some((tour, i, trip)),
            ));
            return;
        }
        crate::game_lists::start_duty_at(self, &line, &tour, trip, at);
        self.chooser = None;
        self.admin_list = None;
        self.list_kind = None;
        self.close_game_menu();
    }

    /// Place the chosen vehicle: in front of the camera in a free or map view, else beside
    /// the vehicle driven (OMSI puts a new vehicle where the map view points).
    pub(crate) fn chooser_pick(&mut self, k: usize) {
        // (a heading is no choice)
        if self.is_heading(k) {
            return;
        }
        // a row of a settings window that drops a list down (the weather preset, the clouds)
        if self.settings_list() {
            let id = self
                .admin_list
                .as_ref()
                .and_then(|l| l.get(k))
                .map(|l| l.1.clone())
                .unwrap_or_default();
            if let Some(d) = crate::game_lists::dropdown_for(self, k, &id) {
                self.dropdown = Some(d);
                self.dd_reveal();
                return;
            }
        }
        self.chooser = None;
        // a list of the menu's (the administration, the options …): done, and the list
        // shown again - or the next one (a line's tours), or back to the menu
        if let Some(list) = self.admin_list.take() {
            let kind = self
                .list_kind
                .take()
                .unwrap_or(crate::game_lists::ListKind::Admin);
            let Some((_, action)) = list.get(k).cloned() else {
                return;
            };
            match crate::game_lists::run(self, &kind, &action) {
                Some(next) => {
                    let keep = next == kind;
                    if !keep {
                        self.menu_top = None;
                    }
                    self.open_list(next);
                    if keep {
                        self.chooser = Some(
                            k.min(
                                self.admin_list
                                    .as_ref()
                                    .map(|l| l.len().saturating_sub(1))
                                    .unwrap_or(0),
                            ),
                        );
                    }
                }
                None if action != "back"
                    && matches!(
                        kind,
                        crate::game_lists::ListKind::Tours(..)
                            | crate::game_lists::ListKind::Numbers
                            | crate::game_lists::ListKind::Destinations
                            | crate::game_lists::ListKind::RouteNumbers
                            | crate::game_lists::ListKind::Hofs
                            | crate::game_lists::ListKind::Spots
                    ) =>
                {
                    self.close_game_menu()
                }
                None => self.menu_top = None,
            }
            return;
        }
        // a vehicle of the list: its livery and depot file are asked for first
        self.menu_top = None;
        let Some((_, bus)) = self.vehicle_list.get(k).cloned() else {
            return;
        };
        self.open_list(crate::game_lists::ListKind::PlaceLivery(bus));
    }

    /// How many lines the menu shows now (the chooser's list, else the game menu's).
    pub(crate) fn menu_len(&self) -> usize {
        match self.chooser {
            Some(_) => self.admin_list.as_ref().unwrap_or(&self.vehicle_list).len(),
            None => self.game_menu_items().len(),
        }
    }
}

impl crate::core::app::App {
    /// The game menu's lines for this session: back to the own bus while walking about,
    /// the administration for a host and a server's admin. What can be set is behind
    /// "Options", "Vehicle options" and "World options".
    pub(crate) fn game_menu_items(&self) -> Vec<(&'static str, &'static str)> {
        let mut v: Vec<(&'static str, &'static str)> = game_menu_for(&self.args).to_vec();
        let mut at = 1;
        if self.on_foot.is_some() && self.player.is_some() {
            v.insert(at, ("tobus", "Back to my bus"));
            at += 1;
        }
        // without a bus of one's own: no line to drive
        if self.player.is_none() {
            v.retain(|x| x.0 != "duty");
        }
        // ending the route is offered only while there is one, skipping a stop while its
        // trip still has one to come
        if self.duty.is_none() {
            v.retain(|x| x.0 != "endduty");
        }
        if !self.duty.as_ref().is_some_and(|d| d.stop_to_skip()) {
            v.retain(|x| x.0 != "skipstop");
        }
        if self.navigator.is_none() {
            v.retain(|x| x.0 != "map");
        }
        let host = self.is_lan_host();
        // the server code: a line to copy it, right under "World options" (only in a LAN session or on a server)
        if self.in_lan_session() || on_server(&self.args) {
            if let Some(w) = v.iter().position(|x| x.0 == "world") {
                v.insert(w + 1, ("copycode", "Copy server code"));
                at = at.max(w + 2);
            }
        }
        if host || self.is_server_admin() {
            let before_quit = v
                .iter()
                .position(|x| x.0 == "quit")
                .unwrap_or(v.len())
                .max(at);
            v.insert(before_quit, ("admin", "Administration..."));
        }
        v
    }

    /// The ids of the game menu's lines that are greyed out and cannot be chosen now.
    pub(crate) fn menu_disabled_ids(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether line `k` of the game menu is greyed out.
    pub(crate) fn menu_item_off(&self, k: usize) -> bool {
        self.chooser.is_none()
            && self
                .game_menu_items()
                .get(k)
                .is_some_and(|m| self.menu_disabled_ids().contains(&m.0))
    }

    /// The next line up or down from `from` that can be chosen (round the ends; the greyed-out
    /// lines are skipped).
    pub(crate) fn menu_step(&self, from: usize, n: usize, down: bool) -> usize {
        let mut k = from;
        for _ in 0..n {
            k = if down { (k + 1) % n } else { (k + n - 1) % n };
            if !self.menu_item_off(k) {
                return k;
            }
        }
        from
    }
}
