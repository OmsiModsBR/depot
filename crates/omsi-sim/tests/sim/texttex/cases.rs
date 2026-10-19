use omsi_content::font::Font;
use omsi_sim::testing::texttex::*;
use std::path::Path;

/// A full-colour font whose colour bitmap is not the alpha's size takes each glyph
/// pixel's colour from the same pixel of the colour bitmap, as OMSI reads it, not
/// the alpha's white (#829).
#[test]
fn a_colour_bitmap_of_another_size_still_colours_the_glyphs() {
    let dir = std::path::PathBuf::from("/fonts");
    let font = Font {
        path: dir.join("colour.oft"),
        name: "Colour".into(),
        bitmap: "colour.bmp".into(),
        alpha: "colour_alpha.bmp".into(),
        height: 4,
        gap: 0,
        chars: Vec::new(),
    };
    let mut lib = FontLibrary::new(&dir);
    lib.set_index(vec![font]);
    // a 4 x 4 white alpha under a 2 x 2 swatch: red, green / blue, yellow
    let swatch = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
    ];
    let decode = |p: &Path| -> Option<(u32, u32, Vec<u8>)> {
        match p.file_name()?.to_str()? {
            "colour_alpha.bmp" => Some((4, 4, vec![255; 64])),
            "colour.bmp" => Some((2, 2, swatch.concat())),
            _ => None,
        }
    };
    let atlas = lib.get("Colour", &decode).expect("the font loads");
    assert_eq!((atlas.width, atlas.height), (4, 4));
    for (x, y) in [(0usize, 0usize), (1, 0), (0, 1), (1, 1), (3, 2)] {
        let i = (y * 4 + x) * 4;
        assert_eq!(
            atlas.color[i..i + 4],
            swatch[(y % 2) * 2 + x % 2],
            "pixel ({x}, {y})"
        );
    }
}

#[test]
fn font_families() {
    assert_eq!(
        family_of("churafont++ Numeric 26x11 Bold"),
        "churafont++ numeric 26x11"
    );
    assert_eq!(family_of("churafont++ 32x10 Heavy"), "churafont++ 32x10");
    assert_eq!(family_of("churafont++ 14x10"), "churafont++ 14x10");
    assert_eq!(family_of("Bold"), "bold");
    assert_eq!(family_of("LEERFELD"), "leerfeld");
}
