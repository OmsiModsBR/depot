//! The launcher's side of the preview: the helper started while a page shows the bus, the
//! picture it sent, and the kept picture shown until then.

use super::super::domain::{
    look::Look,
    message::{Cmd, Msg},
    shape::Shape,
};
use super::{helper::*, kept::*, link::*};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// The mark the in-game "Add vehicle" window's helper keeps (`Preview::for_game`).
pub(super) const GAME_MARK: &str = "showroom-placing-game.txt";

/// Set for a helper that keeps no pictures (the game's).
pub(super) const NO_KEEP: &str = "DEPOT_PREVIEW_NO_KEEP";

/// How long the preview stays drawn by the helper after no page shows it.
const LINGER: f32 = 2.0;

/// The launcher's side of the preview.
pub struct Preview {
    wanted: Option<Look>,
    pub link: Option<Link>,
    sent: (Option<Look>, Option<(u32, u32)>, f32),
    size: Option<(u32, u32)>,
    pub focus_x: f32,
    /// A picture not yet on the GPU (`take_frame`), and the bus it shows.
    frame: Option<(u32, u32, Vec<u8>)>,
    /// What the picture on the GPU shows: its width over its height, and the bus (None while
    /// there is none for the bus wanted).
    pub aspect: f32,
    showing: Option<u64>,
    /// The picture shown is the one kept from an earlier time, not yet the helper's.
    pub from_cache: bool,
    cache: Option<((u64, Shape), Receiver<Option<(u32, u32, Vec<u8>)>>)>,
    cache_for: Option<(u64, Shape)>,
    pub busy: bool,
    pub error: Option<String>,
    /// The helper is running and has drawn the bus wanted.
    pub live: bool,
    unused: f32,
    /// The helper could not be started, or stopped on its own (not tried again for the same
    /// bus).
    pub broken: Option<Look>,
    /// The page shows a picture of the bus, not the live view (`want_picture`).
    picture_only: bool,
    /// The settled picture the helper said it kept last.
    kept: Option<(u64, Shape)>,
    /// The player turned or zoomed the picture of a page that shows one: the live view, asked
    /// for (until another bus is chosen).
    asked: bool,
    /// The graphics interfaces the helper is tried on (`helper_backends`), which one it is
    /// started on, whether the one running has sent a picture yet, and whether one has on
    /// that interface (then it stays: a helper that dies on one bus says nothing of the
    /// interface).
    pub backends: Vec<Option<&'static str>>,
    pub backend: usize,
    pub drew: bool,
    pinned: bool,
    /// When the helper running started, or last sent something (`FIRST_PICTURE_LIMIT`).
    pub heard: Instant,
    /// The file name of the mark this preview's helper keeps of the bus it reads (None: the
    /// common one the page's live helper uses).
    pub mark: Option<&'static str>,
}

impl Default for Preview {
    fn default() -> Self {
        Preview::new()
    }
}

impl Preview {
    pub fn new() -> Preview {
        Preview::with_mark(None)
    }

    /// A preview for the pictures drawn ahead of time: its helper keeps a mark of its own, so
    /// that it and the page's live helper never take each other's for a bus that hung.
    pub fn for_prerender() -> Preview {
        Preview::with_mark(Some("showroom-placing-prerender.txt"))
    }

    /// A preview for the in-game "Add vehicle" window's live view, asked for by the player:
    /// its helper keeps a mark of its own too, and keeps no pictures (the launcher's kept
    /// pictures, its small ones included, are left as they are).
    pub fn for_game() -> Preview {
        Preview::with_mark(Some(GAME_MARK))
    }

