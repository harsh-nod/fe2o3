//! Two-device KFD queue and asynchronous SDMA validation in one process.

use std::time::Duration;

use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, ComputeAqlQueueSessionV1, DeviceSelector, OpenedKfd,
};

const COPY_BYTES: usize = 256 * 1024;

fn parse_unique_id(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        value.parse()?
    })
}

fn admit_device(
    unique_id: u64,
) -> Result<CheckedGfx942XnackMinusDevice, Box<dyn std::error::Error>> {
    Ok(OpenedKfd::open_default()?
        .admit_uapi()?
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))?)
}

fn create_queue(
    device: CheckedGfx942XnackMinusDevice,
) -> Result<ComputeAqlQueueSessionV1, Box<dyn std::error::Error>> {
    let mut queue = device.create_compute_aql_queue(4096)?;
    queue.enable_sdma_copy_engine()?;
    Ok(queue)
}

fn submit_copy(
    queue: &mut ComputeAqlQueueSessionV1,
    value: u8,
) -> Result<fe2o3_kfd::Gfx942SdmaCopyTicketV1, Box<dyn std::error::Error>> {
    let mut source = queue.allocate_sdma_pooled_host_buffer(COPY_BYTES)?;
    let destination = queue.allocate_sdma_pooled_host_buffer(COPY_BYTES)?;
    queue.write_sdma_host_buffer(&mut source, 0, &vec![value; COPY_BYTES])?;
    Ok(queue
        .submit_sdma_copy(source, 0, destination, 0, COPY_BYTES as u32)
        .map_err(|failure| failure.into_parts().0)?)
}

fn complete_copy(
    queue: &mut ComputeAqlQueueSessionV1,
    ticket: fe2o3_kfd::Gfx942SdmaCopyTicketV1,
    expected: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let completed = queue.wait_sdma_copy_for(ticket, Duration::from_secs(10))?;
    let (source, destination) = completed.into_buffers();
    let observed = queue.read_sdma_host_buffer(&destination, 0, COPY_BYTES as u64)?;
    assert!(observed.iter().all(|byte| *byte == expected));
    queue.recycle_sdma_buffer(source)?;
    queue.recycle_sdma_buffer(destination)?;
    queue.trim_sdma_memory_pool()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ids = std::env::args()
        .skip(1)
        .map(|value| parse_unique_id(&value))
        .collect::<Result<Vec<_>, _>>()?;
    if ids.len() != 2 || ids[0] == ids[1] {
        return Err("usage: kfd-sdma-multi-device <unique-id-0> <unique-id-1>".into());
    }

    // SET_XNACK_MODE is process-wide and requires the no-queue admission
    // barrier, so all selected devices are admitted before the first VM/queue.
    let left_device = admit_device(ids[0])?;
    let right_device = admit_device(ids[1])?;
    let mut left = create_queue(left_device)?;
    let mut right = create_queue(right_device)?;
    let foreign = left.allocate_sdma_pooled_host_buffer(4096)?;
    let (error, recovered) = right
        .recycle_sdma_buffer(foreign)
        .expect_err("a second device must reject foreign pooled storage")
        .into_parts();
    assert!(error.to_string().contains("foreign SDMA buffer owner"));
    left.recycle_sdma_buffer(recovered.expect("foreign rejection returns custody"))?;
    assert_eq!(left.trim_sdma_memory_pool()?, 1);
    let left_ticket = submit_copy(&mut left, 0x35)?;
    let right_ticket = submit_copy(&mut right, 0xca)?;
    complete_copy(&mut right, right_ticket, 0xca)?;
    complete_copy(&mut left, left_ticket, 0x35)?;
    let right_destroyed = right.destroy()?;
    let left_destroyed = left.destroy()?;
    assert_eq!(right_destroyed.released_resources(), 8);
    assert_eq!(left_destroyed.released_resources(), 8);
    println!(
        "schema=fe2o3.kfd-multi-device-hardware.v1 devices=2 unique_ids={:016x},{:016x} concurrent_sdma=2 copy_bytes={COPY_BYTES} foreign_buffer_rejected=1 teardown=reverse status=pass",
        ids[0], ids[1]
    );
    Ok(())
}
