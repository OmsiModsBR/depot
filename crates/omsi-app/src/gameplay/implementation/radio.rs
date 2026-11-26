//! The bus radio as live internet radio: reading and saving the station files, the
//! stations the plugins and the map bring, and the streams tuned in while the radio plays.
//! The rules of tuning and the list's text are in `domain/radio.rs`.

use super::super::domain::radio::*;
use glam::Vec3;
use omsi_audio::stream::StreamBuf;
use omsi_audio::{AudioEngine, VoiceId, VoiceParams};
use std::path::PathBuf;
use std::sync::Arc;

/// The player's own stations (`radio.cfg`; the default list while there is none), as the
/// launcher's Sound settings show them to edit (#857).
pub(crate) fn own_stations() -> Vec<(String, String)> {
    station_lines(
        &config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(default_text),
    )
}
/// Keep `list` as the player's own stations in `radio.cfg` (see `with_stations`): the game
/// tunes in to them when it starts.
pub(crate) fn save_stations(list: &[(String, String)]) -> std::io::Result<()> {
    let Some(p) = config_path() else {
        return Err(std::io::Error::other("no home folder"));
    };
    let text = std::fs::read_to_string(&p).unwrap_or_else(|_| default_text());
    std::fs::create_dir_all(p.parent().unwrap_or(&p))?;
    std::fs::write(&p, with_stations(&text, list))
}
fn config_path() -> Option<PathBuf> {
    Some(omsi_cfg::paths::data_dir()?.join("radio.cfg"))
}
/// The stream addresses an OMSI radio plugin keeps in its text files under `plugins`
/// (SuperRadio's `.opl` and its lists; whatever the layout, a line with an http(s) address is
/// a station, named by the text before the address or else by its host): "openOMSI does not
/// load the stations I defined in the .opl".
fn plugin_stations(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut files = Vec::new();
    let mut walk = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = walk.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 2 {
                walk.push((p, depth + 1));
            } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                matches!(
                    x.to_ascii_lowercase().as_str(),
                    "opl" | "cfg" | "ini" | "txt" | "m3u" | "pls"
                )
            }) {
                files.push(p);
            }
        }
    }
    files.sort();
    for f in files {
        let Ok(bytes) = std::fs::read(&f) else {
            continue;
        };
        if bytes.len() > 1 << 20 {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            let Some(at) = line.find("http://").or_else(|| line.find("https://")) else {
                continue;
            };
            let url: String = line[at..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !matches!(c, '"' | '\'' | ';' | ',' | '|'))
                .collect();
            let before = line[..at]
                .trim()
                .trim_end_matches(['=', ':', '|', ',', ';', '"', '\'', '\t'])
                .trim();
            let before = before.trim_start_matches(|c: char| {
                c.is_ascii_digit() || matches!(c, '.' | ')' | '-' | ' ')
            });
            let name = if before.is_empty() || before.len() > 60 {
                url.split('/').nth(2).unwrap_or(&url).to_string()
            } else {
                before.to_string()
            };
            if url.len() > 12 && !out.iter().any(|(_, u): &(String, String)| u == &url) {
                out.push((name, url));
            }
        }
    }
    out
}
/// A map's own stations: `radio.cfg` beside its global.cfg, written like the player's -
/// the stations a bus hears where the map plays, and the frequencies they are on there.
/// Its volume line counts for nothing: how loud the radio is stays the player's business.
fn map_stations(map_cfg: &std::path::Path) -> (Vec<(String, String)>, Frequencies) {
    let Some(dir) = map_cfg.parent() else {
        return Default::default();
    };
    let Ok(bytes) = omsi_cfg::vfs::read(&dir.join("radio.cfg")) else {
        return Default::default();
    };
    let (stations, _, frequencies) = parse_stations(&omsi_cfg::decode_text(&bytes));
    (stations, frequencies)
}
pub struct Radio {
    /// What the buttons play: the map's stations, then the player's.
    pub stations: Vec<(String, String)>,
    /// The player's own (radio.cfg and the radio plugins' lists).
    pub own: Vec<(String, String)>,
    /// The frequencies the stations are on: the player's file's, and the map's over them.
    pub frequencies: Frequencies,
    pub own_frequencies: Frequencies,
    /// The map the list was made for, and whether the list changed while a station played.
    pub map: String,
    pub relisted: bool,
    pub volume: f32,
    /// Shift+R: how far the list is turned from the bus's own station numbers.
    pub offset: usize,
    pub playing: Option<Playing>,
}
pub struct Playing {
    station: usize,
    buf: Arc<StreamBuf>,
    voice: VoiceId,
    /// The status last shown on screen (the song, "no signal" ...).
    shown: String,
    /// The line a text display runs through (see `Radio::display_text`), and since when.
    line: (String, std::time::Instant),
}
impl Radio {
    pub fn load(omsi_root: &std::path::Path) -> Radio {
        let mut stations: Vec<(String, String)>;
        let mut volume = 0.7f32;
        let mut frequencies = Frequencies::new();
        let path = config_path();
        match path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(text) => {
                let (list, vol, on) = parse_stations(&text);
                stations = list;
                volume = vol.unwrap_or(volume);
                frequencies = on;
            }
            None => {
                stations = DEFAULT_STATIONS
                    .iter()
                    .map(|(n, u)| (n.to_string(), u.to_string()))
                    .collect();
                if let Some(p) = &path {
                    let _ = std::fs::create_dir_all(p.parent().unwrap_or(p));
                    let _ = std::fs::write(p, default_text());
                }
            }
        }
        // the stations of OMSI's radio plugins (SuperRadio and the like), after the own ones
        let own = stations.len();
        for (name, url) in plugin_stations(&omsi_root.join("plugins")) {
            if !stations.iter().any(|(_, u)| u.eq_ignore_ascii_case(&url)) {
                stations.push((name, url));
            }
        }
        if stations.len() > own {
            log::info!(
                "radio: {} stations of the OMSI radio plugins added",
                stations.len() - own
            );
        }
        if !stations.is_empty() {
            log::info!("radio: {} stations", stations.len());
        }
        Radio {
            stations: stations.clone(),
            own: stations,
            frequencies: frequencies.clone(),
            own_frequencies: frequencies,
            map: String::new(),
            relisted: false,
            volume,
            offset: 0,
            playing: None,
        }
    }

    /// The frequency the station that plays is on where the bus is (`94.6 MHz`), for a
    /// radio's display: of the places the station's frequencies are given for, the nearest
    /// to (x, y) counts; a frequency without a place is the station's everywhere else.
    /// None for a station without frequencies - the display then keeps what its script
    /// writes.
    pub fn frequency(&self, x: f64, y: f64) -> Option<String> {
        let p = self.playing.as_ref()?;
        let on = self
            .frequencies
            .get(&self.stations.get(p.station)?.1.to_ascii_lowercase())?;
        Some(format!("{} MHz", frequency_at(on, x, y)?))
    }

    /// The map that plays (its global.cfg under `root`): its radio.cfg's stations come
    /// first, on the first station buttons, and the player's own follow - less those the
    /// map names too.
    pub fn set_map(&mut self, root: &std::path::Path, map: &str) {
        if self.map == map {
            return;
        }
        self.map = map.to_string();
        let (mut list, on) = map_stations(&omsi_cfg::resolve_path(root, map));
        if !list.is_empty() {
            log::info!(
                "radio: {} stations of the map, {} with their frequencies",
                list.len(),
                on.len()
            );
        }
        self.frequencies = self.own_frequencies.clone();
        self.frequencies.extend(on);
        for s in &self.own {
            if !list.iter().any(|(_, u)| u.eq_ignore_ascii_case(&s.1)) {
                list.push(s.clone());
            }
        }
        if list != self.stations {
            self.stations = list;
            self.offset = 0;
            self.relisted = true;
        }
    }

    /// The station the bus's radio is tuned to, None while it is off.
    fn wanted(&self, v: &omsi_sim::VehicleInstance) -> Option<usize> {
        if self.stations.is_empty() {
            return None;
        }
        let button = match v.var("SndExt_Radio") {
            Some(x) if x >= 0.5 => Some(x.round() as usize - 1),
            _ => None,
        };
        let button = button.or_else(|| v.var("Snd_Radio").filter(|x| *x >= 0.5).map(|_| 0));
        button.map(|b| (b + self.offset) % self.stations.len())
    }

    /// Follow the player's bus radio; a text for the screen when the station or its song
    /// changes.
    pub fn update(
        &mut self,
        audio: &AudioEngine,
        v: &omsi_sim::VehicleInstance,
        inside: bool,
    ) -> Option<String> {
        let wanted = self.wanted(v);
        // (another map's list: the same button is another station now)
        if std::mem::take(&mut self.relisted) {
            self.stop(audio);
        }
        if self.playing.as_ref().map(|p| p.station) != wanted {
            self.stop(audio);
            if let Some(station) = wanted {
                let (name, url) = &self.stations[station];
                log::info!("radio: station {} {name} ({url})", station + 1);
                let buf = omsi_audio::radio::open(url);
                let voice = audio.play_stream(
                    buf.clone(),
                    VoiceParams {
                        gain: 0.0,
                        ..Default::default()
                    },
                );
                self.playing = Some(Playing {
                    station,
                    buf,
                    voice,
                    shown: String::new(),
                    line: (String::new(), std::time::Instant::now()),
                });
            }
        }
        let p = self.playing.as_mut()?;
        // the volume knob, where the radio has one (0..1, some go to 2)
        let knob = v
            .var("SndVol_Radio")
            .map(|x| x.clamp(0.0, 2.0))
            .unwrap_or(1.0);
        let gain = self.volume * knob;
        // heard through the bodywork from outside
        let pos = v.position.as_vec3() + Vec3::Z * 1.5;
        let params = if inside {
            VoiceParams {
                gain,
                ..Default::default()
            }
        } else {
            VoiceParams {
                gain,
                position: Some(pos),
                range: 4.0,
                lowpass_hz: 700.0,
                ..Default::default()
            }
        };
        audio.set_params(p.voice, params);
        let status = p.buf.status();
        if status != p.shown && !status.is_empty() && status != "connecting …" {
            p.shown = status.clone();
            let line = format!(
                "Radio {}: {} - {status}",
                p.station + 1,
                self.stations[p.station].0
            );
            log::info!("{line}");
            return Some(line);
        }
        None
    }

    /// What a radio with a text display shows now (`VehicleInstance::radio_text`): the
    /// station and its song, running through the line where they do not fit. Empty while
    /// the radio is off; None without stations (the scripts' own texts then stay).
    pub fn display_text(&mut self) -> Option<String> {
        if self.stations.is_empty() {
            return None;
        }
        let Some(p) = self.playing.as_mut() else {
            return Some(String::new());
        };
        let line = display_line(&self.stations[p.station].0, &p.buf.status());
        if p.line.0 != line {
            p.line = (line, std::time::Instant::now());
        }
        Some(marquee(
            &p.line.0,
            DISPLAY_WIDTH,
            p.line.1.elapsed().as_secs_f32(),
        ))
    }

    /// Shift+R: the next station of the list on every button.
    pub fn next_station(&mut self) -> String {
        if self.stations.is_empty() {
            return "No radio stations (see ~/.depot/radio.cfg)".into();
        }
        self.offset = (self.offset + 1) % self.stations.len();
        match &self.playing {
            Some(p) => {
                let next = (p.station + 1) % self.stations.len();
                format!("Radio {}: {}", next + 1, self.stations[next].0)
            }
            None => "The bus radio is off (switch it on in the cockpit)".into(),
        }
    }

    pub fn stop(&mut self, audio: &AudioEngine) {
        if let Some(p) = self.playing.take() {
            p.buf.close();
            audio.stop(p.voice);
        }
    }
}
