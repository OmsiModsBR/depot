use openomsi_game::testing::env_file::*;

#[test]
fn the_settable_switches_are_the_reviewed_ones() {
    let mut got: Vec<&str> = omsi_cfg::flags::ALL
        .iter()
        .filter(|f| f.env_txt)
        .map(|f| f.name)
        .collect();
    got.sort_unstable();
    let mut want = vec![
        "OMSI_TOUCH",
        "OMSI_MOBILE",
        "OMSI_LAUNCHER_SIZE",
        "OMSI_BACKEND",
        "OMSI_SAFE_GPU",
        "OMSI_FULL_GPU",
        "OMSI_GPU_LIMITS",
        "OMSI_BASIC_PIPELINES",
        "OMSI_GRAPHICS",
        "OMSI_NO_SHADOWS",
        "OMSI_NO_CORONAS",
        "OMSI_NO_ENVMAP",
        "OMSI_NO_BUMP",
        "OMSI_NO_CULL",
        "OMSI_NO_SURF",
        "OMSI_NO_BC",
        "OMSI_NO_TEXCOMPRESS",
        "OMSI_NO_FXAA",
        "OMSI_NO_AO",
        "OMSI_NO_RT",
        "OMSI_NO_RT_GRADE",
        "OMSI_NO_CLOUDS",
        "OMSI_NO_ENHANCED",
        "OMSI_NO_PLUGINS",
        "OMSI_MAX_FPS",
        "OMSI_TEXTURE_MEMORY",
        "OMSI_MUTE",
        "OMSI_SEED",
        "OMSI_PROFILE",
        "OMSI_GPU_TIMERS",
        "OMSI_DEBUG_NAV",
        "OMSI_DEBUG_STARTUP",
        "OMSI_DEBUG_MISSING",
    ];
    want.sort_unstable();
    // (a variable marked `env_txt` in the register, or unmarked, changes what a file other
    // apps can write may set: look at it, then change this list)
    assert_eq!(got, want);
}

#[test]
fn only_harmless_switches_are_taken() {
    let mut skipped = Vec::new();
    let got = parse(
        "OMSI_NO_SHADOWS=1\n# a comment\nOMSI_MAX_FPS = 30 \nOMSI_BACKEND=gl\n",
        &mut skipped,
    );
    assert_eq!(
        got,
        vec![
            ("OMSI_NO_SHADOWS".into(), "1".into()),
            ("OMSI_MAX_FPS".into(), "30".into()),
            ("OMSI_BACKEND".into(), "gl".into())
        ]
    );
    assert!(skipped.is_empty());
}

#[test]
fn paths_programs_urls_and_scripts_are_refused() {
    let mut skipped = Vec::new();
    let text = "OMSI_ROOT=/sdcard/evil\nOMSI_CONTENT=/sdcard/evil\nOMSI_CONTENT_ZIP=/sdcard/e.zip\nOMSI_BIN=/data/x\nOMSI_WINE=/bin/sh\nOMSI_CLOUDFLARED=/bin/sh\n\
OMSI_PRESENCE_URL=https://evil.example\nOMSI_INPUT=t=1 key F1\nOMSI_LAUNCHER_INPUT=t=1 click 1,1\nLD_PRELOAD=/sdcard/x.so\nPATH=/sdcard\nHOME=/sdcard\n";
    assert!(parse(text, &mut skipped).is_empty());
    assert_eq!(skipped.len(), 12, "{skipped:?}");
}

#[test]
fn an_allowed_switch_with_a_strange_value_is_refused() {
    assert!(allowed("OMSI_MAX_FPS", "60"));
    assert!(!allowed("OMSI_MAX_FPS", "60; rm -rf"));
    assert!(!allowed("OMSI_BACKEND", "/etc/passwd"));
    assert!(!allowed("OMSI_BACKEND", &"a".repeat(MAX_VALUE + 1)));
    assert!(!allowed("omsi_no_shadows", "1"));
}
