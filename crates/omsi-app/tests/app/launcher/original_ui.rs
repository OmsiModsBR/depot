use omsi_launcher_lib::original_engine::install::InstallPlan;
use omsi_launcher_lib::original_engine::install::PlannedFile;
use openomsi_game::testing::launcher::original_ui::*;

#[test]
fn files_are_summed_up_by_folder_with_root_files_first() {
    let f = |p: &str, b| PlannedFile {
        path: p.into(),
        bytes: b,
    };
    let plan = InstallPlan {
        files: vec![
            f("plugins/a.dll", 10),
            f("OmsiLaunch.exe", 5),
            f("plugins/b.dll", 20),
            f(".omsilaunch/docs/x.md", 1),
        ],
        total_bytes: 36,
        replaces: vec![],
        conflicts: vec![],
    };
    let s = summarize(&plan);
    assert_eq!(s[0], ("OmsiLaunch.exe".to_string(), 1, 5));
    assert_eq!(s[1], ("plugins\\".to_string(), 2, 30));
    assert_eq!(s[2], (".omsilaunch\\".to_string(), 1, 1));
}
