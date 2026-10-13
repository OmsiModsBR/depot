//! The LAN session and voice chat, each frame.

use super::*;

/// The LAN session and what the game keeps of the other players in it. None of it does
/// anything while multiplayer is switched off (`docs/removed/MULTIPLAYER_DISABLED.md`).
#[derive(Default)]
pub(crate) struct Online {
    /// The session itself, with the chat line.
    pub lan: Option<omsi_net::LanSession>,
    /// The other players' buses (drawn and heard like AI vehicles), the sync tables and the
    /// host's world as far as it was taken over.
    pub remotes: lan::LanGame,
    /// Positional voice through GreenTeaSpeak in a session (`voice`).
    pub voice: Option<crate::voice::Voice>,
    /// Other players on foot whose avatars are drawn (their ids).
    pub remote_walkers: Vec<u32>,
    /// The player on foot is in this other player's bus (see `lan`: drawn from inside).
    pub inside_remote: Option<u32>,
    /// A dedicated server said we administer it (`admin`).
    pub is_admin: bool,
    /// The copies of objects the LAN host made in the editor, as this client shows them.
    pub remote_added: std::collections::HashMap<i64, crate::scene::TileGpu>,
}

impl App {
    /// Whether this game joined another's LAN session (the host then decides the weather,
    /// the clock and the map's edits).
    pub(crate) fn is_lan_client(&self) -> bool {
        self.online.lan.as_ref().is_some_and(|l| l.role == omsi_net::Role::Client)
    }

    /// Whether this game is in a LAN session at all (hosting or joined).
    pub(crate) fn in_lan_session(&self) -> bool {
        self.online.lan.is_some()
    }

    /// The weather `spec` for the players, when this game hosts a session.
    pub(crate) fn share_weather_as_host(&mut self, spec: &str) {
        if let Some(l) = self.online.lan.as_mut().filter(|l| l.role == omsi_net::Role::Host) {
            l.set_weather(spec);
        }
    }

    /// Whether a dedicated server said we administer it (`admin`).
    pub(crate) fn is_server_admin(&self) -> bool {
        self.online.is_admin
    }

    /// Whether this game hosts a LAN session.
    pub(crate) fn is_lan_host(&self) -> bool {
        self.online.lan.as_ref().is_some_and(|l| l.role == omsi_net::Role::Host)
    }

