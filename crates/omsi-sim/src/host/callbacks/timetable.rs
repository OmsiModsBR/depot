//! Script callbacks of the timetable family, dispatched from [`Host::callback`](super::super::VehicleHost).

use crate::host::{arg_i32, VehicleHost};
use omsi_script::Stacks;

impl VehicleHost {
    /// The `callback`s of the timetable family; `false` when the name is not one of them.
    pub(in crate::host) fn cb_timetable(&mut self, lname: &str, stacks: &mut Stacks) -> bool {
        match lname {
            // --- timetable callbacks
            "getttlinestring" => stacks.push_str(self.tt_line.clone()),
            "getttdelay" => stacks.push(self.tt_delay),
            "getttbusstopcount" => stacks.push(self.tt_stops.len() as f32),
            // Without a timetable there is no current stop. Returning the first
            // stop (0) makes the Atron repeatedly detect an arrival and clear its
            // sales text after the holding brake has been on for three seconds.
            "getttbusstopindex" => stacks.push(if self.tt_stops.is_empty() {
                -1.0
            } else {
                self.tt_busstop_index as f32
            }),
            "gettterminusindex" | "getttterminusindex" => {
                stacks.push(if self.tt_stops.is_empty() {
                    -1.0
                } else {
                    self.tt_terminus_index as f32
                })
            }
            "getttbusstopname" => {
                let i = arg_i32(stacks.pop());
                stacks.push_str(
                    self.tt_stops
                        .get(usize::try_from(i).unwrap_or(usize::MAX))
                        .map(|s| s.0.clone())
                        .unwrap_or_default(),
                );
            }
            "getttbusstopdep" => {
                let i = arg_i32(stacks.pop());
                stacks.push(
                    self.tt_stops
                        .get(usize::try_from(i).unwrap_or(usize::MAX))
                        .map(|s| s.2)
                        .unwrap_or(0.0),
                );
            }
            "getttbusstoparr" => {
                let i = arg_i32(stacks.pop());
                stacks.push(
                    self.tt_stops
                        .get(usize::try_from(i).unwrap_or(usize::MAX))
                        .map(|s| s.1)
                        .unwrap_or(0.0),
                );
            }
            _ => return false,
        }
        true
    }
}
