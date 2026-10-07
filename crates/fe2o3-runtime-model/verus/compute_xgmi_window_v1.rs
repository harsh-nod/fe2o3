//! Exact shared window/packet arithmetic, not allocation, mapping or DMA refinement.
use vstd::prelude::*;
include!("../../fe2o3-kfd/src/sdma/compute_xgmi_window_body.rs");

verus! {

spec fn bounded_window(
    source_len: u64, destination_len: u64,
    source_offset: u64, destination_offset: u64, bytes: u64,
) -> bool {
    bytes > 0
        && source_offset as int + bytes as int <= source_len as int
        && destination_offset as int + bytes as int <= destination_len as int
}

spec fn relative_packet(bytes: u64, relative: u64, packet_bytes: u32) -> bool {
    packet_bytes > 0 && relative as int + packet_bytes as int <= bytes as int
}

spec fn projected_packet(
    source_offset: u64, destination_offset: u64,
    bytes: u64, relative: u64, packet_bytes: u32,
) -> bool {
    relative_packet(bytes, relative, packet_bytes)
        && source_offset as int + relative as int + packet_bytes as int <= u64::MAX as int
        && destination_offset as int + relative as int + packet_bytes as int <= u64::MAX as int
}

fn compute_xgmi_window_bounds_v1(
    source_len: u64, destination_len: u64,
    source_offset: u64, destination_offset: u64, bytes: u64,
) -> (result: bool)
    ensures result == bounded_window(source_len, destination_len, source_offset, destination_offset, bytes),
{
    gfx942_compute_xgmi_window_bounds_v1!(source_len, destination_len, source_offset, destination_offset, bytes)
}

fn compute_xgmi_window_packet_v1(
    source_offset: u64, destination_offset: u64,
    bytes: u64, relative: u64, packet_bytes: u32,
) -> (result: Option<(u64, u64)>)
    ensures
        result.is_some() == projected_packet(source_offset, destination_offset, bytes, relative, packet_bytes),
        match result {
            None => true,
            Some(pair) => pair.0 as int == source_offset as int + relative as int
                && pair.1 as int == destination_offset as int + relative as int
                && pair.0 as int + packet_bytes as int <= u64::MAX as int
                && pair.1 as int + packet_bytes as int <= u64::MAX as int,
        },
{
    gfx942_compute_xgmi_window_packet_v1!(source_offset, destination_offset, bytes, relative, packet_bytes)
}

// Composition of the two shared bodies. The real packet planner supplies
// relative/packet_bytes; its packet-count/coverage theorem remains separate.
fn compute_xgmi_bounded_window_packet_v1(
    source_len: u64, destination_len: u64,
    source_offset: u64, destination_offset: u64,
    bytes: u64, relative: u64, packet_bytes: u32,
) -> (result: Option<(u64, u64)>)
    ensures
        result.is_some() == (bounded_window(source_len, destination_len, source_offset, destination_offset, bytes)
            && relative_packet(bytes, relative, packet_bytes)),
        match result {
            None => true,
            Some(pair) => pair.0 as int == source_offset as int + relative as int
                && pair.1 as int == destination_offset as int + relative as int
                && source_offset <= pair.0
                && destination_offset <= pair.1
                && pair.0 as int + packet_bytes as int <= source_offset as int + bytes as int
                && pair.1 as int + packet_bytes as int <= destination_offset as int + bytes as int
                && pair.0 as int + packet_bytes as int <= source_len as int
                && pair.1 as int + packet_bytes as int <= destination_len as int,
        },
{
    if compute_xgmi_window_bounds_v1(source_len, destination_len, source_offset, destination_offset, bytes) {
        compute_xgmi_window_packet_v1(source_offset, destination_offset, bytes, relative, packet_bytes)
    } else {
        None
    }
}

}
