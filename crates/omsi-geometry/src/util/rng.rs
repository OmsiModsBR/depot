//! A fast pseudo-random sequence for effects (rain, spray, coins, ambience): not for
//! anything that must be unpredictable.

/// One step of Marsaglia's xorshift64 (shifts 13, 7, 17): advances `state` and returns it.
/// A `state` of 0 stays 0.
pub fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// The next number of `state`'s sequence in 0..1, from its top 24 bits.
pub fn unit_f32(state: &mut u64) -> f32 {
    (xorshift64(state) >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sequence_is_marsaglias() {
        let mut s = 1u64;
        assert_eq!(xorshift64(&mut s), 1_082_269_761);
        assert_eq!(s, 1_082_269_761);
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..1000 {
            let u = unit_f32(&mut s);
            assert!((0.0..1.0).contains(&u));
        }
    }
}
