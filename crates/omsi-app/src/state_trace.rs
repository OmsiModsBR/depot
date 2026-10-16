//! The offscreen run's state, frame by frame (`OMSI_STATE_TRACE=<file>`): where the
//! player's vehicle is and how fast it goes, the AI cars, the road lights and the rail
//! signals, and the player's duty. Written for comparing two commits
//! (`scripts/offscreen-trace-diff.sh`): a change that only moves code leaves the file
//! identical, to the bit.
//!
//! Numbers are written as `{:?}` writes them - the shortest decimal that reads back to the
//! same value - so two traces differ exactly where the state does. One block per frame,
//! the state before that frame's step, and a last block with the state the run ends on:
//!
//! ```text
//! frame 12 t 0.4
//!   player p 148.25 75.1 -0.1 heading 0.0 v 0.0 8.3 0.0 speed 8.3
//!   cars 2
//!     car 3 p 151.75 120.4 0.0 heading 180.0 speed 11.1 lane 4 s 29.6
//!   lights 0: 0 6
//!   rail
//!   duty -
//! ```

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::player::Player;
use crate::schedule::PlayerDuty;
use crate::traffic::Traffic;

pub(crate) struct StateTrace {
    path: PathBuf,
    text: String,
}

impl StateTrace {
    /// The trace `OMSI_STATE_TRACE` asks for, if any.
    pub(crate) fn from_env() -> Option<StateTrace> {
        let path = omsi_cfg::flags::STATE_TRACE.text().filter(|p| !p.trim().is_empty())?;
        Some(StateTrace { path: PathBuf::from(path), text: String::new() })
    }

    /// The state at the start of frame `i` (`t` seconds into the run).
    pub(crate) fn frame(
        &mut self,
        i: usize,
        t: f32,
        world: &crate::scene::World,
        player: Option<&Player>,
        traffic: Option<&Traffic>,
        duty: Option<&PlayerDuty>,
    ) {
        let s = &mut self.text;
        let _ = writeln!(s, "frame {i} t {t:?}");
        match player {
            Some(p) => {
                let v = &p.vehicle;
                let vel = v.rigid().map(|rb| rb.velocity()).unwrap_or_default();
                let _ = writeln!(
                    s,
                    "  player p {:?} {:?} {:?} heading {:?} v {:?} {:?} {:?} speed {:?}",
                    v.position.x,
                    v.position.y,
                    v.position.z,
                    v.heading,
                    vel.x,
                    vel.y,
                    vel.z,
                    v.motion().speed
                );
            }
            None => s.push_str("  player -\n"),
        }
        let Some(t) = traffic else {
            s.push_str("  cars -\n  lights -\n  rail -\n");
            Self::duty(s, player, duty);
            return;
        };
        // (by id: the order of `cars` is how they were put on the road and taken off)
        let mut cars: Vec<_> = t.cars.iter().collect();
        cars.sort_by_key(|c| c.id);
        let _ = writeln!(s, "  cars {}", cars.len());
        for c in cars {
            let v = &c.vehicle;
            let _ = writeln!(
                s,
                "    car {} p {:?} {:?} {:?} heading {:?} speed {:?} lane {} s {:?}",
                c.id, v.position.x, v.position.y, v.position.z, v.heading, c.state.speed, c.state.lane, c.state.s
            );
        }
        s.push_str("  lights");
        for (k, states) in t.controller_states().iter().enumerate() {
            let _ = write!(s, " {k}:");
            for st in states {
                let _ = write!(s, " {st}");
            }
        }
        s.push('\n');
        // (the rail signals as the frame's step will set them; the offscreen run hands
        // them no player's rail, see `window_and_offscreen_differ_in_the_rail_given_to_signal_aspects`)
        let mut rail: Vec<(i64, f32)> = t.signal_aspects(&world.signal_routes, None).into_iter().collect();
        rail.sort_by_key(|r| r.0);
        s.push_str("  rail");
        for (id, aspect) in rail {
            let _ = write!(s, " {id}:{aspect:?}");
        }
        s.push('\n');
        Self::duty(s, player, duty);
    }

    fn duty(s: &mut String, player: Option<&Player>, duty: Option<&PlayerDuty>) {
        match duty {
            Some(d) => {
                let delay = player.map(|p| p.vehicle.host.tt_delay).unwrap_or_default();
                let _ = writeln!(s, "  duty line {} tour {} trip {} next_stop {} delay {delay:?}", d.line, d.tour, d.trip_index, d.next_stop);
            }
            None => s.push_str("  duty -\n"),
        }
    }

    /// Write the trace out.
    pub(crate) fn finish(self) -> anyhow::Result<()> {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.path, self.text).map_err(|e| anyhow::anyhow!("OMSI_STATE_TRACE: cannot write {}: {e}", self.path.display()))?;
        log::info!("state trace written to {}", self.path.display());
        Ok(())
    }
}
