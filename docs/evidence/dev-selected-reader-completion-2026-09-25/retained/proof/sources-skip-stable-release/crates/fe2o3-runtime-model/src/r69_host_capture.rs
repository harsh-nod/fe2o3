//! Pure range validation for an owned host capture destination.
//!
//! Source identity, incarnation, initialization, coherence and quiescence are
//! separate adapter obligations. This predicate grants no read or completion
//! authority and does not establish destination allocation or credit ownership.

/// Accepts exactly a nonempty, nonoverflowing range with an exact-size destination.
pub const fn r69_host_capture_range_v1(
    offset: u64,
    byte_len: u64,
    allocation_bytes: u64,
    destination_bytes: u64,
) -> bool {
    if byte_len == 0 || byte_len != destination_bytes {
        return false;
    }
    match offset.checked_add(byte_len) {
        Some(end) => end <= allocation_bytes,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_nonempty_ranges_include_allocation_end() {
        for (offset, length, allocation) in [
            (0, 1, 1),
            (0, 8, 8),
            (3, 5, 8),
            (3, 4, 8),
            (0, u64::MAX, u64::MAX),
            (u64::MAX - 1, 1, u64::MAX),
        ] {
            assert!(r69_host_capture_range_v1(
                offset, length, allocation, length
            ));
        }
    }

    #[test]
    fn zero_length_is_rejected_even_with_empty_destination() {
        for (offset, allocation) in [(0, 0), (0, 8), (8, 8), (u64::MAX, u64::MAX)] {
            assert!(!r69_host_capture_range_v1(offset, 0, allocation, 0));
        }
    }

    #[test]
    fn destination_must_be_neither_shorter_nor_longer() {
        for destination in [0, 1, 7, 9, u64::MAX] {
            assert!(!r69_host_capture_range_v1(0, 8, 8, destination));
        }
    }

    #[test]
    fn wrapping_end_is_rejected_even_when_wrapped_value_would_fit() {
        for (offset, length) in [(u64::MAX, 1), (u64::MAX - 1, 2), (u64::MAX, u64::MAX)] {
            assert!(!r69_host_capture_range_v1(offset, length, u64::MAX, length));
        }
    }

    #[test]
    fn nonoverflowing_end_must_fit_allocation() {
        for (offset, length, allocation) in [(0, 1, 0), (0, 9, 8), (3, 6, 8), (9, 1, 8)] {
            assert!(!r69_host_capture_range_v1(
                offset, length, allocation, length
            ));
        }
    }

    #[test]
    fn boundary_product_matches_wider_integer_oracle() {
        let boundaries = [0, 1, 2, 7, 8, u64::MAX - 1, u64::MAX];
        for offset in boundaries {
            for length in boundaries {
                for allocation in boundaries {
                    for destination in boundaries {
                        let end = u128::from(offset) + u128::from(length);
                        let expected = length != 0
                            && length == destination
                            && end <= u128::from(u64::MAX)
                            && end <= u128::from(allocation);
                        assert_eq!(
                            r69_host_capture_range_v1(offset, length, allocation, destination),
                            expected,
                            "offset={offset} length={length} allocation={allocation} destination={destination}"
                        );
                    }
                }
            }
        }
    }
}
