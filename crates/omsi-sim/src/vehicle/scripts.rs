//! The scripts' side of a vehicle: variables, triggers, text textures, paint and service.

use super::weather::relative_humidity;
use super::VehicleInstance;
use std::path::Path;

impl VehicleInstance {
    /// Prepare the text textures with fonts from `lib`.
    pub fn init_text_textures(
        &mut self,
        lib: &mut crate::texttex::FontLibrary,
        decode: &dyn Fn(&Path) -> Option<(u32, u32, Vec<u8>)>,
    ) {
        self.text_textures = self
            .ty
            .model
            .text_textures
            .iter()
            .map(|t| crate::texttex::TextTextureState::new(t.clone(), lib.get(&t.font, decode)))
            .collect();
    }

    /// Current text of a string variable (empty when it does not exist).
    pub fn str_var(&self, name: &str) -> String {
        self.ty
            .program
            .str_var(name)
            .map(|i| self.state.str_vars[i as usize].clone())
            .unwrap_or_default()
    }

    /// The string a `[texttexture]` names: its first field is either a script variable's name
    /// or - as the Chinese AI cars write it, `[texttexture] 0 CN_REG ...` - the *number* of a
    /// built-in string (`program/stringvarlist_roadvehicle.txt`: 0 ident, 1 number, ...).
    /// OMSI takes a number as that index (the scenery objects' `[texttexture]` do the same,
    /// see `resolve_scenery_freetex_name`); taken for a variable's name it matches nothing and
    /// the plate stays empty. See `Program::text_texture_var`.
    pub fn text_texture_string(&self, field: &str) -> String {
        self.ty
            .program
            .text_texture_var(field)
            .map(|i| self.state.str_vars[i as usize].clone())
            .unwrap_or_default()
    }

    /// Re-render changed text textures; returns the indices with a pending image.
    pub fn update_text_textures(&mut self) -> Vec<usize> {
        let mut changed = Vec::new();
        // `Refresh_Strings` is reset; an unchanged string would draw the same picture again
        if let Some(id) = self.ty.program.var("Refresh_Strings") {
            self.state.vars[id as usize] = 0.0;
        }
        for i in 0..self.text_textures.len() {
            let field = self.text_textures[i].def.variable.clone();
            let text = self.text_texture_string(&field);
            if self.text_textures[i].update(&text) {
                changed.push(i);
            }
        }
        changed
    }

    /// The variables a paint scheme sets: its `[setvar]` lines, and `Colorscheme` - the
    /// scheme's index in the `.cti` list, −1 for the model's own textures (OMSI
    /// the original at the spawn; the original reads it back as the index).
    pub fn apply_paint_vars(&mut self, scheme: Option<usize>) {
        let scheme = scheme.filter(|i| *i < self.ty.paint_schemes.len());
        self.set_var("Colorscheme", scheme.map(|i| i as f32).unwrap_or(-1.0));
        if let Some(i) = scheme {
            for (var, v) in self.ty.paint_schemes[i].set_vars.clone() {
                self.set_var(&var, v);
            }
        }
    }

    /// Where variable `name` sits among the script's variables (`State::vars`).
    pub fn var_slot(&self, name: &str) -> Option<usize> {
        omsi_script::compile::with_lower(name, |k| self.var_index.get(k).map(|&i| i as usize))
    }

    pub fn var(&self, name: &str) -> Option<f32> {
        omsi_script::compile::with_lower(name, |k| self.var_index.get(k).copied())
            .map(|i| self.state.vars[i as usize])
    }

    /// Whether variable `name` was declared in the vehicle's script set (as opposed to built-in host variables).
    pub fn has_script_var(&self, name: &str) -> bool {
        self.ty.program.has_script_var(name)
    }

    /// The name of script variable `index` (lower case), for diagnostics and key helpers.
    pub fn var_name(&self, index: usize) -> Option<&str> {
        self.var_index
            .iter()
            .find(|(_, &i)| i as usize == index)
            .map(|(n, _)| n.as_str())
    }

    pub fn set_var(&mut self, name: &str, v: f32) -> bool {
        match omsi_script::compile::with_lower(name, |k| self.var_index.get(k).copied()) {
            Some(i) => {
                self.state.vars[i as usize] = v;
                true
            }
            None => false,
        }
    }

