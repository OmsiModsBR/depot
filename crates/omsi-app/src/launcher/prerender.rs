//! Pictures of the library's vehicles drawn ahead of time, for the Vehicles page's small
//! pictures of each paint.
//!
//! The pictures are drawn by a preview helper of their own (`preview::Preview`, so the
//! interface fallback and the watchdog on a helper that says nothing are the same ones the
//! page's live preview has; it keeps its own mark of the bus it reads, so it and the live
//! helper never take each other's for a bus that hung), one vehicle paint after another, in the
//! background. It is stopped when a game starts and while the window is hidden (`suspend`), and
//! goes on afterwards. Each picture is kept by the helper as any settled picture is, small and
//! at one fixed shape (`preview::thumb_shape`), in a folder of its own, so the page finds it
//! whatever the size of its own preview: only what is not kept yet is drawn. The run starts when another OMSI 2 folder is saved under Setup (once the
//! content is read) and from the Vehicles page's button, which reads the content again first
//! so that new vehicles are found; it can be cancelled.

use super::preview::{self, Preview, Shape};
use super::showroom::Look;
use omsi_launcher_ui::vehicles::{PrerenderPhase, PrerenderStatus};
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// How long the content may take to start being read after a run was asked for.
pub const READING_STARTS: Duration = Duration::from_secs(20);

/// A picture that takes longer than this is given up (the preview helper's own watchdog ends
/// a helper that says nothing for 240 s; this one is for a helper that talks but never settles).
pub const PER_PICTURE: Duration = Duration::from_secs(300);

/// The shape of every picture drawn here.
pub fn shape() -> Shape {
    preview::thumb_shape()
}

pub struct Prerender {
    preview: Preview,
    /// Start a run as soon as the content has been read (the folder was just chosen, or the
    /// button asked for a new reading).
    pub after_content: bool,
    /// The content was seen being read since the run was asked for, and when it was asked.
    seen_reading: bool,
    pub asked_at: Instant,
    /// The helper was stopped for a game or a hidden window (the picture in flight starts over).
    pub paused: bool,
    scan: Option<Receiver<Vec<Look>>>,
    queue: VecDeque<Look>,
    pub current: Option<(Look, Instant)>,
    pub phase: PrerenderPhase,
    done: usize,
    total: usize,
    failed: usize,
}

impl Default for Prerender {
    fn default() -> Self {
        Prerender {
            preview: Preview::for_prerender(),
            after_content: false,
            seen_reading: false,
            asked_at: Instant::now(),
            paused: false,
            scan: None,
            queue: VecDeque::new(),
            current: None,
            phase: PrerenderPhase::Idle,
            done: 0,
            total: 0,
            failed: 0,
        }
    }
}

impl Prerender {
    pub fn status(&self) -> PrerenderStatus {
        PrerenderStatus {
            phase: self.phase,
            done: self.done,
            total: self.total,
            failed: self.failed,
        }
    }

    /// How many pictures are made so far in this run (it changes when a new one is there).
    pub fn progress(&self) -> usize {
        self.done
    }

    /// A run is about to start (the content is being read): the page says so at once.
    pub fn looking(&mut self) {
        self.phase = PrerenderPhase::Looking;
        self.after_content = true;
        self.seen_reading = false;
        self.asked_at = Instant::now();
    }

    /// Whether the content has been read since the run was asked for, so that the run can
    /// start on the new list (`loading`: it is being read now; `empty`: it has no vehicles).
    /// A reading that never starts, or ends with nothing, drops the request.
    pub fn content_ready(&mut self, loading: bool, empty: bool) -> bool {
        if !self.after_content {
            return false;
        }
        if loading {
            self.seen_reading = true;
        } else if self.seen_reading {
            self.after_content = false;
            if empty {
                self.phase = PrerenderPhase::Idle;
                return false;
            }
            return true;
        } else if self.asked_at.elapsed() > READING_STARTS {
            self.cancel();
        }
        false
    }

    /// Stop the helper for a game or a hidden window: what is drawn stays, the run goes on
    /// from the picture in flight when it can (`update`).
    pub fn suspend(&mut self) {
        if self.preview_in_use() {
            self.preview.stop();
        }
        self.paused = true;
    }

