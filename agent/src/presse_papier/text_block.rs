//! The length of the text in a `CF_UNICODETEXT` block, bounded by the block.
//!
//! **Pure, outside the `#[cfg(windows)]` of `win32.rs`, to be tested on the
//! host.** Windows does not enforce the terminator of `CF_UNICODETEXT`: any
//! process of the session can put an unterminated block on the clipboard,
//! and scanning for a NUL without a bound would read past the allocation.
//! The bound is the block's size (`GlobalSize`), in UTF-16 units.

/// The number of UTF-16 units before the first NUL of `units`, or all of
/// them when the block carries no NUL.
pub(super) fn utf16_length(units: &[u16]) -> usize {
    units.iter().position(|&u| u == 0).unwrap_or(units.len())
}

#[cfg(test)]
mod tests {
    use super::utf16_length;

    #[test]
    fn the_text_stops_at_the_first_nul() {
        assert_eq!(utf16_length(&[0x48, 0x69, 0, 0x41, 0]), 2);
        assert_eq!(utf16_length(&[0, 0x41]), 0);
    }

    #[test]
    fn an_unterminated_block_is_read_to_its_end_and_no_further() {
        assert_eq!(utf16_length(&[0x48, 0x69, 0x21]), 3);
        assert_eq!(utf16_length(&[]), 0);
    }
}
