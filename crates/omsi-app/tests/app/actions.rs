/// The stock cars' gates keep the gear in `antrieb_getr_gang` (#866).
#[test]
fn the_gear_is_read_where_the_gates_store_it() {
    let program = |vars: &str, osc: &str| {
        let dir =
            std::env::temp_dir().join(format!("omsi_gates_{}_{}", std::process::id(), vars.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let (vl, sc) = (dir.join("varlist.txt"), dir.join("antrieb.osc"));
        std::fs::write(&vl, vars).unwrap();
        std::fs::write(&sc, osc).unwrap();
        let p = omsi_script::compile(&omsi_script::CompileInput {
            varlists: vec![vl],
            scripts: vec![sc],
            ..Default::default()
        });
        let _ = std::fs::remove_dir_all(&dir);
        p
    };
    let stock = program("antrieb_getr_gang\n", "{trigger:kw_s_1_fest}\n{trigger:kw_s_1}\n1 (S.L.antrieb_getr_gang)\n{end}\n{end}\n{trigger:kw_s_2}\n2 (S.L.antrieb_getr_gang)\n{end}\n");
    assert_eq!(
        openomsi_game::testing::actions::gate_gear_var(&stock).as_deref(),
        Some("antrieb_getr_gang")
    );
    // a lever moved by every gate is not the gear
    let own = program("lever_moved\nmy_gear\n", "{trigger:kw_s_1}\n1 (S.L.lever_moved)\n1 (S.L.my_gear)\n{end}\n{trigger:kw_s_2}\n1 (S.L.lever_moved)\n2 (S.L.my_gear)\n{end}\n");
    assert_eq!(
        openomsi_game::testing::actions::gate_gear_var(&own).as_deref(),
        Some("my_gear")
    );
}
