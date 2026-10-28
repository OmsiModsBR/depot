/// The Mods page installs a .zip only; loose folders are copied by hand.
#[test]
fn only_a_zip_is_installed_from_the_page() {
    let dir = std::env::temp_dir().join(format!("depot-is-zip-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("Folder.zip")).unwrap();
    std::fs::create_dir_all(dir.join("Example bus")).unwrap();
    for f in ["Example bus.zip", "Map.ZIP", "pack.rar"] {
        std::fs::write(dir.join(f), b"PK").unwrap();
    }
    let is = |name: &str| {
        openomsi_game::testing::launcher::state::is_zip(&dir.join(name).to_string_lossy())
    };
    let seen = (
        is("Example bus.zip"),
        is("Map.ZIP"),
        is("Example bus"),
        is("pack.rar"),
        is("Folder.zip"),
        is("Missing.zip"),
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(seen, (true, true, false, false, false, false));
}
