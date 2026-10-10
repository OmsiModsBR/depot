//! One ceiling for the sizes and counts the content parsers take from files, so that a
//! damaged or hostile file asks for a refusal and not for gigabytes. The limits are far above
//! anything OMSI 2 content uses.

/// The largest side (pixels) of a texture a content file may ask for (DDS, TGA).
pub const MAX_TEXTURE_DIM: u32 = 16_384;

/// The largest side of a texture the game draws itself (`[texttexture]`, `[htmltexture]`,
/// `[scripttexture]`).
pub const MAX_DYNAMIC_TEXTURE_DIM: u32 = 4_096;

/// Entries of a list in a content file (texture changes, entry points, scenery objects,
/// paths, ...).
pub const MAX_LIST_COUNT: usize = 1_000_000;

/// Strings per record of a `.hof` (`stringcount_terminus` / `stringcount_busstop`).
pub const MAX_HOF_STRINGS: usize = 256;

/// The camera numbers a vehicle's `reflexion<N>.bmp` may name.
pub const MAX_MIRRORS: usize = 32;

/// The size of an RGBA texture of `width` x `height` pixels (both at least 1) when each side
/// is at most `max_dim`: `(width, height, bytes)`. `None` for a side that is not positive or
/// is past the limit, so the caller refuses the texture (log and skip) before it allocates.
pub fn checked_texture_size(width: i64, height: i64, max_dim: u32) -> Option<(u32, u32, usize)> {
    let max = max_dim as i64;
    if width < 1 || height < 1 || width > max || height > max {
        return None;
    }
    let bytes = (width as usize).checked_mul(height as usize)?.checked_mul(4)?;
    Some((width as u32, height as u32, bytes))
}

/// A side read from a file for a texture the game draws: at most [`MAX_DYNAMIC_TEXTURE_DIM`]
/// (values below 1 stay as they are for the callers' own `max(1)`).
pub fn clamp_dynamic_dim(v: i32) -> i32 {
    v.min(MAX_DYNAMIC_TEXTURE_DIM as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_sizes() {
        assert_eq!(checked_texture_size(512, 256, MAX_TEXTURE_DIM), Some((512, 256, 512 * 256 * 4)));
        assert_eq!(checked_texture_size(0, 8, MAX_TEXTURE_DIM), None);
        assert_eq!(checked_texture_size(65_536, 65_536, MAX_TEXTURE_DIM), None);
        assert_eq!(checked_texture_size(i64::MAX, 2, MAX_TEXTURE_DIM), None);
        assert_eq!(clamp_dynamic_dim(i32::MAX), 4096);
        assert_eq!(clamp_dynamic_dim(-3), -3);
    }

    #[test]
    fn count_is_bounded_by_lines_left() {
        let f = crate::CfgFile::from_str("x.cfg", "[entries]\n99999999999\na\nb\n");
        let mut r = f.reader();
        r.next_keyword();
        assert_eq!(r.count(MAX_LIST_COUNT), 3);
    }
}
