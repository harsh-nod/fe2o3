use super::*;
use std::collections::BTreeSet;

#[test]
fn every_tile_value_has_one_exact_lane_item() {
    for format in [Format::Fp4E2M1, Format::Fp8E4M3] {
        for role in [Role::A, Role::B] {
            let mut coordinates = BTreeSet::new();
            for lane in 0..64 {
                for item in 0..32 {
                    let value = logical_value(format, role, lane, item).unwrap();
                    assert!(coordinates.insert((value.row_delta, value.column_delta)));
                    assert!(value.dword < 8);
                    assert!(value.shift < 32);
                }
            }
            let (rows, columns) = match role {
                Role::A => (16, 128),
                Role::B => (128, 16),
            };
            assert_eq!(coordinates.len(), 2048);
            for row in 0..rows {
                for column in 0..columns {
                    assert!(coordinates.contains(&(row, column)));
                }
            }
        }
    }
}

#[test]
fn fp4_consumes_one_byte_per_logical_value_not_packed_storage() {
    let view = LogicalView {
        offset: 13,
        rows: 16,
        columns: 128,
        stride: 139,
        allocation_len: 13 + 15 * 139 + 128,
    };
    assert_eq!(view.check_extent(), Ok(()));
    for lane in 0..64 {
        for item in 0..32 {
            let value = logical_value(Format::Fp4E2M1, Role::A, lane, item).unwrap();
            assert_eq!(
                view.source_byte(value, 0, 0),
                Some(13 + u64::from(lane % 16) * 139 + u64::from(lane / 16) * 32 + u64::from(item))
            );
        }
    }
}

#[test]
fn fp4_masks_high_nibbles_and_zeroes_upper_four_dwords() {
    for byte in 0..=255_u8 {
        let packed = pack_logical_values(Format::Fp4E2M1, &[byte; 32]);
        let expected = u32::from(byte & 15) * 0x1111_1111;
        assert_eq!(&packed[..4], &[expected; 4]);
        assert_eq!(&packed[4..], &[0; 4]);
    }
    for item in 0..32 {
        let mut values = [0; 32];
        values[item] = 0xaf;
        let registers = pack_logical_values(Format::Fp4E2M1, &values);
        for (word, register) in registers.into_iter().enumerate() {
            assert_eq!(
                register,
                if word == item / 8 {
                    15 << ((item % 8) * 4)
                } else {
                    0
                }
            );
        }
    }
}

#[test]
fn fp8_keeps_all_bits_and_both_depth_halves() {
    for lane in 0..64 {
        for item in 0..32 {
            let a = logical_value(Format::Fp8E4M3, Role::A, lane, item).unwrap();
            let b = logical_value(Format::Fp8E4M3, Role::B, lane, item).unwrap();
            let depth = if item < 16 {
                (lane / 16) * 16 + item
            } else {
                64 + (lane / 16) * 16 + item - 16
            };
            assert_eq!((a.row_delta, a.column_delta), (lane % 16, depth));
            assert_eq!((b.row_delta, b.column_delta), (depth, lane % 16));
        }
    }
    for byte in 0..=255_u8 {
        assert_eq!(
            pack_logical_values(Format::Fp8E4M3, &[byte; 32]),
            [u32::from(byte) * 0x0101_0101; 8]
        );
    }
}

#[test]
fn invalid_lane_and_item_are_not_modulo_normalized() {
    for format in [Format::Fp4E2M1, Format::Fp8E4M3] {
        for role in [Role::A, Role::B] {
            for lane in [64, 65, u32::MAX] {
                assert_eq!(logical_value(format, role, lane, 0), None);
            }
            for item in [32, 33, u32::MAX] {
                assert_eq!(logical_value(format, role, 0, item), None);
            }
        }
    }
}

#[test]
fn constructor_preserves_empty_extent_and_error_order() {
    let mut view = LogicalView {
        offset: 5,
        rows: 0,
        columns: 8,
        stride: 0,
        allocation_len: 5,
    };
    assert_eq!(view.check_extent(), Ok(()));
    view.offset = 6;
    assert_eq!(
        view.check_extent(),
        Err(ExtentError::OutOfBounds {
            required: 6,
            actual: 5
        })
    );
    view.rows = 2;
    view.offset = u64::MAX;
    assert_eq!(view.check_extent(), Err(ExtentError::InvalidStride));
    view.stride = u64::MAX;
    assert_eq!(view.check_extent(), Err(ExtentError::ExtentOverflow));
    view.rows = 1;
    view.columns = 1;
    assert_eq!(view.check_extent(), Err(ExtentError::ExtentOverflow));
}

#[test]
fn clipped_or_overflowed_coordinates_zero_fill_without_a_read() {
    let view = LogicalView {
        offset: 3,
        rows: 1,
        columns: 1,
        stride: 1,
        allocation_len: 4,
    };
    assert_eq!(view.check_extent(), Ok(()));
    let origin = logical_value(Format::Fp4E2M1, Role::A, 0, 0).unwrap();
    assert_eq!(view.source_byte(origin, 0, 0), Some(3));
    let next = logical_value(Format::Fp4E2M1, Role::A, 1, 1).unwrap();
    assert_eq!(view.source_byte(next, 0, 0), None);
    assert_eq!(view.source_byte(next, u64::MAX, 0), None);
    assert_eq!(view.source_byte(next, 0, u64::MAX), None);
    let invalid = LogicalView {
        offset: u64::MAX,
        rows: 2,
        columns: 2,
        stride: u64::MAX,
        allocation_len: u64::MAX,
    };
    assert_eq!(invalid.source_byte(next, 0, 0), None);
    let empty = LogicalView { rows: 0, ..view };
    assert_eq!(empty.source_byte(origin, 0, 0), None);
}

#[test]
fn all_lane_packings_match_source_order_on_strided_storage() {
    for format in [Format::Fp4E2M1, Format::Fp8E4M3] {
        for role in [Role::A, Role::B] {
            let mut storage = vec![0_u8; 40_000];
            for (index, byte) in storage.iter_mut().enumerate() {
                *byte = (index.wrapping_mul(37).wrapping_add(193) & 255) as u8;
            }
            let view = LogicalView {
                offset: 17,
                rows: 140,
                columns: 140,
                stride: 151,
                allocation_len: storage.len() as u64,
            };
            assert_eq!(view.check_extent(), Ok(()));
            for lane in 0..64 {
                let mut values = [0; 32];
                let mut expected = [0_u32; 8];
                for item in 0..32 {
                    let value = logical_value(format, role, lane, item).unwrap();
                    values[item as usize] =
                        storage[view.source_byte(value, 2, 3).unwrap() as usize];
                    let depth = match format {
                        Format::Fp4E2M1 => (lane / 16) * 32 + item,
                        Format::Fp8E4M3 if item < 16 => (lane / 16) * 16 + item,
                        Format::Fp8E4M3 => 64 + (lane / 16) * 16 + item - 16,
                    };
                    let (row, column) = match role {
                        Role::A => (2 + lane % 16, 3 + depth),
                        Role::B => (2 + depth, 3 + lane % 16),
                    };
                    let byte = storage[17 + row as usize * 151 + column as usize];
                    match format {
                        Format::Fp4E2M1 => {
                            expected[(item / 8) as usize] |=
                                u32::from(byte & 15) << ((item % 8) * 4)
                        }
                        Format::Fp8E4M3 => {
                            expected[(item / 4) as usize] |= u32::from(byte) << ((item % 4) * 8)
                        }
                    }
                }
                assert_eq!(pack_logical_values(format, &values), expected);
            }
        }
    }
}
