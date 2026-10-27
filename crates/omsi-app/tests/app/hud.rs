use omsi_content::font::FontAtlas;
use openomsi_game::testing::hud::*;

/// Every glyph 10 px wide with no gap.
fn font() -> FontAtlas {
    let chars = "abcdefghijklmnopqrstuvwxyz:"
        .chars()
        .map(|ch| omsi_content::font::FontChar {
            ch,
            x0: 0,
            x1: 10,
            y: 0,
        })
        .collect();
    FontAtlas::new(
        omsi_content::font::Font {
            height: 12,
            gap: 0,
            chars,
            ..Default::default()
        },
        1,
        1,
        vec![0; 4],
        vec![0; 4],
    )
}

#[test]
fn long_lines_break_between_words() {
    let f = font();
    assert_eq!(wrap(&f, "short line", 200), vec!["short line"]);
    // a space without a glyph counts as the gap (1 px)
    assert_eq!(
        wrap(&f, "no duty:   line five does not run", 100),
        vec!["no duty:", "line five", "does not", "run"]
    );
    assert_eq!(
        wrap(&f, "unbreakablewordhere ok", 100),
        vec!["unbreakablewordhere", "ok"]
    );
    for row in wrap(
        &f,
        "no duty: line five does not run on that date at all",
        250,
    ) {
        assert!(f.text_width(&row) <= 250, "{row}");
    }
    assert_eq!(
        legible(&f, "line 5 & 5N: 1000_FPW"),
        "line 5 + 5N: 1000 FPW"
    );
}

/// A glyph pixel stays white and opaque, is ringed by a dark outline several pixels
/// wide, and a plate runs behind the row as far as the text goes.
#[test]
fn rows_get_an_outline_and_a_plate() {
    let (w, lh) = (40u32, 12u32);
    let mut row = vec![0u8; (w * lh * 4) as usize];
    let at = |x: u32, y: u32| ((y * w + x) * 4) as usize;
    row[at(10, 6)..at(10, 6) + 4].copy_from_slice(&[255, 255, 255, 255]);
    let mut img = vec![0u8; (w * lh * 4) as usize];
    compose_row(&mut img, &row, w, lh, 0);
    let x = 10 + OUTLINE as u32;
    assert_eq!(&img[at(x, 6)..at(x, 6) + 4], &[255, 255, 255, 255]);
    let ring = &img[at(x + 2, 6)..at(x + 2, 6) + 4];
    assert!(ring[0] == 0 && ring[3] > 200, "{ring:?}");
    let plate = &img[at(x + 8, 1)..at(x + 8, 1) + 4];
    assert!(plate[0] == 0 && (90..120).contains(&plate[3]), "{plate:?}");
    assert_eq!(img[at(30, 6) + 3], 0);
}