    fn with_mark(mark: Option<&'static str>) -> Preview {
        // A mark of the bus a helper was reading, left by the last launcher, says nothing of
        // the bus: its helpers are ended with it (`end_with_launcher`), in the middle of
        // reading one as well. (Within a launcher's session a bus that stops a helper is still
        // found: the next helper reads the mark.)
        static CLEARED: std::sync::Once = std::sync::Once::new();
        static CLEARED_OWN: std::sync::Once = std::sync::Once::new();
        static CLEARED_GAME: std::sync::Once = std::sync::Once::new();
        match mark {
            None => CLEARED.call_once(super::scene::clear_placing_mark),
            Some(GAME_MARK) => CLEARED_GAME.call_once(|| super::scene::clear_mark(GAME_MARK)),
            Some(n) => CLEARED_OWN.call_once(|| super::scene::clear_mark(n)),
        }
        Preview {
            wanted: None,
            link: None,
            sent: (None, None, -1.0),
            size: None,
            focus_x: 0.5,
            frame: None,
            aspect: 16.0 / 9.0,
            showing: None,
            from_cache: false,
            cache: None,
            cache_for: None,
            busy: false,
            error: None,
            live: false,
            unused: 0.0,
            broken: None,
            picture_only: false,
            kept: None,
            asked: false,
            backends: helper_backends(&crate::settings::Settings::load().graphics_api),
            backend: 0,
            drew: false,
            pinned: false,
            heard: Instant::now(),
            mark,
        }
    }

    /// Show this live (a bus, its paint, the time and weather to light it with): the page
    /// whose preview this is (Vehicles), the helper drawing it while the page is open.
    pub fn want(&mut self, look: Look) {
        self.picture_only = false;
        self.set(look);
    }

    /// Show a picture of this: the kept one when there is one, else the one the helper draws
    /// and keeps, and then ends (the live view is the Vehicles page's, or asked for by turning
    /// the picture).
    pub fn want_picture(&mut self, look: Look) {
        self.picture_only = true;
        self.set(look);
    }

    fn set(&mut self, look: Look) {
        if self.wanted.as_ref() == Some(&look) {
            return;
        }
        if self.wanted.as_ref().map(tag) != Some(tag(&look)) {
            // another bus: nothing of the last one stands for it (its kept picture, if there is
            // one, comes in `update`)
            self.showing = None;
            self.live = false;
            self.from_cache = false;
            self.cache_for = None;
            self.cache = None;
            self.asked = false;
        }
        self.wanted = Some(look);
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.asked = true;
        if let Some(l) = self.link.as_ref() {
            let _ = l.cmds.send(Cmd::Orbit(dx, dy));
        }
    }

    pub fn zoom_by(&mut self, k: f32) {
        self.asked = true;
        if let Some(l) = self.link.as_ref() {
            let _ = l.cmds.send(Cmd::Zoom(k));
        }
    }

    /// The helper could not draw the bus wanted (it could not be started, or stopped on its own
    /// on every interface).
    pub fn gave_up(&self) -> bool {
        self.broken.is_some() && self.broken == self.wanted
    }

    /// The bus and shape of the picture the helper kept last.
    pub fn kept(&self) -> Option<(u64, Shape)> {
        self.kept
    }

    /// The page shows a picture, not the live view (see `want_picture`).
    pub fn picture_only(&self) -> bool {
        self.picture_only
    }

    /// A page that shows a picture has it, kept (from an earlier time, or by the helper just
    /// now for this shape): no helper is needed for it.
    pub fn picture_done(&self) -> bool {
        let Some(t) = self.wanted.as_ref().map(tag) else {
            return false;
        };
        let shape = self.size.map(|(w, h)| Shape::of(w, h, self.focus_x));
        self.picture_only
            && !self.asked
            && self.has_picture()
            && (self.from_cache
                || (self.kept.map(|k| k.0) == Some(t) && self.kept.map(|k| k.1) == shape))
    }

    /// The kept picture of the bus wanted, for the shape the preview has, read on a thread
    /// (while the helper has not drawn that bus).
    fn read_cache(&mut self) {
        let (Some(look), Some((w, h))) = (self.wanted.as_ref(), self.size) else {
            return;
        };
        if self.live {
            return;
        }
        let key = (tag(look), Shape::of(w, h, self.focus_x));
        if self.cache_for == Some(key) {
            return;
        }
        self.cache_for = Some(key);
        let (tx, rx) = channel();
        let look = look.clone();
        std::thread::spawn(move || {
            let f = cache_file(&look, key.1);
            let img = image::open(&f)
                .ok()
                .map(|i| i.to_rgba8())
                .map(|i| (i.width(), i.height(), i.into_raw()));
            let _ = tx.send(img);
        });
        self.cache = Some((key, rx));
    }

