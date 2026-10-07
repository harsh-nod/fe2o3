//! Conservative constant-size byte coverage, separate from content digests.
//! Identity, exclusive custody and actual write completion belong to the caller.

include!("initialized_prefix_body.rs");

macro_rules! rust_expr {
    ($body:expr) => {
        $body
    };
}

pub(crate) const fn covers(prefix: u64, extent: u64, offset: u64, len: u64) -> bool {
    initialized_prefix_covers_body!(rust_expr, prefix, extent, offset, len)
}

/// A known-source completed write may extend a prefix but cannot bridge a gap.
/// An unknown-source write invalidates the intersecting suffix before effects.
pub(crate) const fn after_write(
    prefix: u64,
    extent: u64,
    offset: u64,
    len: u64,
    known: bool,
) -> Option<u64> {
    initialized_prefix_after_write_body!(rust_expr, prefix, extent, offset, len, known)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustive_small_byte_sets_match_conservative_prefix() {
        for extent in 0..=12 {
            for prefix in 0..=14 {
                for offset in 0..=14 {
                    for len in 0..=14 {
                        let valid = prefix <= extent && len != 0 && offset + len <= extent;
                        assert_eq!(
                            covers(prefix, extent, offset, len),
                            prefix <= extent && len != 0 && offset + len <= prefix
                        );
                        for known in [false, true] {
                            let result = after_write(prefix, extent, offset, len, known);
                            if !valid {
                                assert_eq!(result, None);
                                continue;
                            }
                            let mut bytes = [false; 12];
                            bytes[..prefix as usize].fill(true);
                            bytes[offset as usize..(offset + len) as usize].fill(known);
                            let expected = bytes[..extent as usize]
                                .iter()
                                .take_while(|value| **value)
                                .count() as u64;
                            assert_eq!(result, Some(expected));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn maximum_extents_do_not_wrap_or_admit_invalid_ranges() {
        assert!(covers(u64::MAX, u64::MAX, u64::MAX - 1, 1));
        assert!(!covers(u64::MAX, u64::MAX, u64::MAX, 1));
        assert!(!covers(8, 7, 0, 1));
        assert_eq!(after_write(0, u64::MAX, 0, u64::MAX, true), Some(u64::MAX));
        assert_eq!(after_write(u64::MAX, u64::MAX, u64::MAX, 1, true), None);
        assert_eq!(after_write(u64::MAX, u64::MAX, 1, u64::MAX, false), None);
        assert_eq!(after_write(0, 0, 0, 0, true), None);
    }

    #[test]
    fn sequential_windows_and_partial_overwrites_preserve_only_known_bytes() {
        let prefix = after_write(0, 32, 0, 12, true).unwrap();
        let prefix = after_write(prefix, 32, 12, 20, true).unwrap();
        assert!(covers(prefix, 32, 0, 32));
        assert_eq!(after_write(prefix, 32, 5, 3, true), Some(32));
        let prefix = after_write(prefix, 32, 5, 3, false).unwrap();
        assert_eq!(prefix, 5);
        assert_eq!(after_write(prefix, 32, 8, 24, true), Some(5));
        assert_eq!(after_write(prefix, 32, 5, 27, true), Some(32));
    }
}
