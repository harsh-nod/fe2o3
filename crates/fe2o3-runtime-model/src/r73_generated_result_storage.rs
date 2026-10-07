//! Result-peak and returned-buffer guards, not typed-storage or completion authority.

use crate::{R67ResourceKindV1, R67ResourceVectorV1};

/// One output reserves encoded plus typed bytes; a read-only return reserves one copy.
/// The ledger owner record is separate from every resource-vector coordinate.
pub const fn r73_generated_result_peak_v1(
    bytes: u64,
    typed_output: bool,
) -> Option<R67ResourceVectorV1> {
    if typed_output && bytes > u64::MAX / 2 {
        return None;
    }
    let peak = if typed_output { bytes * 2 } else { bytes };
    Some(R67ResourceVectorV1::ZERO.with(R67ResourceKindV1::ReplyBytes, peak))
}

/// This exact extent/access check does not authenticate the returned data's producer.
pub const fn r73_generated_result_shape_v1(
    expected_bytes: u64,
    actual_bytes: u64,
    capacity_bytes: u64,
    access_matches: bool,
) -> bool {
    expected_bytes == actual_bytes && actual_bytes == capacity_bytes && access_matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r73_peak_uses_only_reply_bytes_and_preserves_empty_members() {
        for bytes in [0, 1, 4, 4096, u64::MAX / 2] {
            for output in [false, true] {
                let charge = r73_generated_result_peak_v1(bytes, output).unwrap();
                for (index, value) in charge.counts().iter().copied().enumerate() {
                    assert_eq!(
                        value,
                        if index == R67ResourceKindV1::ReplyBytes as usize {
                            bytes * if output { 2 } else { 1 }
                        } else {
                            0
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn r73_typed_peak_overflow_rejects_without_rejecting_valid_read_only_extents() {
        for bytes in [u64::MAX / 2 + 1, u64::MAX - 1, u64::MAX] {
            assert_eq!(r73_generated_result_peak_v1(bytes, true), None);
            assert_eq!(
                r73_generated_result_peak_v1(bytes, false)
                    .unwrap()
                    .get(R67ResourceKindV1::ReplyBytes),
                bytes
            );
        }
    }

    #[test]
    fn r73_shape_requires_every_coordinate_including_empty_and_maximum_spans() {
        for bytes in [0, 1, 4096, u64::MAX] {
            assert!(r73_generated_result_shape_v1(bytes, bytes, bytes, true));
            assert!(!r73_generated_result_shape_v1(bytes, bytes, bytes, false));
            let other = bytes ^ 1;
            assert!(!r73_generated_result_shape_v1(other, bytes, bytes, true));
            assert!(!r73_generated_result_shape_v1(bytes, other, bytes, true));
            assert!(!r73_generated_result_shape_v1(bytes, bytes, other, true));
        }
    }
}
