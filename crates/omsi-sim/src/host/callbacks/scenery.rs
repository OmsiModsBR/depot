//! Script callbacks of the scenery family, dispatched from [`Host::callback`](super::super::VehicleHost).

use crate::host::{arg_i32, VehicleHost};
use omsi_script::Stacks;

impl VehicleHost {
    /// The `callback`s of the scenery family; `false` when the name is not one of them.
    pub(in crate::host) fn cb_scenery(&mut self, lname: &str, stacks: &mut Stacks) -> bool {
        match lname {
            // --- scenery objects (`program/callbacklist_scenobj.txt`): the n-th bus due at
            // the object's stop. Past the last one the line and terminus are empty (the stock
            // display leaves its line dark on an empty terminus) and the time is 0.
            "getarrbusline" => {
                let i = arg_i32(stacks.pop());
                stacks.push_str(self.arrival(i).map(|a| a.line.clone()).unwrap_or_default());
            }
            "getarrbusterminus" => {
                let i = arg_i32(stacks.pop());
                stacks.push_str(
                    self.arrival(i)
                        .map(|a| a.terminus.clone())
                        .unwrap_or_default(),
                );
            }
            "getarrbustimediff" => {
                let i = arg_i32(stacks.pop());
                stacks.push(self.arrival(i).map(|a| a.due).unwrap_or(0.0));
            }
            _ => return false,
        }
        true
    }
}
