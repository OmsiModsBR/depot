//! Script callbacks of the passengers family, dispatched from [`Host::callback`](super::super::VehicleHost).

use crate::host::{arg_i32, VehicleHost};
use omsi_script::{Stacks, State};

impl VehicleHost {
    /// The `callback`s of the passengers family; `false` when the name is not one of them.
    pub(in crate::host) fn cb_passengers(
        &mut self,
        lname: &str,
        stacks: &mut Stacks,
        state: &State,
    ) -> bool {
        match lname {
            // how high a point of the vehicle stands over the ground (the NL/NG ramp
            // measures the kerb this way before extending)
            "getheightabovepoint" => {
                let z = stacks.pop();
                let y = stacks.pop();
                let x = stacks.pop();
                let h = self
                    .ground_probe
                    .as_ref()
                    .map(|f| f(x, y, z))
                    .unwrap_or(0.0);
                stacks.push(h);
            }
            // passengers standing on one link of the cabin path network
            "gethumancountonpathlink" => {
                let i = arg_i32(stacks.pop());
                stacks.push(
                    self.humans_on_path_link
                        .get(usize::try_from(i).unwrap_or(usize::MAX))
                        .copied()
                        .unwrap_or(0) as f32,
                );
            }
            "givechangecoin" => {
                let i = arg_i32(stacks.pop());
                if i >= 0 {
                    self.change_coins.push(i as usize);
                }
            }
            // OMSI's number-specific random value: the same for a fleet
            // number and characteristic every time (broken matrix pixels, wear effects)
            "nrspecrandom" => {
                let n = arg_i32(stacks.pop());
                let number = self
                    .number_var
                    .and_then(|i| state.str_vars.get(i as usize))
                    .cloned()
                    .unwrap_or_default();
                stacks.push(omsi_vehicle::sound::spec_random(&number, n));
            }
            "getticketname" | "gettticketname" => {
                let i = arg_i32(stacks.pop());
                stacks.push_str(
                    self.tickets
                        .as_ref()
                        .and_then(|t| t.tickets.get(usize::try_from(i).unwrap_or(usize::MAX)))
                        .map(|t| t.name.clone())
                        .unwrap_or_default(),
                );
            }
            "getticketvalue" => {
                let i = arg_i32(stacks.pop());
                stacks.push(
                    self.tickets
                        .as_ref()
                        .and_then(|t| t.tickets.get(usize::try_from(i).unwrap_or(usize::MAX)))
                        .map(|t| t.value)
                        .unwrap_or(0.0),
                );
            }
            "gethumancountonseat" => {
                let i = arg_i32(stacks.pop());
                stacks.push(
                    self.humans_on_seat
                        .get(usize::try_from(i).unwrap_or(usize::MAX))
                        .copied()
                        .unwrap_or(0) as f32,
                );
            }
            _ => return false,
        }
        true
    }
}
