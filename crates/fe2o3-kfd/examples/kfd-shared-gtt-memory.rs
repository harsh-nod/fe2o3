//! Isolated MI300X validation for the shared GTT memory authority.

use std::process::Command;

use fe2o3_kfd::{
    DeviceSelector, GFX942_CONTEXT_SAVE_MAPPING_BYTES_V1, GFX942_EOP_BYTES_V1, OpenedKfd,
    SHARED_GTT_MEMORY_PROFILE_SHA256_V1, SharedMemorySessionPhaseV1,
};

const CHILD_ENV: &str = "FE2O3_KFD_SHARED_GTT_LIVE_CHILD";

fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if let Some(hex) = value.strip_prefix("0x") {
        Ok(u64::from_str_radix(hex, 16)?)
    } else {
        Ok(value.parse()?)
    }
}

fn run_child(unique_id: u64) -> Result<(), Box<dyn std::error::Error>> {
    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut session = device.acquire_shared_gtt_memory_session()?;
    let mut control = session.allocate_host_visible_coherent(8192)?;
    let mut kernarg = session.allocate_kernarg(4096)?;
    let mut ring = session.allocate_aql_queue(4096)?;
    let mut executable = session.allocate_executable(4096)?;
    let mut eop = session.allocate_executable(usize::try_from(GFX942_EOP_BYTES_V1)?)?;
    let mut context_save =
        session.allocate_executable(usize::try_from(GFX942_CONTEXT_SAVE_MAPPING_BYTES_V1)?)?;
    session.with_bytes_mut(&mut control, |bytes| bytes.fill(0x11))?;
    session.with_bytes_mut(&mut kernarg, |bytes| bytes.fill(0x22))?;
    session.with_bytes_mut(&mut ring, |bytes| bytes.fill(0x33))?;
    session.with_bytes_mut(&mut executable, |bytes| bytes.fill(0x44))?;
    session.with_bytes_mut(&mut eop, |bytes| {
        bytes[0] = 0x55;
        bytes[bytes.len() - 1] = 0x55;
    })?;
    session.with_bytes_mut(&mut context_save, |bytes| {
        bytes[0] = 0x66;
        bytes[bytes.len() - 1] = 0x66;
    })?;
    let executable = session.seal_executable(executable)?;
    let eop = session.seal_executable(eop)?;
    let context_save = session.seal_executable(context_save)?;

    let control = session.map_to_gpu(control)?;
    let control = session.unmap_from_gpu(control)?;
    let kernarg = session.map_to_gpu(kernarg)?;
    let kernarg = session.unmap_from_gpu(kernarg)?;
    let ring = session.map_to_gpu(ring)?;
    let ring = session.unmap_from_gpu(ring)?;
    let executable = session.map_executable_to_gpu(executable)?;
    let executable = session.unmap_executable_from_gpu(executable)?;
    let eop = session.map_executable_to_gpu(eop)?;
    let eop = session.unmap_executable_from_gpu(eop)?;
    let context_save = session.map_executable_to_gpu(context_save)?;
    let context_save = session.unmap_executable_from_gpu(context_save)?;

    session.with_bytes(&control, |bytes| assert_eq!(bytes[0], 0x11))?;
    session.with_bytes(&kernarg, |bytes| assert_eq!(bytes[0], 0x22))?;
    session.with_bytes(&ring, |bytes| assert_eq!(bytes[0], 0x33))?;
    session.with_bytes(&executable, |bytes| assert_eq!(bytes[0], 0x44))?;
    session.with_bytes(&eop, |bytes| {
        assert_eq!((bytes[0], bytes[bytes.len() - 1]), (0x55, 0x55));
    })?;
    session.with_bytes(&context_save, |bytes| {
        assert_eq!((bytes[0], bytes[bytes.len() - 1]), (0x66, 0x66));
    })?;
    session.release(control)?;
    session.release(kernarg)?;
    session.release(ring)?;
    session.release_executable(executable)?;
    session.release_executable(eop)?;
    session.release_executable(context_save)?;
    assert_eq!(session.phase(), SharedMemorySessionPhaseV1::Active);
    println!(
        "profile_sha256={} unique_id={unique_id:016x} shared_vm=success allocations=6 distinct_guarded_va=6 aql_double_map=success control_gtt=8192 executable_seal=success eop_gtt={} cwsr_gtt={} map_unmap=6 release=6",
        SHARED_GTT_MEMORY_PROFILE_SHA256_V1,
        GFX942_EOP_BYTES_V1,
        GFX942_CONTEXT_SAVE_MAPPING_BYTES_V1,
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let unique = std::env::args()
        .nth(1)
        .ok_or("usage: kfd-shared-gtt-memory <selected-unique-id>")?;
    if std::env::var_os(CHILD_ENV).is_some() {
        return run_child(parse_u64(&unique)?);
    }
    let status = Command::new(std::env::current_exe()?)
        .arg(unique)
        .env(CHILD_ENV, "1")
        .status()?;
    if !status.success() {
        return Err(format!("isolated shared GTT child failed with {status}").into());
    }
    Ok(())
}
