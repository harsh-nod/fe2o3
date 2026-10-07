// Executable range guard corresponding to src/r69_host_capture.rs.
// This verifies range arithmetic, not descriptor extraction, private witness
// issuance, source coherence, native reads, destination ownership or whole drain.
use vstd::prelude::*;
verus! {
pub open spec fn capture_range_v1(
    offset: u64, byte_len: u64, allocation_bytes: u64, destination_bytes: u64,
) -> bool {
    &&& byte_len > 0
    &&& byte_len == destination_bytes
    &&& offset as int + byte_len as int <= u64::MAX as int
    &&& offset as int + byte_len as int <= allocation_bytes as int
}

pub fn host_capture_range_v1(
    offset: u64, byte_len: u64, allocation_bytes: u64, destination_bytes: u64,
) -> (accepted: bool)
    ensures accepted == capture_range_v1(offset, byte_len, allocation_bytes, destination_bytes),
{
    if byte_len == 0 || byte_len != destination_bytes { return false; }
    match offset.checked_add(byte_len) {
        Some(end) => end <= allocation_bytes,
        None => false,
    }
}

pub proof fn accepted_range_is_exact_v1(
    offset: u64, byte_len: u64, allocation_bytes: u64, destination_bytes: u64,
)
    requires capture_range_v1(offset, byte_len, allocation_bytes, destination_bytes),
    ensures
        byte_len > 0,
        byte_len == destination_bytes,
        (offset as int) < (offset as int) + (byte_len as int),
        offset as int + byte_len as int <= allocation_bytes as int,
{}

pub proof fn overflowing_range_is_rejected_v1(
    offset: u64, byte_len: u64, allocation_bytes: u64, destination_bytes: u64,
)
    requires offset as int + byte_len as int > u64::MAX as int,
    ensures !capture_range_v1(offset, byte_len, allocation_bytes, destination_bytes),
{}

pub proof fn outside_allocation_is_rejected_v1(
    offset: u64, byte_len: u64, allocation_bytes: u64, destination_bytes: u64,
)
    requires offset as int + byte_len as int > allocation_bytes as int,
    ensures !capture_range_v1(offset, byte_len, allocation_bytes, destination_bytes),
{}
}
