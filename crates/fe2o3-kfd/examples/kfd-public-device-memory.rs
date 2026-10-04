//! Isolated gfx942 validation for CPU initialization of public device memory.

use std::process::Command;

use fe2o3_kfd::{
    DeviceSelector, GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_SHA256_V1,
    Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1, OpenedKfd,
    SharedMemorySessionPhaseV1,
};

const CHILD_ENV: &str = "FE2O3_KFD_PUBLIC_DEVICE_MEMORY_LIVE_CHILD";

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
    let bytes = vec![0x5a; 4097].into_boxed_slice();
    let role = Gfx942DeviceContentRoleV1::new([0x51; 32], 0)?;
    let descriptor = Gfx942DeviceContentDescriptorV1::from_bytes(role, &bytes)?;
    let initialized = session.initialize_gfx942_device_memory(bytes, 4096, descriptor)?;
    if initialized.content() != descriptor
        || initialized.layout().requested_bytes() != descriptor.byte_len()
        || initialized.layout().uapi_flags() != 0xa000_0001
    {
        return Err("initialized public-device-memory observation mismatch".into());
    }
    session.release_initialized_gfx942_device_memory(initialized)?;
    if session.phase() != SharedMemorySessionPhaseV1::Active
        || session.retained_device_memory_lease_count() != 0
        || session.retained_device_memory_bytes() != 0
    {
        return Err("public-device-memory release mismatch".into());
    }
    println!(
        "profile_sha256={} unique_id={unique_id:016x} flags=a0000001 cpu_initialize=success gpu_map_unmap=success release=success",
        GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_SHA256_V1,
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let unique = std::env::args()
        .nth(1)
        .ok_or("usage: kfd-public-device-memory <selected-unique-id>")?;
    if std::env::var_os(CHILD_ENV).is_some() {
        return run_child(parse_u64(&unique)?);
    }
    let status = Command::new(std::env::current_exe()?)
        .arg(unique)
        .env(CHILD_ENV, "1")
        .status()?;
    if !status.success() {
        return Err(format!("isolated public device-memory child failed with {status}").into());
    }
    Ok(())
}
