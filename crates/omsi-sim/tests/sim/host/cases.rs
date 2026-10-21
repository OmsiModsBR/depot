use crate::support::scratch::ScratchPack;
use omsi_script::{compile, CompileInput, Vm};
use omsi_script::{Host, NameId, Stacks, State};
use omsi_sim::testing::host::*;
use omsi_sim::testing::scripttex::ScriptTexture;
use omsi_sim::SimClock;
use omsi_vehicle::hof::Hof;
use std::sync::Arc;

/// A bus put down fires triggers while its `{init}` runs, before any sound set exists:
/// they are forgotten, the announcements are not, and a trigger fired later is kept.
#[test]
fn sound_triggers_fired_before_the_sounds_exist_are_forgotten() {
    let mut host = VehicleHost::new(SimClock::default());
    host.snapshot_triggers
        .insert("ev_engineshutdown".to_string());
    host.sound_trigger_vars("ev_EngineShutdown", NameId::default(), &[1.0, 2.0]);
    host.sound_trigger_file("ev_announce", "Announcements\\stop.wav");
    assert_eq!(host.fired_triggers.len(), 1);
    assert_eq!(host.fired_trigger_vars.len(), 1);
    host.forget_fired_sound_triggers();
    assert!(host.fired_triggers.is_empty() && host.fired_trigger_vars.is_empty());
    assert_eq!(host.fired_file_triggers.len(), 1, "announcements stay");
    host.sound_trigger("ev_enginestart", NameId::default());
    assert_eq!(host.fired_triggers, vec!["ev_enginestart".to_string()]);
}

#[test]
fn departure_wants_keep_the_stops_asked_most_recently() {
    let mut host = VehicleHost::new(SimClock::default());
    for i in 0..MAX_HTML_DEPARTURE_STOPS + 3 {
        host.want_departures(format!("stop {i}"));
    }
    assert_eq!(host.html_departure_wants.len(), MAX_HTML_DEPARTURE_STOPS);
    assert_eq!(
        host.html_departure_wants.first().map(String::as_str),
        Some("stop 3")
    );
    assert_eq!(
        host.html_departure_wants.last().map(String::as_str),
        Some("stop 10")
    );
    // asked again: moves to the back and nothing is pushed out
    host.want_departures("stop 3".to_string());
    assert_eq!(host.html_departure_wants.len(), MAX_HTML_DEPARTURE_STOPS);
    assert_eq!(
        host.html_departure_wants.last().map(String::as_str),
        Some("stop 3")
    );
    assert_eq!(
        host.html_departure_wants.first().map(String::as_str),
        Some("stop 4")
    );
}

/// A callback's argument is rounded, as OMSI's `fistp` does: 1100.9999 out of the
/// script's arithmetic names terminus 1101, not 1100.
#[test]
fn callback_arguments_are_rounded() {
    let p = compile(&CompileInput::default());
    let mut state = State::new(&p);
    let mut host = VehicleHost::new(SimClock::default());
    let terminus = |code: i32, name: &str| omsi_vehicle::hof::Terminus {
        code,
        strings: vec![name.to_string()],
        ..Default::default()
    };
    host.hof = Some(Arc::new(Hof {
        termini: vec![terminus(1100, "A"), terminus(1101, "B")],
        ..Default::default()
    }));
    let mut stacks = Stacks::default();
    stacks.push(1100.9999);
    host.callback("GetTerminusIndex", 0, &mut stacks, &mut state);
    assert_eq!(stacks.pop(), 1.0);
    // index 0.9999, string 0: the second terminus' first string
    stacks.push(0.9999);
    stacks.push(0.0);
    host.callback("GetTerminusString", 0, &mut stacks, &mut state);
    assert_eq!(stacks.pop_str(), "B");
    // a half goes to the even number, as under Delphi's control word
    assert_eq!(
        (arg_i32(0.5), arg_i32(1.5), arg_i32(2.5), arg_i32(-0.5)),
        (0, 2, 2, 0)
    );
    assert_eq!(arg_idx(-1.0), usize::MAX);
}

#[test]
fn script_texture_colour_is_kept_per_texture() {
    let p = compile(&CompileInput::default());
    let mut state = State::new(&p);
    let mut host = VehicleHost::new(SimClock::default());
    host.script_textures.push(ScriptTexture::new(2, 1));
    host.script_textures.push(ScriptTexture::new(2, 1));
    host.script_textures[0].put(1, 0, [12, 34, 56, 78]);
    host.script_textures[1].color = [1, 2, 3, 255];
    let mut stacks = Stacks::default();

    // RHLib reads a source pixel, then asks its target texture whether its
    // previously selected drawing colour is transparent.
    stacks.push(0.0);
    stacks.push(1.0);
    stacks.push(0.0);
    host.callback("STReadPixel", 0, &mut stacks, &mut state);

    stacks.push(0.0);
    host.callback("STGetA", 0, &mut stacks, &mut state);
    assert_eq!(stacks.pop(), 78.0);
    stacks.push(1.0);
    host.callback("STGetA", 0, &mut stacks, &mut state);
    assert_eq!(stacks.pop(), 255.0);
}

#[test]
fn stfilter_marks_a_script_texture_for_mipmaps() {
    let pack = ScratchPack::new("host-stfilter-test");
    let dir = pack.path();
    let script = dir.join("matrix.osc");
    std::fs::write(
        &script,
        "{trigger:matrix_refresh}\n0 (M.V.STLock)\n0 (M.V.STUnlock)\n0 (M.V.STFilter)\n{end}\n",
    )
    .unwrap();
    let p = compile(&CompileInput {
        scripts: vec![script],
        ..Default::default()
    });
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let mut host = VehicleHost::new(SimClock::default());
    host.script_textures.push(ScriptTexture::new(32, 16));
    host.script_textures[0].dirty = false;
    let mut st = State::new(&p);
    let mut vm = Vm::new();

    assert!(vm.run_trigger(&p, "matrix_refresh", &mut st, &mut host));
    assert!(!host.script_textures[0].locked);
    assert!(host.script_textures[0].mipmaps);
    assert!(host.script_textures[0].dirty);
}