    /// LAN play, once a frame: send our bus, take in the others', and keep a drawn
    /// vehicle for each of them.
    pub(crate) fn tick_lan(&mut self, dt: f32) {
        let walker = self.walker_pose();
        let radio_keyed = self.voice_radio_held();
        let Some(lan) = self.online.lan.as_mut() else {
            // (the session is over: the plugin is told so)
            self.online.voice = None;
            return;
        };
        let duty = self
            .duty
            .as_ref()
            .map(|d| &d.trips[d.trip_index])
            .map(|t| (t.line.as_str(), t.terminus.as_str()));
        let frame = lan::Frame {
            audio: self.audio.as_ref(),
            listener: self.camera.as_ref().map(|c| c.position),
            muffled: self.in_cab || self.online.inside_remote.is_some(),
            riders: self.humans.as_ref().map(|h| h.riding()).unwrap_or(0),
            clock: Some(&self.clock),
            tour: self.duty.as_ref().map(|d| format!("{}/{}", d.line, d.tour)),
            walker,
            inside_of: self.online.inside_remote,
            radio_keyed,
        };
        let updates = lan::tick(
            lan,
            &mut self.online.remotes,
            dt,
            &self.args,
            self.player.as_mut(),
            self.world.as_deref(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
            self.traffic.as_mut(),
            self.humans.as_mut(),
            duty,
            &frame,
        );
        for u in updates {
            self.apply_world_update(u);
        }
        let cmds = self.online.lan.as_mut().map(|l| l.take_commands()).unwrap_or_default();
        for (from, text) in cmds {
            self.lan_command(from, &text);
        }
        self.tick_voice(dt);
    }

    /// A command another player's game sent ours (`LanSession::command`).
    pub(crate) fn lan_command(&mut self, from: u32, text: &str) {
        let Some(lan) = self.online.lan.as_ref() else { return };
        let my_id = lan.my_id;
        if let Some(ev) = text.strip_prefix("trigger ") {
            // a switch worked by a passenger of ours: only by one who is in our bus
            let aboard = lan.peers().find(|p| p.pose.id == from).and_then(|p| p.pose.walker).and_then(|w| w.aboard).map(|a| a.owner == my_id).unwrap_or(false);
            if !aboard {
                log::info!("LAN: player {from} asked for switch {ev} of our bus from outside it: ignored");
                return;
            }
            if let Some(p) = self.player.as_mut() {
                log::info!("LAN: player {from} works {ev} in our bus");
                p.vehicle.trigger(ev.trim());
            }
            return;
        }
        // the voice server of the session (`voice`): asked of the host, told by it
        if text == "voice?" {
            if lan.role == omsi_net::Role::Host {
                let answer = crate::voice::VoiceServer::command(crate::voice::hosted().as_ref());
                if let Some(l) = self.online.lan.as_mut() {
                    l.command(from, &answer);
                }
            }
            return;
        }
        if text.starts_with("voice ") {
            if from == 1 {
                if let (Some(v), Some(server)) = (self.online.voice.as_mut(), crate::voice::VoiceServer::parse_command(text)) {
                    v.set_server(server);
                }
            }
            return;
        }
        crate::admin::command(self, from, text);
    }

    /// The voice chat (`voice`), once a frame of a session: started with it when the
    /// settings allow, the host's voice server asked for, the plugin told who is where.
    pub(crate) fn tick_voice(&mut self, dt: f32) {
        let Some(lan) = self.online.lan.as_mut() else {
            self.online.voice = None;
            return;
        };
        // (a dedicated server has nobody to talk at its place; a joining game that lost
        // its host is in no session to talk in)
        if !self.settings.voice_chat || self.args.server.is_some() || !lan.connected {
            self.online.voice = None;
            return;
        }
        let v = self.online.voice.get_or_insert_with(|| crate::voice::Voice::new(crate::voice::DEFAULT_PORT));
        match lan.role {
            omsi_net::Role::Host => {
                // (once: hosted() reads voice.cfg)
                if !v.known() {
                    v.set_server(crate::voice::hosted());
                }
            }
            omsi_net::Role::Client => {
                if lan.connected && v.should_ask(dt) {
                    lan.command(1, "voice?");
                }
            }
        }
        let lan = self.online.lan.as_ref().unwrap();
        let my_bus = self.player.as_ref().map(|p| p.vehicle.position);
        let others = crate::voice::speakers(lan, &self.online.remotes, my_bus);
        let inside = if self.in_cab { Some(lan.my_id) } else { self.online.inside_remote };
        // driving a bus (not on foot): on the company radio automatically
        let on_radio = self.player.is_some() && !self.ego;
        let radio_keyed = on_radio && self.voice_radio_held();
        let listener = self.camera.as_ref().map(|c| crate::voice::Listener {
            at: c.position,
            yaw: c.yaw,
            inside,
            on_radio,
            radio_keyed,
        });
        let me = (lan.my_name.clone(), lan.my_id);
        if let Some(v) = self.online.voice.as_mut() {
            v.tick(dt, (&me.0, me.1), listener, &others);
        }
    }

    /// Is the bindable bus radio key (`voice_radio` in Controls) held right now?
    /// Keyboard chord or a controller button bound to the same action (held while down).
    pub(crate) fn voice_radio_held(&self) -> bool {
        if self.pad_voice_radio {
            return true;
        }
        let held = |a: KeyCode, b: KeyCode| self.keys.contains(&a) || self.keys.contains(&b);
        let chord = omsi_content::input::chord(
            held(KeyCode::ShiftLeft, KeyCode::ShiftRight),
            held(KeyCode::ControlLeft, KeyCode::ControlRight),
            held(KeyCode::AltLeft, KeyCode::AltRight),
        );
        self.game_keys.iter().any(|b| {
            b.action.eq_ignore_ascii_case("voice_radio")
                && b.scan_code != 0
                && b.matches(chord)
                && self.keys.iter().any(|k| crate::keys::dik_code(*k) == Some(b.scan_code))
        })
    }

    /// The host's world as LAN play asks for it: its clock (set or caught up with) and its
    /// weather, for everything that keeps a clock of its own.
    pub(crate) fn apply_world_update(&mut self, u: lan::WorldUpdate) {
        match u {
            lan::WorldUpdate::Clock {
                year,
                day_of_year,
                time,
            } => {
                self.clock.year = year;
                self.clock.day_of_year = day_of_year;
                self.clock.time = time;
                if let Some(t) = self.traffic.as_mut() {
                    t.day_time = time;
                }
            }
            lan::WorldUpdate::Slew(s) => {
                self.clock.time = (self.clock.time + s).clamp(0.0, 86399.999);
                if let Some(t) = self.traffic.as_mut() {
                    t.day_time += s;
                }
            }
            lan::WorldUpdate::Weather(w) => {
                log::info!(
                    "LAN: the host's weather: {}",
                    w.as_deref().unwrap_or("the map's default")
                );
                // (coming over to it as the host does, not at a stroke - the streets stay
                // as wet as they are and dry or wet with it)
                self.change_weather(w, false, 240.0);
            }
            lan::WorldUpdate::Tours(tours) => {
                if let Some(s) = self.schedule.as_mut() {
                    s.set_lan_tours(tours);
                }
            }
        }
        if let Some(p) = self.player.as_mut() {
            p.vehicle.host.clock = self.clock.clone();
        }
    }
}
