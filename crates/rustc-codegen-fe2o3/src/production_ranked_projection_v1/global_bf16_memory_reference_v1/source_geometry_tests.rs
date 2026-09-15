// Independent executable specifications of views.rs/tensor.rs. These tests do
// not issue source constructor, allocation, or memory-refinement evidence.
mod source_geometry_tests {
    use super::*;

    fn constructor_extent(offset: u64, rows: u64, columns: u64, stride: u64) -> Option<u64> {
        if rows == 0 || columns == 0 {
            return Some(offset);
        }
        if stride < columns {
            return None;
        }
        offset.checked_add((rows - 1).checked_mul(stride)?.checked_add(columns)?)
    }

    #[test]
    fn bf16_checked_constructor_empty_and_tail_requests_preserve_physical_extent() {
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            for (offset, rows, columns, stride) in [
                (7, 15, 13, 31),
                (0, 1, 1, 1),
                (17, 0, u64::MAX, 0),
                (17, u64::MAX, 0, u64::MAX),
            ] {
                let length = constructor_extent(offset, rows, columns, stride).unwrap();
                let schedule = source_read_schedule_v1(contract(role), inputs(stride)).unwrap();
                for lane in 0..64 {
                    let values = [lane, offset, rows, columns, 0, 0, length];
                    for event in &schedule.events {
                        let actual =
                            (eval(&event.guard, &values) != 0).then(|| eval(&event.index, &values));
                        assert_eq!(
                            actual,
                            native_source(role, event.component, stride, &values)
                        );
                        if rows == 0 || columns == 0 {
                            assert_eq!(actual, None);
                        }
                        if let Some(index) = actual {
                            assert!(index < length);
                        }
                    }
                }
            }
        }
        assert_eq!(constructor_extent(17, 0, u64::MAX, u64::MAX), Some(17));
        // A zero-sized logical shape still cannot move the physical offset past len.
        assert!(constructor_extent(17, 0, u64::MAX, u64::MAX).unwrap() > 16);
    }

    #[test]
    fn bf16_checked_constructor_overflow_cannot_be_replaced_by_wrapping_extent() {
        assert_eq!(constructor_extent(0, 16, 17, 16), None);
        assert_eq!(constructor_extent(0, u64::MAX, 1, 2), None);
        assert_eq!(constructor_extent(u64::MAX, 1, 1, 1), None);
        assert_eq!(constructor_extent(0, 2, 2, u64::MAX), None);
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            let schedule = source_read_schedule_v1(contract(role), inputs(31)).unwrap();
            for lane in 0..64 {
                for (first, second) in [(u64::MAX, 0), (0, u64::MAX), (u64::MAX - 1, u64::MAX - 1)]
                {
                    let values = [lane, 7, 15, 13, first, second, 454];
                    for event in &schedule.events {
                        assert_eq!(
                            (eval(&event.guard, &values) != 0).then(|| eval(&event.index, &values)),
                            native_source(role, event.component, 31, &values)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bf16_tail_events_preserve_u16_nan_zero_and_subnormal_payload_bits() {
        let bits: [u16; 8] = [0, 0x8000, 1, 0x007f, 0x7f80, 0xff80, 0x7fc1, 0x7f81];
        for role in [SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B] {
            let schedule = source_read_schedule_v1(contract(role), inputs(4)).unwrap();
            let mut observed = std::collections::BTreeSet::new();
            for lane in 0..64 {
                let values = [lane, 0, 2, 4, 0, 0, bits.len() as u64];
                for event in &schedule.events {
                    let expected = native_source(role, event.component, 4, &values);
                    let present = eval(&event.guard, &values) != 0;
                    assert_eq!(present, expected.is_some());
                    let result = if present {
                        let index = eval(&event.index, &values);
                        observed.insert(index);
                        bits[index as usize]
                    } else {
                        assert_eq!(event.fallback.scalar(), BITS);
                        u16::try_from(eval(&event.fallback, &values)).unwrap()
                    };
                    assert_eq!(
                        result,
                        expected.map(|index| bits[index as usize]).unwrap_or(0)
                    );
                }
            }
            assert_eq!(observed, (0..bits.len() as u64).collect());
        }
    }

    #[test]
    fn bf16_requests_do_not_invent_dynamic_constructor_or_lane_authority() {
        let mut source = inputs(16);
        source.stride = symbol(3);
        assert_eq!(
            source_read_schedule_v1(contract(SemanticMfmaOperandRoleV1::A), source).err(),
            Some(CpuReadRequestErrorV1::UnsupportedDynamicStride)
        );
        let schedule =
            source_read_schedule_v1(contract(SemanticMfmaOperandRoleV1::B), inputs(16)).unwrap();
        assert_eq!(
            eval(&schedule.lane_precondition, &[64, 0, 16, 16, 0, 0, 256]),
            0
        );
        // Numeric lane range and a read schedule alone are not source lane custody.
    }
}
