//! Arithmetic of the actual shared packet bodies, not mapping or DMA refinement.
use vstd::prelude::*;
include!("../../fe2o3-kfd/src/sdma/compute_xgmi_plan_body.rs");

verus! {

const PACKET_BYTES: u64 = 0x003f_ffe0;
const MAX_PACKETS: u64 = 4096;

spec fn admitted(total: u64) -> bool {
    0 < total <= PACKET_BYTES as int * MAX_PACKETS as int
}

spec fn exact_count(total: u64) -> int {
    (total as int - 1) / PACKET_BYTES as int + 1
}

fn compute_xgmi_packet_count_v1(total: u64) -> (result: Option<usize>)
    ensures
        match result {
            None => !admitted(total),
            Some(count) => admitted(total)
                && count as int == exact_count(total)
                && 1 <= count <= MAX_PACKETS
                && (count as int - 1) * (PACKET_BYTES as int) < total
                && total <= (count as int) * (PACKET_BYTES as int),
        },
{
    compute_xgmi_packet_count_body_v1!(total, PACKET_BYTES, MAX_PACKETS)
}

fn compute_xgmi_packet_at_v1(total: u64, count: usize, index: usize)
    -> (result: Option<(u64, u32)>)
    requires admitted(total), count as int == exact_count(total),
    ensures
        result.is_some() == (index < count),
        match result {
            None => true,
            Some(packet) => packet.0 as int == index as int * PACKET_BYTES as int
                && 0 < packet.1 <= PACKET_BYTES
                && packet.0 as int + packet.1 as int <= total
                && packet.1 as int == if total as int - (packet.0 as int) < PACKET_BYTES {
                    total as int - packet.0 as int
                } else { PACKET_BYTES as int }
                && (packet.0 as int + packet.1 as int == total) == (index + 1 == count),
        },
{
    compute_xgmi_packet_at_body_v1!(total, count, index, PACKET_BYTES)
}

fn compute_xgmi_packet_coverage_v1(total: u64, index: usize)
    -> (result: Option<(u64, u32)>)
    ensures
        match result {
            None => !admitted(total) || index as int >= exact_count(total),
            Some(packet) => admitted(total)
                && packet.0 as int == index as int * PACKET_BYTES as int
                && packet.0 as int + packet.1 as int <= total
                && 0 < packet.1 <= PACKET_BYTES
                && (index == 0 ==> packet.0 == 0)
                && (index as int + 1 < exact_count(total)
                    ==> packet.0 as int + packet.1 as int
                        == (index as int + 1) * PACKET_BYTES as int)
                && (index as int + 1 == exact_count(total)
                    ==> packet.0 as int + packet.1 as int == total),
        },
{
    match compute_xgmi_packet_count_v1(total) {
        None => None,
        Some(count) => compute_xgmi_packet_at_v1(total, count, index),
    }
}

}
