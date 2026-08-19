//! Default-feature linked fixture for the production shared GTT authority.

use fe2o3_kfd::{
    DeviceSelector, OpenedKfd, SHARED_GTT_MEMORY_PROFILE_SHA256_V1, SharedMemorySessionPhaseV1,
};

fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if let Some(hex) = value.strip_prefix("0x") {
        Ok(u64::from_str_radix(hex, 16)?)
    } else {
        Ok(value.parse()?)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let unique = std::env::args()
        .nth(1)
        .ok_or("usage: kfd-shared-gtt-memory-policy <selected-unique-id>")?;
    let unique_id = parse_u64(&unique)?;
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut session = device.acquire_shared_gtt_memory_session()?;

    let mut ordinary = session.allocate_host_visible_coherent(4097)?;
    let mut kernarg = session.allocate_kernarg(256)?;
    let mut ring = session.allocate_aql_queue(4096)?;
    let mut executable = session.allocate_executable(4096)?;
    session.with_bytes_mut(&mut ordinary, |bytes| bytes.fill(0x11))?;
    session.with_bytes_mut(&mut kernarg, |bytes| bytes.fill(0x22))?;
    session.with_bytes_mut(&mut ring, |bytes| bytes.fill(0x33))?;
    session.with_bytes_mut(&mut executable, |bytes| bytes.fill(0x44))?;
    let executable = session.seal_executable(executable)?;
    session.with_bytes(&executable, |bytes| assert_eq!(bytes[0], 0x44))?;

    let ordinary = session.map_to_gpu(ordinary)?;
    let ordinary = session.unmap_from_gpu(ordinary)?;
    let kernarg = session.map_to_gpu(kernarg)?;
    let kernarg = session.unmap_from_gpu(kernarg)?;
    let ring = session.map_to_gpu(ring)?;
    let ring = session.unmap_from_gpu(ring)?;
    let executable = session.map_executable_to_gpu(executable)?;
    let executable = session.unmap_executable_from_gpu(executable)?;

    session.release(ordinary)?;
    session.release(kernarg)?;
    session.release(ring)?;
    session.release_executable(executable)?;
    assert_eq!(session.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(session.retained_allocation_count(), 0);
    let journal = session.model_journal_summary();
    println!(
        "profile_sha256={} unique_id={unique_id:016x} profiles=ordinary,kernarg,aql,executable allocations=4 map_unmap=4 seal=success release=4 model_vms={} model_reservations={} model_allocations={} model_mappings={}",
        SHARED_GTT_MEMORY_PROFILE_SHA256_V1,
        journal.vm_records(),
        journal.reservation_records(),
        journal.allocation_records(),
        journal.mapping_records(),
    );
    Ok(())
}