    fn preview_in_use(&self) -> bool {
        self.phase == PrerenderPhase::Running
    }

    /// Stop the run: what is made stays.
    pub fn cancel(&mut self) {
        self.after_content = false;
        self.scan = None;
        self.queue.clear();
        self.current = None;
        self.phase = PrerenderPhase::Idle;
        self.paused = false;
        self.preview.stop();
    }

    /// Find which paints of `vehicles` have no picture yet (on a thread: it reads the buses'
    /// folders) and draw those, in the light of `scene` (its map, weather, time and date).
    pub fn start(&mut self, vehicles: &[omsi_launcher_lib::VehicleInfo], scene: Look) {
        self.after_content = false;
        self.queue.clear();
        self.current = None;
        (self.done, self.total, self.failed) = (0, 0, 0);
        self.phase = PrerenderPhase::Looking;
        let buses: Vec<(String, Vec<String>)> = vehicles
            .iter()
            .map(|v| {
                // (the bus's own livery has the empty paint name)
                let paints = std::iter::once(String::new())
                    .chain(v.paints.iter().cloned())
                    .collect();
                (v.file.clone(), paints)
            })
            .collect();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut missing = Vec::new();
            for (bus, paints) in buses {
                let looks = paints
                    .into_iter()
                    .map(|paint| Look {
                        bus: bus.clone(),
                        paint,
                        ..scene.clone()
                    })
                    .collect();
                missing.extend(preview::missing_kept(looks, shape()));
            }
            // (so many that a run could prune its own pictures: the rest is for the next run)
            missing.truncate(preview::RUN_MAX);
            let _ = tx.send(missing);
        });
        self.scan = Some(rx);
    }

    /// Once per frame. Nothing is drawn while a game runs (the card is the game's then).
    pub fn update(&mut self, dt: f32, game_running: bool) {
        if let Some(rx) = &self.scan {
            match rx.try_recv() {
                Ok(looks) => {
                    self.scan = None;
                    self.total = looks.len();
                    self.queue = looks.into();
                    log::info!("launcher: {} vehicle pictures to draw", self.total);
                    self.phase = if self.total == 0 {
                        PrerenderPhase::Done
                    } else {
                        PrerenderPhase::Running
                    };
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => {
                    self.scan = None;
                    self.phase = PrerenderPhase::Idle;
                }
            }
        }
        if self.phase != PrerenderPhase::Running || game_running {
            // (nothing is drawn while a game runs, and the helper is not left up for it)
            if game_running && !self.paused {
                self.suspend();
            }
            // (the helper of a run that ended lets go after a moment)
            self.preview.update(dt, None);
            return;
        }
        if std::mem::take(&mut self.paused) {
            // (the time the helper was stopped is not the picture's)
            if let Some(c) = self.current.as_mut() {
                c.1 = Instant::now();
            }
        }
        if self.current.is_none() {
            let Some(look) = self.queue.pop_front() else {
                self.phase = PrerenderPhase::Done;
                self.preview.stop();
                return;
            };
            self.preview.want_picture(look.clone());
            self.current = Some((look, Instant::now()));
        }
        self.preview.update(dt, Some(preview::THUMB_SIZE));
        // (the frames are not shown: the pictures are kept by the helper)
        let _ = self.preview.take_frame();
        let Some((look, since)) = self.current.clone() else {
            return;
        };
        let tag = preview::tag(&look);
        let kept = self.preview.kept() == Some((tag, shape()))
            || (self.preview.from_cache && self.preview.showing() == Some(tag));
        if kept {
            self.done += 1;
            self.current = None;
        } else if self.preview.gave_up() || since.elapsed() > PER_PICTURE {
            log::warn!(
                "launcher: the picture of {} ({}) could not be drawn, left out",
                look.bus,
                if look.paint.is_empty() {
                    "its own paint"
                } else {
                    look.paint.as_str()
                }
            );
            self.failed += 1;
            self.done += 1;
            self.current = None;
            // (a helper that hung on this one is ended; the next starts a new one)
            self.preview.stop();
        }
    }
}
