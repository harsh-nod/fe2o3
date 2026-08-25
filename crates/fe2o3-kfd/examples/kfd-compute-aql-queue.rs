//! Isolated MI300X CREATE/doorbell-map/DESTROY validation without MMIO stores.

use std::process::Command;

use fe2o3_kfd::{DeviceSelector, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1, OpenedKfd};

const CHILD_ENV: &str = "FE2O3_KFD_COMPUTE_AQL_QUEUE_CHILD";

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
    let mut queue = device.create_compute_aql_queue(4096)?;
    let observation = queue.observation();
    assert_eq!(
        observation.queue_id(),
        0,
        "isolated process must receive queue ID zero"
    );
    assert_eq!(observation.ring_bytes(), 4096);
    assert_eq!(observation.doorbell_slice_bytes(), 8192);
    assert!(observation.doorbell_byte_offset() < 8192);
    assert_eq!(observation.doorbell_byte_offset() % 8, 0);
    assert!((1..=255).contains(&observation.event_id()));
    assert_eq!(observation.cwsr_shadow_pages(), 8);
    queue.verify_doorbell_dontfork()?;
    queue.verify_exception_shadows_dontfork()?;
    let destroyed = queue.destroy()?;
    assert_eq!(destroyed.queue_id(), 0);
    assert_eq!(destroyed.released_resources(), 5);
    println!(
        "profile_sha256={} unique_id={unique_id:016x} queue_id={} event_id={} cwsr_shadow_pages={} runtime=enabled-before-create-then-disabled ring=4096 roles=ring,control,eop,cwsr,completion-signals gtt_policy=accepted doorbell_slice={} doorbell_byte_offset={} dontfork=confirmed mmio_stores=0 packets=0 destroy=queue-then-event-then-runtime-confirmed resources_returned={}",
        GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1,
        observation.queue_id(),
        observation.event_id(),
        observation.cwsr_shadow_pages(),
        observation.doorbell_slice_bytes(),
        observation.doorbell_byte_offset(),
        destroyed.released_resources(),
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let unique = std::env::args()
        .nth(1)
        .ok_or("usage: kfd-compute-aql-queue <selected-unique-id>")?;
    if std::env::var_os(CHILD_ENV).is_some() {
        return run_child(parse_u64(&unique)?);
    }
    let status = Command::new(std::env::current_exe()?)
        .arg(unique)
        .env(CHILD_ENV, "1")
        .status()?;
    if !status.success() {
        return Err(format!("isolated compute-AQL queue child failed with {status}").into());
    }
    Ok(())
}
