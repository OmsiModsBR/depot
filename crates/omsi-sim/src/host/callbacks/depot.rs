//! Script callbacks of the depot family, dispatched from [`Host::callback`](super::super::VehicleHost).

use crate::host::{arg_i32, VehicleHost};
use omsi_script::Stacks;

impl VehicleHost {
    /// The `callback`s of the depot family; `false` when the name is not one of them.
    pub(in crate::host) fn cb_depot(&mut self, lname: &str, stacks: &mut Stacks) -> bool {
        match lname {
            // --- depot (HOF) callbacks of the IBIS: arguments come off the float stack, the
            // last pushed first
            "getrouteindex" => {
                let code = stacks.pop();
                let code_i = arg_i32(code);
                let idx = self
                    .hof
                    .as_ref()
                    .and_then(|h| {
                        h.info_trips.iter().position(|t| {
                            let c = t.code.trim();
                            omsi_cfg::parse_i32(c) == code_i
                                || (code - omsi_cfg::parse_f32(c)).abs() < 1e-3
                        })
                    })
                    .map(|i| i as f32)
                    .unwrap_or(-1.0);
                stacks.push(idx);
            }
            // An index of -1 (what the lookups answer for an unknown code) names no entry:
            // the strings are empty and the codes -1. Reading entry 0 instead put the first
            // terminus of the depot ("LEERFELD") where a mod's display looks for a custom
            // font or an optional sign.
            "getrouteterminusindex" => {
                let route = arg_i32(stacks.pop());
                let idx = self.hof.as_ref().and_then(|h| {
                    let trip = h.info_trips.get(usize::try_from(route).ok()?)?;
                    let code = omsi_cfg::parse_i32(&trip.route);
                    h.termini.iter().position(|t| t.code == code)
                });
                stacks.push(idx.map(|i| i as f32).unwrap_or(-1.0));
            }
            "getterminuscode" => {
                let idx = arg_i32(stacks.pop());
                let code = self.terminus(idx).map(|t| t.code as f32).unwrap_or(-1.0);
                stacks.push(code);
            }
            "getterminusindex" => {
                let code = arg_i32(stacks.pop());
                let idx = self
                    .hof
                    .as_ref()
                    .and_then(|h| h.termini.iter().position(|t| t.code == code))
                    .map(|i| i as f32)
                    .unwrap_or(-1.0);
                stacks.push(idx);
            }
            "getterminusstring" => {
                let n = arg_i32(stacks.pop());
                let idx = arg_i32(stacks.pop());
                let s = self
                    .terminus(idx)
                    .and_then(|t| t.strings.get(usize::try_from(n).ok()?))
                    .cloned()
                    .unwrap_or_default();
                stacks.push_str(s);
            }
            "getbusstopcount" => {
                let route = arg_i32(stacks.pop());
                let n = self
                    .hof
                    .as_ref()
                    .and_then(|h| h.info_busstop_lists.get(usize::try_from(route).ok()?))
                    .map(|l| l.len() as f32)
                    .unwrap_or(0.0);
                stacks.push(n);
            }
            "getroutebusstopident" => {
                let stop = arg_i32(stacks.pop());
                let route = arg_i32(stacks.pop());
                let s = self
                    .hof
                    .as_ref()
                    .and_then(|h| h.info_busstop_lists.get(usize::try_from(route).ok()?))
                    .and_then(|l| l.get(usize::try_from(stop).ok()?))
                    .cloned()
                    .unwrap_or_default();
                stacks.push_str(s);
            }
            "getbusstopindex" => {
                let ident = stacks.pop_str();
                let idx = self
                    .hof
                    .as_ref()
                    .and_then(|h| {
                        h.bus_stops
                            .iter()
                            .position(|b| b.ident.trim().eq_ignore_ascii_case(ident.trim()))
                    })
                    .map(|i| i as f32)
                    .unwrap_or(-1.0);
                stacks.push(idx);
            }
            "getbusstopstring" => {
                let n = arg_i32(stacks.pop());
                let idx = arg_i32(stacks.pop());
                let s = self
                    .bus_stop(idx)
                    .and_then(|b| b.strings.get(usize::try_from(n).ok()?))
                    .cloned()
                    .unwrap_or_default();
                stacks.push_str(s);
            }
            "getdepotstringglobal" => {
                let n = arg_i32(stacks.pop());
                let s = self
                    .hof
                    .as_ref()
                    .and_then(|h| h.global_strings.get(usize::try_from(n).ok()?))
                    .cloned()
                    .unwrap_or_default();
                stacks.push_str(s);
            }
            _ => return false,
        }
        true
    }
}