    /// Variables the engine feeds the model directly (`Dirt_Norm`, `AI`, …): OMSI's
    /// varlists rarely declare them, and an undeclared `[alphascale]` variable used to
    /// read as 1.0 - every bus wore its dirt film at full strength from the first frame.
    pub fn set_engine_var(&mut self, name: &str, v: f32) {
        if !self.set_var(name, v) {
            let id = self.state.vars.len() as omsi_script::VarId;
            self.state.vars.push(v);
            self.var_index.insert(name.to_ascii_lowercase(), id);
        }
    }

    pub fn trigger(&mut self, name: &str) -> bool {
        let p = self.ty.program.clone();
        let rear_target_was_open = self.var("doorTarget_23").is_some_and(|target| target > 0.0);
        let rear_force_close_was_active =
            self.var("bdoor_embtn_cls").is_some_and(|close| close > 0.5);
        let rear_was_open = self.var("doorTarget_23").is_some_and(|target| target > 0.0)
            || self.var("door_2").is_some_and(|door| door > 0.05)
            || self.var("door_3").is_some_and(|door| door > 0.05);
        let mut fired = self
            .vm
            .run_trigger(&p, name, &mut self.state, &mut self.host);
        // Several Volvo Wright door scripts expose the actual force-close operation as
        // `bus_dooraft1_external_CL`, while the dashboard `bus_dooraftclose` trigger only
        // sounds the button when its handbrake guard rejects the request.  Use that explicit
        // close path when the dashboard request left the rear door open.  Other buses are
        // unaffected because the fallback trigger is only present in those scripts.
        // A toggle pressed while the leaves are already closing is an open request.  The
        // fallback must not immediately undo that request just because the leaves are still
        // physically open for a few frames.
        let toggle_is_reopen_request = name.eq_ignore_ascii_case("bus_dooraft")
            && !rear_target_was_open
            && (rear_force_close_was_active
                || self.var("doorTarget_23").is_some_and(|target| target > 0.0));
        if (name.eq_ignore_ascii_case("bus_dooraftclose")
            || (name.eq_ignore_ascii_case("bus_dooraft") && !toggle_is_reopen_request))
            && rear_was_open
            && (self.var("doorTarget_23").is_some_and(|target| target > 0.0)
                || self.var("door_2").is_some_and(|door| door > 0.05)
                || self.var("door_3").is_some_and(|door| door > 0.05))
            && p.trigger("bus_dooraft1_external_CL").is_some()
        {
            fired |= self.vm.run_trigger(
                &p,
                "bus_dooraft1_external_CL",
                &mut self.state,
                &mut self.host,
            );
        }
        fired
    }

    /// The script variables as `names` would leave them, run one after another, with the
    /// vehicle left exactly as it was: its variables, the machine's random numbers, the
    /// sounds and messages the triggers asked for.
    pub fn trial_triggers(&mut self, names: &[&str]) -> Vec<f32> {
        let (state, vm) = (self.state.clone(), self.vm.clone());
        let (fired, fired_files, messages, time_written) = (
            self.host.fired_triggers.len(),
            self.host.fired_file_triggers.len(),
            self.host.messages.clone(),
            self.host.time_written,
        );
        let fired_vars = self.host.fired_trigger_vars.len();
        for n in names {
            self.trigger(n);
        }
        let out = self.state.vars.clone();
        self.state = state;
        self.vm = vm;
        self.host.fired_triggers.truncate(fired);
        self.host.fired_trigger_vars.truncate(fired_vars);
        self.host.fired_file_triggers.truncate(fired_files);
        self.host.messages = messages;
        self.host.time_written = time_written;
        out
    }

