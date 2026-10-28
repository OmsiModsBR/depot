#[test]
fn only_a_run_that_died_compiling_on_vulkan_is_told_so() {
    let compiled = "[t INFO r] opening graphics device: Mali (Vulkan, vendor 0x13b5)\n[t INFO r] renderer: compiling the scene shaders\n";
    assert!(openomsi_game::testing::launcher::dialogs::died_compiling_on_vulkan(compiled));
    // compiled long ago, then ran out of memory loading
    assert!(
        !openomsi_game::testing::launcher::dialogs::died_compiling_on_vulkan(&format!(
            "{compiled}[t INFO g] status: 63 fps, view driver\n[t INFO m] loading tiles 75 %\n"
        ))
    );
    // on OpenGL it is never the Vulkan driver
    assert!(!openomsi_game::testing::launcher::dialogs::died_compiling_on_vulkan("[t INFO r] opening graphics device: Mali (Gl, vendor 0x13b5)\n[t INFO r] renderer: compiling the scene shaders\n"));
}
