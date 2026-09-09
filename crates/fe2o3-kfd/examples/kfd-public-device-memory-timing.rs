//! Isolated timing harness for arbitrary owned-image initialization of public
//! gfx942 device memory.

use std::process::Command;
use std::time::Instant;

use fe2o3_kfd::{
    DeviceSelector, GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_SHA256_V1,
    Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1, OpenedKfd,
    SharedMemorySessionPhaseV1,
};

const CHILD_ENV: &str = "FE2O3_KFD_PUBLIC_DEVICE_MEMORY_TIMING_CHILD";
const DEFAULT_BYTE_LEN: u64 = 16_384_000_000;

fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if let Some(hex) = value.strip_prefix("0x") {
        Ok(u64::from_str_radix(hex, 16)?)
    } else {
        Ok(value.parse()?)
    }
}

fn elapsed_millis(start: Instant) -> u128 {
    start.elapsed().as_millis()
}

fn run_child(unique_id: u64, byte_len: u64) -> Result<(), Box<dyn std::error::Error>> {
    let byte_len_usize = usize::try_from(byte_len)?;
    if byte_len_usize == 0 {
        return Err("byte length must be nonzero".into());
    }

    let device = OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?;
    let mut session = device.acquire_shared_gtt_memory_session()?;

    let source_start = Instant::now();
    let bytes = vec![0x5a; byte_len_usize].into_boxed_slice();
    let source_allocation_ms = elapsed_millis(source_start);

    let descriptor_start = Instant::now();
    let role = Gfx942DeviceContentRoleV1::new([0x71; 32], 0)?;
    let descriptor = Gfx942DeviceContentDescriptorV1::from_bytes(role, &bytes)?;
    let descriptor_sha256_ms = elapsed_millis(descriptor_start);

    let initialization_start = Instant::now();
    let initialized = session.initialize_gfx942_device_memory(bytes, 4096, descriptor)?;
    let initialization_ms = elapsed_millis(initialization_start);
    if initialized.content() != descriptor
        || initialized.layout().requested_bytes() != byte_len
        || initialized.layout().uapi_flags() != 0xa000_0001
    {
        return Err("initialized public-device-memory observation mismatch".into());
    }

    let release_start = Instant::now();
    session.release_initialized_gfx942_device_memory(initialized)?;
    let release_ms = elapsed_millis(release_start);
    if session.phase() != SharedMemorySessionPhaseV1::Active
        || session.retained_device_memory_lease_count() != 0
        || session.retained_device_memory_bytes() != 0
    {
        return Err("public-device-memory release mismatch".into());
    }

    println!(
        "profile_sha256={} unique_id={unique_id:016x} bytes={byte_len} source_allocation_ms={source_allocation_ms} descriptor_sha256_ms={descriptor_sha256_ms} initialization_ms={initialization_ms} release_ms={release_ms} flags=a0000001 gpu_map_unmap=success release=success",
        GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_SHA256_V1,
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let unique = arguments
        .next()
        .ok_or("usage: kfd-public-device-memory-timing <selected-unique-id> [byte-len]")?;
    let byte_len = arguments
        .next()
        .map(|value| parse_u64(&value))
        .transpose()?
        .unwrap_or(DEFAULT_BYTE_LEN);
    if arguments.next().is_some() {
        return Err(
            "usage: kfd-public-device-memory-timing <selected-unique-id> [byte-len]".into(),
        );
    }
    if std::env::var_os(CHILD_ENV).is_some() {
        return run_child(parse_u64(&unique)?, byte_len);
    }
    let status = Command::new(std::env::current_exe()?)
        .arg(unique)
        .arg(byte_len.to_string())
        .env(CHILD_ENV, "1")
        .status()?;
    if !status.success() {
        return Err(
            format!("isolated public device-memory timing child failed with {status}").into(),
        );
    }
    Ok(())
}
