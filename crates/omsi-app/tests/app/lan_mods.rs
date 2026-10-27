use openomsi_game::testing::lan_mods::*;

#[test]
fn paths_that_are_refused() {
    assert!(refuse_path("maps/Ahlheim/global.cfg").is_none());
    assert!(refuse_path("Vehicles/LiAZ/Model/model.cfg").is_none());
    assert!(refuse_path("Sceneryobjects/X/texture/a.dds").is_none());
    for bad in [
        "../etc/passwd",
        "maps/../../x.cfg",
        "/abs/maps/x",
        "C:/Windows/x.cfg",
        "maps\\x\\y.cfg",
        "Plugins/evil.dll",
        "Vehicles/X/Plugins/a.cfg",
        "Vehicles/X/setup.exe",
        "maps/x/run.BAT",
        "maps/x/a.sh",
        "maps/x/lib.dylib",
        "maps/x/pack.zip",
        "Startup/x.cfg",
        "x.cfg",
        "maps//x.cfg",
        "maps/x/y?.cfg",
    ] {
        assert!(refuse_path(bad).is_some(), "{bad} should be refused");
    }
}

#[test]
fn programs_are_seen_by_their_content() {
    assert!(looks_executable(b"MZ\x90\x00"));
    assert!(looks_executable(b"\x7fELF\x02"));
    assert!(looks_executable(b"#!/bin/sh"));
    assert!(looks_executable(&[0xcf, 0xfa, 0xed, 0xfe]));
    assert!(!looks_executable(b"DDS "));
    assert!(!looks_executable(b"[mesh]"));
}