    /// Dirt and spray. OMSI writes three engine variables every frame and the bus scripts
    /// turn them into what you see: `Dirt_Norm` is how dirty the body is (the `[alphascale]`
    /// of the dirt overlay), `DirtRate` how fast the windscreen is soiling right now
    /// (`dirt.osc` adds it up into `Dirt_Wiped`, which the wipers clear), and `PrecipRate`
    /// is signed, so rain wets the glass in `rain.osc` and dry weather dries it again.
    pub(super) fn update_dirt(&mut self, dt: f32) {
        let rain = self.host.precip_rate.clamp(0.0, 1.0);
        let speed = (self.motion.velocity_kmh().abs() / 50.0).min(2.0);
        // a dry duty adds a few percent, a wet one soils the bus in a couple of hours
        // a dry duty of a couple of hours shows; a wet one soils the bus in half an hour
        self.dirt = (self.dirt + dt * speed * (8.0e-5 + 1.2e-3 * rain)).clamp(0.0, 1.0);
        self.set_engine_var("Dirt_Norm", self.dirt);
        // spray off a wet road, dust off a dry one
        self.set_engine_var("DirtRate", speed * (1.5e-4 + 6.0e-3 * rain));
        // rain soaks the glass in a few seconds; without it the film dries in about a minute.
        // Snow builds the film up the same way: `rain.osc` only ever adds `PrecipRate *
        // Timegap` and clamps at 1 - it never asks what is falling - so in the original the
        // glass gets as covered in a snowfall as in a shower and the wipers clear it. (The
        // film wears snow crystals then, see `rain::snow_on_glass`.) Held to a fifth for a
        // "haze", the panes stayed clear in the thickest snowfall and the wipers had
        // nothing to do (#883).
        let rate = if rain > 0.0 { rain * 0.25 } else { -0.02 };
        self.set_engine_var("PrecipRate", rate);
        // the state of the road, for the tyre sounds and the wheel spray
        self.set_engine_var("StreetCond", self.host.street_cond);
        // what is coupled to this vehicle (the train scripts light their ends by it: a head
        // lamp only where nothing is coupled, the tail lamps at the last car)
        self.set_engine_var("train_frontcoupling", 0.0);
        self.set_engine_var(
            "train_backcoupling",
            if self.trailers.is_empty() { 0.0 } else { 1.0 },
        );
        self.set_engine_var("train_me_reverse", 0.0);
    }

    /// The engine's own vehicle variables that OMSI binds to fields of the vehicle
    /// (the original registers them, points each at its field, and
    /// keeps the fields up to date): the odometer (`kmcounter_km` whole
    /// kilometres, `kmcounter_m` the metres of the fractions), the people aboard, whether a
    /// timetable is driven, the precipitation type, and the cabin air - kept within reach of
    /// the weather (never more than 10 °C from the air outside and pulled towards 18..25 °C
    /// as a heated/ventilated bus is; the absolute humidity follows the outside air).
    pub(crate) fn update_engine_vars(&mut self, dt: f32) {
        self.update_driver_seat(dt);
        // `[kmcounter_init] year km`: in service since that year, so many kilometres a year -
        // the odometer starts at what that comes to on the day driven (it stood at 0 on
        // every bus that has one, #305), a little different from bus to bus of the kind.
        // OMSI's formula: Random(100)/10 + 8 + max(0, years) * km * (1 + 0.2 *
        // (Random(100) - 50) / 50), and 1980 / 60000 km a year without the keyword
        // (OMSI's defaults).
        if !self.km_started {
            self.km_started = true;
            if self.host.km_base == 0.0 {
                let (year, per_year) = self.ty.def.km_counter_init.unwrap_or((1980, 60000.0));
                let years = ((self.host.clock.year - year) as f64
                    + self.host.clock.day_of_year as f64 / 365.0)
                    .max(0.0);
                let seed = (std::ptr::addr_of!(self.host) as u64)
                    .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                    >> 40;
                let (r1, r2) = ((seed % 100) as f64, ((seed / 100) % 100) as f64);
                self.host.km_base =
                    r1 / 10.0 + 8.0 + years * per_year as f64 * (1.0 + 0.2 * (r2 - 50.0) / 50.0);
            }
        }
        // (the sum is split, not the parts: 0.7 km + 0.5 km is 1 km 200 m, not 0 km 1200 m)
        let total = self.odometer_km();
        self.set_engine_var("kmcounter_km", total.trunc() as f32);
        self.set_engine_var("kmcounter_m", (total.fract() * 1000.0) as f32);
        self.set_engine_var("humans_count", self.host.humans_count);
        self.set_engine_var("schedule_active", self.host.schedule_active);
        self.set_engine_var("precipType", self.host.precip_type);
        // A bus whose scripts keep their own cabin air (`heizung.osc` of the stock buses and
        // their descendants: they integrate the temperature from heat flows) is left to
        // them; only the relative humidity comes from their temperature and humidity. An
        // engine model on top of theirs broke that balance - the Sprinter 312D's cabin
        // went to infinity within a second, and its engine with it.
        let owns = |name: &str| {
            self.ty
                .program
                .var(name)
                .is_some_and(|v| self.ty.program.stores(v))
        };
        if owns("Cabinair_Temp") {
            if let (Some(t), Some(hum)) = (self.var("Cabinair_Temp"), self.var("Cabinair_absHum")) {
                if !owns("Cabinair_relHum") && t.is_finite() && t > -237.0 {
                    self.set_engine_var("Cabinair_relHum", relative_humidity(t, hum));
                }
            }
            return;
        }
        let out = self.host.temperature;
        let (mut t, mut hum, last) =
            self.cabin_air
                .unwrap_or((out.clamp(18.0, 25.0), 9.0, f32::NAN));
        // a script of the bus's own wrote the cabin temperature: that is what it is now
        if let Some(cur) = self.var("Cabinair_Temp") {
            if !last.is_nan() && (cur - last).abs() > 1e-4 {
                t = cur;
            }
        }
        let lower = (out + 10.0).min(18.0).max(out - 10.0);
        let upper = (out + 20.0).min(out.max(25.0)).max(lower);
        let target = t.clamp(lower, upper);
        t += (target - t) * (dt / 120.0).min(1.0);
        let out_hum = if self.host.abs_humidity > 0.0 {
            self.host.abs_humidity
        } else {
            9.0
        };
        hum += (out_hum - hum) * (dt / 300.0).min(1.0);
        // relative humidity: against the saturation (Magnus) at the cabin temperature

        self.set_engine_var("Cabinair_Temp", t);
        self.set_engine_var("Cabinair_absHum", hum);
        self.set_engine_var("Cabinair_relHum", relative_humidity(t, hum));
        self.cabin_air = Some((t, hum, t));
    }

