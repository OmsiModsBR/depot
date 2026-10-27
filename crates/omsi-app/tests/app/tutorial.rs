#[test]
fn text() {
    let (t, x) = openomsi_game::testing::tutorial::page_text(
        "<style>b{}</style><h2>Hello!</h2><p>One &quot;two&quot;</p><p>Three</p>",
    );
    assert_eq!(t, "Hello!");
    assert_eq!(x, "One \"two\"\nThree");
}