#[test]
fn atron_unlock_filter_relock_publishes_the_released_image() {
    let pack = ScratchPack::new("host-atron-test");
    let dir = pack.path();
    let script = dir.join("atron.osc");
    std::fs::write(
        &script,
        "{trigger:draw}\n0 (M.V.STLock)\n0 255 255 255 255 (M.V.STSetColor)\n0 1 1 (M.V.STDrawPixel)\n0 (M.V.STUnlock)\n0 (M.V.STFilter)\n0 (M.V.STLock)\n0 2 1 (M.V.STDrawPixel)\n{end}\n{trigger:publish}\n0 (M.V.STUnlock)\n0 (M.V.STFilter)\n0 (M.V.STLock)\n{end}\n",
    ).unwrap();
    let p = compile(&CompileInput {
        scripts: vec![script],
        ..Default::default()
    });
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let mut host = VehicleHost::new(SimClock::default());
    host.script_textures.push(ScriptTexture::new(4, 2));
    let mut state = State::new(&p);
    let mut vm = Vm::new();
    assert!(vm.run_trigger(&p, "draw", &mut state, &mut host));
    let t = &mut host.script_textures[0];
    assert!(t.locked);
    assert!(t.mipmaps);
    let first = t
        .take_upload()
        .expect("STUnlock must survive STLock in the same frame");
    assert_eq!(&first[20..24], &[255; 4]);
    assert_eq!(
        &first[24..28],
        &[0; 4],
        "edits after relocking wait for the next unlock"
    );
    assert!(t.take_upload().is_none());
    assert!(vm.run_trigger(&p, "publish", &mut state, &mut host));
    let second = host.script_textures[0].take_upload().unwrap();
    assert_eq!(&second[24..28], &[255; 4]);
    assert!(vm.run_trigger(&p, "publish", &mut state, &mut host));
    assert!(
        host.script_textures[0].take_upload().is_none(),
        "filtering unchanged pixels needs no new upload"
    );
}

#[test]
fn atron_arrival_check_does_not_clear_sales_text_without_a_timetable() {
    let pack = ScratchPack::new("host-atron-arrival-test");
    let dir = pack.path();
    let script = dir.join("arrival.osc");
    std::fs::write(&script, "{trigger:check}\n(M.V.GetTTBusstopIndex) 1 - -1 >=\n{if}\n0 (M.V.STNewTex)\n{endif}\n{end}\n").unwrap();
    let p = compile(&CompileInput {
        scripts: vec![script],
        ..Default::default()
    });
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let mut host = VehicleHost::new(SimClock::default());
    host.script_textures.push(ScriptTexture::new(1, 1));
    host.script_textures[0].put(0, 0, [255; 4]);
    let mut state = State::new(&p);
    let mut vm = Vm::new();
    assert!(vm.run_trigger(&p, "check", &mut state, &mut host));
    assert_eq!(host.script_textures[0].rgba, vec![255; 4]);
    // The first stop of an actual timetable still reports an arrival.
    host.tt_stops.push(("First stop".into(), 0.0, 0.0));
    assert!(vm.run_trigger(&p, "check", &mut state, &mut host));
    assert_eq!(host.script_textures[0].rgba, vec![0; 4]);
    host.script_textures[0].put(0, 0, [255; 4]);
    host.tt_stops.clear();
    assert!(vm.run_trigger(&p, "check", &mut state, &mut host));
    assert_eq!(host.script_textures[0].rgba, vec![255; 4]);
}

/// The engine part of the stock SD200/SD202/NL202 collision block: a rear hit low down
/// damages the engine, one at the front only the general account.
#[test]
fn a_collision_block_reads_the_energy_twice() {
    let pack = ScratchPack::new("host-test");
    let dir = pack.path();
    let script = dir.join("collision.osc");
    std::fs::write(
        &script,
        "{trigger:collision}\n(L.L.collision_energy) (L.S.coll_energy) + (S.L.collision_energy)\n(L.S.coll_pos_y) -4.70 <\n(L.S.coll_pos_z) 1.10 < &&\n{if}\n(L.L.collision_energy_eng) (L.S.coll_energy) + (S.L.collision_energy_eng)\n{endif}\n{end}\n",
    )
        .unwrap();
    let vars = dir.join("vars.txt");
    std::fs::write(&vars, "collision_energy\ncollision_energy_eng\n").unwrap();
    let p = compile(&CompileInput {
        varlists: vec![vars],
        scripts: vec![script],
        ..Default::default()
    });
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let mut host = VehicleHost::new(SimClock::default());
    let mut st = State::new(&p);
    let mut vm = Vm::new();
    host.coll_pos = [-1.23, -5.73, 0.90];
    host.coll_energy = 115.5;
    assert!(vm.run_trigger(&p, "collision", &mut st, &mut host));
    host.coll_pos = [1.23, 5.62, 0.88];
    host.coll_energy = 20.0;
    vm.run_trigger(&p, "collision", &mut st, &mut host);
    assert_eq!(st.get(p.var("collision_energy").unwrap()), 135.5);
    assert_eq!(st.get(p.var("collision_energy_eng").unwrap()), 115.5);
}