    /// The window was minimized or covered: the helper ends now, without the moment it is
    /// given after the player leaves a page (nothing on the screen shows the preview then).
    pub fn stop(&mut self) {
        // (stopped at once: the picture it may be writing goes to its place whole or not at
        // all, and the mark of the bus it was reading is the launcher's to clear)
        let mark = self.mark;
        if let Some(mut c) = self.link.as_mut().and_then(|l| l.child.take()) {
            log::info!(
                "launcher: the preview helper is stopped (the window is hidden, or a game runs)"
            );
            let _ = c.kill();
            std::thread::spawn(move || {
                let _ = c.wait();
                clear_own_mark(mark);
            });
        }
        self.unused = self.unused.max(LINGER + 1.0);
        self.update(0.0, None);
    }

    /// Per frame (or per tick while the window is hidden, with `shown` None): `shown` the size
    /// the preview is drawn at this frame (in pixels), None when no page shows it.
    pub fn update(&mut self, dt: f32, shown: Option<(u32, u32)>) {
        if let Some(s) = shown {
            self.size = Some(s);
        }
        self.read_cache();
        // the kept picture, read from its file
        if let Some((key, rx)) = self.cache.as_ref() {
            match rx.try_recv() {
                Ok(img) => {
                    if let (Some(img), false, Some(t)) =
                        (img, self.live, self.wanted.as_ref().map(tag))
                    {
                        if self.cache_for.as_ref() == Some(key) && key.0 == t {
                            self.aspect = img.0 as f32 / img.1.max(1) as f32;
                            self.frame = Some(img);
                            self.showing = Some(t);
                            self.from_cache = true;
                        }
                    }
                    self.cache = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.cache = None,
            }
        }
        // (a page that shows a picture needs the helper only until it has one)
        if shown.is_some() && !self.picture_done() {
            self.unused = 0.0;
        } else {
            self.unused += dt;
        }
        // the helper while a page shows the preview, and a moment after
        if self.unused > LINGER {
            if self.link.take().is_some() {
                log::info!("launcher: the preview is not shown any more, its helper ends");
            }
            self.live = false;
            self.busy = false;
            self.sent = (None, None, -1.0);
            return;
        }
        let Some(look) = self.wanted.clone() else {
            return;
        };
        // (not while the kept picture is still being read: it may be all the page needs)
        let reading = self.picture_only && self.cache.is_some();
        if self.link.is_none()
            && self.broken.as_ref() != Some(&look)
            && shown.is_some()
            && !reading
            && !self.picture_done()
        {
            match Link::start(
                &look.root,
                self.backends.get(self.backend).copied().flatten(),
                self.mark,
            ) {
                Ok(l) => {
                    self.link = Some(l);
                    self.drew = false;
                    self.heard = Instant::now();
                    self.sent = (None, None, -1.0);
                    // (what an earlier helper said holds no more; this one says it anew)
                    self.busy = true;
                    self.error = None;
                }
                Err(e) => {
                    log::warn!("launcher: the preview helper could not be started: {e}");
                    self.error = Some("The 3D preview could not be started".into());
                    self.broken = Some(look.clone());
                }
            }
        }
        let Some(link) = self.link.as_ref() else {
            return;
        };
        if self.sent.0.as_ref() != Some(&look) {
            let _ = link.cmds.send(Cmd::Look(look.clone()));
            self.sent.0 = Some(look.clone());
        }
        if self.size.is_some() && self.sent.1 != self.size {
            let (w, h) = self.size.unwrap_or((16, 16));
            let _ = link.cmds.send(Cmd::Size(w, h));
            self.sent.1 = self.size;
        }
        if (self.sent.2 - self.focus_x).abs() > 1e-4 {
            let _ = link.cmds.send(Cmd::Focus(self.focus_x));
            self.sent.2 = self.focus_x;
        }
        let want = tag(&look);
        let mut ended = false;
        loop {
            let msg = link.msgs.try_recv();
            if msg.is_ok() {
                self.heard = Instant::now();
            }
            match msg {
                // (a picture of the bus shown before, while the one wanted is read: passed over)
                Ok(Msg::Frame(w, h, t, px)) if t == want => {
                    (self.drew, self.pinned) = (true, true);
                    self.aspect = w as f32 / h.max(1) as f32;
                    self.frame = Some((w, h, px));
                    self.showing = Some(t);
                    self.from_cache = false;
                    self.live = true;
                    self.cache = None;
                }
                Ok(Msg::Frame(..)) => (self.drew, self.pinned) = (true, true),
                Ok(Msg::Kept(t, shape)) => self.kept = Some((t, shape)),
                Ok(Msg::Status { busy, error }) => {
                    self.busy = busy;
                    if error.is_some() {
                        self.showing = None;
                        self.live = false;
                    }
                    self.error = error;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    ended = true;
                    break;
                }
            }
        }
        // (a helper with no picture yet that said nothing for so long is taken for one that
        // hung: stopped, and so started on the next interface; the mark of the bus it was
        // reading is the launcher's to clear, as in `stop`)
        if !ended && !self.drew && self.heard.elapsed() > FIRST_PICTURE_LIMIT {
            log::warn!("launcher: the preview helper said nothing in {} s and sent no picture, it is stopped", FIRST_PICTURE_LIMIT.as_secs());
            if let Some(c) = self.link.as_mut().and_then(|l| l.child.as_mut()) {
                let _ = c.kill();
                let _ = c.wait();
            }
            clear_own_mark(self.mark);
            ended = true;
        }
        if ended {
            // (how it ended: the code it gave, if it has given one yet)
            let code = self
                .link
                .as_mut()
                .and_then(|l| l.child.as_mut())
                .and_then(|c| {
                    let until = Instant::now() + Duration::from_millis(300);
                    loop {
                        match c.try_wait() {
                            Ok(Some(s)) => return Some(s.to_string()),
                            Ok(None) if Instant::now() < until => {
                                std::thread::sleep(Duration::from_millis(20))
                            }
                            _ => return None,
                        }
                    }
                });
            log::warn!(
                "launcher: the preview helper stopped ({}; its log: {})",
                code.as_deref().unwrap_or("still ending"),
                helper_log().display()
            );
            self.link = None;
            self.live = false;
            self.busy = false;
            // (one that ended before its first picture is started on the next interface)
            if !self.drew && !self.pinned && self.backend + 1 < self.backends.len() {
                // (its end is the interface's, not the bus's: the mark of the bus it was
                // reading, if any, is not left for the next helper to take as a bus that hung,
                // as when the launcher stops a helper)
                clear_own_mark(self.mark);
                self.backend += 1;
                log::warn!(
                    "launcher: the preview helper is started again on {}",
                    self.backends[self.backend].unwrap_or("the settings' interface")
                );
                self.error = None;
                return;
            }
            self.error
                .get_or_insert_with(|| "The 3D preview stopped".into());
            self.broken = Some(look);
            // (what stays is the kept picture, if there is one, not the helper's last)
            if !self.from_cache {
                self.showing = None;
                self.cache_for = None;
            }
        }
    }

    /// The picture that came since the last call, to go to the GPU.
    pub fn take_frame(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.frame.take()
    }

    /// Which bus and paint (`tag`) the picture shown is of, if any.
    pub fn showing(&self) -> Option<u64> {
        self.showing
    }

    /// A picture of the bus wanted is there to show (live, or kept from an earlier time).
    pub fn has_picture(&self) -> bool {
        self.showing.is_some() && self.showing == self.wanted.as_ref().map(tag)
    }

    /// A bus is wanted and its picture is still to come (no error stopped it).
    pub fn loading(&self) -> bool {
        self.wanted.is_some() && !self.has_picture() && self.error.is_none()
    }
}
