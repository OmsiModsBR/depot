use crate::support::scratch::ScratchPack;
use omsi_script::{compile, CompileInput};
use omsi_sim::testing::scenery::*;
use std::sync::Arc;

#[test]
fn busstop_frame_resolves_each_placements_texture() {
    let pack = ScratchPack::new("busstop-freetex");
    let dir = pack.path();
    let vars = dir.join("strings.txt");
    let script = dir.join("BusStop.osc");
    std::fs::write(&vars, "BusStop\nTexture\n").unwrap();
    std::fs::write(
        &script,
        concat!(
            "{init}\n{end}\n{frame}\n",
            "(L.$.BusStop) \"\" $= !\n{if}\n",
            "\"Busstop\\\" $+ (L.$.BusStop) $+ \".png\" $+ (S.$.Texture)\n",
            "{endif}\n{end}\n",
        ),
    )
    .unwrap();
    let program = Arc::new(compile(&CompileInput {
        stringvarlists: vec![vars],
        scripts: vec![script],
        ..Default::default()
    }));
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    for name in ["BentenDaini_1", "BentenDaini_2", ""] {
        let mut inst = SceneryInstance::new(
            program.clone(),
            &[],
            omsi_sim::SimClock::default(),
            &[name.to_string()],
        );
        inst.update(0.0, &SceneryVars::default());
        let expected = if name.is_empty() {
            String::new()
        } else {
            format!("Busstop\\{name}.png")
        };
        assert_eq!(inst.str_var("Texture"), expected);
    }
}