    /// Fire a trigger and read the number the script leaves on the stack. OMSI asks the
    /// bus how long its repairs take this way (`malfunction_gettime`): every damaged system
    /// adds its minutes to the value passing through the chain of macros.
    pub fn trigger_value(&mut self, name: &str) -> Option<f32> {
        let p = self.ty.program.clone();
        self.vm.stacks = Default::default();
        if !self
            .vm
            .run_trigger(&p, name, &mut self.state, &mut self.host)
        {
            return None;
        }
        Some(self.vm.stacks.st[0])
    }

    /// Run one of the engine's service triggers with `secs` on the clock: OMSI holds
    /// `veh_tank` / `veh_wash` down while the pump or the wash runs and the bus script
    /// decides what a second of it is worth (the SD202 takes 3 litres and caps at 250).
    /// One step of a depot service: its trigger (`veh_tank`, `veh_wash`) with `secs` of
    /// the script's clock. False when the bus has no such handling.
    pub fn service(&mut self, name: &str, secs: f32) -> bool {
        let keep = self.host.clock.timegap;
        self.host.clock.timegap = secs;
        let ok = self.trigger(name);
        self.host.clock.timegap = keep;
        ok
    }

    /// Refuel until the tank stops filling. Returns the tank content the script reports.
    pub fn refuel(&mut self) -> Option<f32> {
        let mut last = f32::NEG_INFINITY;
        for _ in 0..600 {
            if !self.service("veh_tank", 1.0) {
                return None;
            }
            let now = self.var("engine_tank_content")?;
            if now <= last + 1e-3 {
                break;
            }
            last = now;
        }
        self.var("engine_tank_content")
    }

    /// Run the bus through the wash until it stops getting cleaner.
    pub fn wash(&mut self) -> Option<f32> {
        self.dirt = 0.0;
        self.set_engine_var("Dirt_Norm", 0.0);
        let mut last = f32::INFINITY;
        for _ in 0..600 {
            if !self.service("veh_wash", 1.0) {
                return None;
            }
            let now = self.var("Dirt_Wiped")?;
            if now >= last - 1e-4 {
                break;
            }
            last = now;
        }
        self.var("Dirt_Wiped")
    }

    /// Minutes the workshop needs for the damage the bus has right now.
    pub fn repair_minutes(&mut self) -> Option<f32> {
        self.trigger_value("malfunction_gettime")
    }

    /// Repair everything (the engine fires this once the driver accepts the waiting time).
    pub fn repair(&mut self) -> bool {
        self.trigger("malfunction_reset")
    }

    /// Brightest `[interiorlight]` of the vehicle right now (0..1), for the passengers.
    pub fn interior_light(&self) -> f32 {
        self.ty
            .model
            .interior_lights
            .iter()
            .map(|il| {
                il.variable
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .or_else(|| self.var(&il.variable))
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0)
                    * il.range.clamp(0.0, 1.0)
            })
            .fold(0.0, f32::max)
    }
}
