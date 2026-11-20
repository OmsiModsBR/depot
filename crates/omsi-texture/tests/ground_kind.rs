//! The ground byte a texture's `.cfg` gives the faces drawn with it.

use omsi_texture::TextureCfg;

fn cfg(surface: i32, moisture: bool, puddles: bool) -> TextureCfg {
    TextureCfg {
        surface,
        moisture,
        puddles,
        ..TextureCfg::default()
    }
}

/// The kind in the low five bits, `0x20` for a surface that gets wet and `0x40` for one that
/// collects puddles; puddles wet it too.
#[test]
fn the_ground_byte_carries_the_kind_and_the_water() {
    assert_eq!(cfg(0, false, false).ground_kind(), 0);
    assert_eq!(cfg(2, false, false).ground_kind(), 2);
    assert_eq!(cfg(1, true, false).ground_kind(), 0x21);
    assert_eq!(cfg(16, false, true).ground_kind(), 0x70);
    assert_eq!(cfg(1, true, true).ground_kind(), 0x61);
}

/// A kind outside the five bits never spills into the water bits.
#[test]
fn a_kind_out_of_range_stays_in_its_bits() {
    assert_eq!(cfg(-3, false, false).ground_kind(), 0);
    assert_eq!(cfg(200, false, false).ground_kind(), 0x1f);
}
