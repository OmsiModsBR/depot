//! Little-endian numbers read out of a binary file at a byte offset.

/// The `u16` at `o`. Panics when `b` ends before it.
pub fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

/// The `u32` at `o`. Panics when `b` ends before it.
pub fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// The `u64` at `o`. Panics when `b` ends before it.
pub fn le64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([
        b[o],
        b[o + 1],
        b[o + 2],
        b[o + 3],
        b[o + 4],
        b[o + 5],
        b[o + 6],
        b[o + 7],
    ])
}

/// The `u16` at `o`, None when `b` ends before it.
pub fn get_le16(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o.checked_add(2)?).map(|_| le16(b, o))
}

/// The `u32` at `o`, None when `b` ends before it.
pub fn get_le32(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o.checked_add(4)?).map(|_| le32(b, o))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_at_an_offset() {
        let b = [0xAA, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        assert_eq!(le16(&b, 1), 0x0201);
        assert_eq!(le32(&b, 1), 0x0403_0201);
        assert_eq!(le64(&b, 1), 0x0807_0605_0403_0201);
        assert_eq!(get_le16(&b, 7), Some(0x0807));
        assert_eq!(get_le16(&b, 8), None);
        assert_eq!(get_le32(&b, 5), Some(0x0807_0605));
        assert_eq!(get_le32(&b, 6), None);
        assert_eq!(get_le32(&b, usize::MAX), None);
    }
}
