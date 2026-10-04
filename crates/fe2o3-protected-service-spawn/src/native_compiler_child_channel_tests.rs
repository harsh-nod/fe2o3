use crate::compiler_service_channel::{
    COMPILER_SERVICE_FD, TRANSFER_BYTES, Transfer, decode, encode_transfer, transfer_matches,
};

#[test]
fn existing_client_wire_is_exact_and_every_noncanonical_byte_refuses() {
    let expected = *b"FE2CEC2\0\x02\0\0\0\x04\x03\x02\x01\xc3\0\0\0\x08\x07\x06\x05";
    assert_eq!(TRANSFER_BYTES, 24);
    assert_eq!(COMPILER_SERVICE_FD, 195);
    assert_eq!(encode_transfer(0x01020304, 0x05060708), Some(expected));
    assert!(transfer_matches(&expected, 0x01020304, 0x05060708));
    assert_eq!(
        decode(&expected),
        Some(Transfer {
            child_pid: 0x01020304,
            parent_pid: 0x05060708
        })
    );
    for offset in 0..TRANSFER_BYTES {
        let mut changed = expected;
        changed[offset] ^= 1;
        assert!(!transfer_matches(&changed, 0x01020304, 0x05060708));
        if !(12..16).contains(&offset) && !(20..24).contains(&offset) {
            assert!(decode(&changed).is_none());
        }
    }
}

#[test]
fn wire_helpers_reject_zero_and_out_of_range_pids() {
    for pid in [0, i32::MAX as u32 + 1, u32::MAX] {
        assert!(encode_transfer(pid, 1).is_none());
        assert!(encode_transfer(1, pid).is_none());
        for (child, parent) in [(pid, 1), (1, pid)] {
            let mut wire = encode_transfer(1, 2).unwrap();
            wire[12..16].copy_from_slice(&child.to_le_bytes());
            wire[20..24].copy_from_slice(&parent.to_le_bytes());
            assert!(decode(&wire).is_none());
            assert!(!transfer_matches(&wire, child, parent));
        }
    }
    for (child, parent) in [(1, i32::MAX as u32), (i32::MAX as u32, 1)] {
        let wire = encode_transfer(child, parent).unwrap();
        assert!(transfer_matches(&wire, child, parent));
    }
}

#[test]
fn wire_helpers_reject_equal_child_and_parent_pids() {
    for pid in [1_u32, 42, i32::MAX as u32] {
        assert!(encode_transfer(pid, pid).is_none());
        let mut wire = encode_transfer(pid, 2).unwrap();
        wire[20..24].copy_from_slice(&pid.to_le_bytes());
        assert!(decode(&wire).is_none());
        assert!(!transfer_matches(&wire, pid, pid));
    }
}
