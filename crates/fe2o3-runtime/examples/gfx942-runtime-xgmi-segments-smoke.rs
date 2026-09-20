//! Opt-in native correctness test. Admit an idle, identity-checked pair before
//! invoking this process; it does not reserve GPUs or measure HIP/HSA parity.

use fe2o3_runtime::{
    KfdNativeXgmiRuntimeBackendV1, RuntimeAccessV1, RuntimeCancellationV1, RuntimeContextV1,
    RuntimeDeviceIdV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimePeerCopySegmentV1,
    RuntimePollV1,
};
use std::{
    error::Error,
    fmt::Debug,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Context = RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>;
const BYTES: usize = 65_536;

fn error(error: impl Debug) -> Box<dyn Error> {
    format!("{error:?}").into()
}

fn unique_id(text: &str) -> Result<u64> {
    let id = match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16)?,
        None => text.parse()?,
    };
    if id == 0 {
        return Err("zero unique ID".into());
    }
    Ok(id)
}

fn descriptors(count: usize) -> Vec<RuntimePeerCopySegmentV1> {
    (0..count as u64)
        .map(|i| RuntimePeerCopySegmentV1 {
            source_offset: (i * 13) % 32_740,
            destination_offset: (i * 17) % 49_120,
            byte_len: i % 19 + 1,
        })
        .collect()
}

fn run(
    context: &mut Context,
    devices: [RuntimeDeviceIdV1; 2],
    count: usize,
    poll_flush: bool,
) -> Result<()> {
    let source = context
        .allocate(
            devices[0],
            RuntimeMemoryKindV1::DeviceLocal,
            BYTES as u64,
            4096,
        )
        .map_err(error)?;
    let destination = context
        .allocate(
            devices[1],
            RuntimeMemoryKindV1::DeviceLocal,
            BYTES as u64,
            4096,
        )
        .map_err(error)?;
    let stream = context.create_stream(devices[1]).map_err(error)?;
    let input: Vec<_> = (0..BYTES).map(|i| (i % 251) as u8).collect();
    let mut expected = vec![0xa5; BYTES];
    context.write_allocation(source, 0, &input).map_err(error)?;
    context
        .write_allocation(destination, 0, &expected)
        .map_err(error)?;
    let source_region = RuntimeMemoryRegionV1 {
        allocation: source,
        access: RuntimeAccessV1::Read,
        byte_offset: 32,
        byte_len: 32_768,
    };
    let destination_region = RuntimeMemoryRegionV1 {
        allocation: destination,
        access: RuntimeAccessV1::Write,
        byte_offset: 64,
        byte_len: 49_152,
    };
    let descriptors = descriptors(count);
    for segment in &descriptors {
        let from = 32 + segment.source_offset as usize;
        let to = 64 + segment.destination_offset as usize;
        let bytes = segment.byte_len as usize;
        expected[to..to + bytes].copy_from_slice(&input[from..from + bytes]);
    }

    let mut cancelled = context
        .peer_copy_segments(stream, source_region, destination_region, &descriptors, &[])
        .map_err(error)?;
    if context.cancel(&mut cancelled).map_err(error)? != RuntimeCancellationV1::Cancelled {
        return Err("prepublication cancellation failed".into());
    }
    context.release_submission(cancelled).map_err(error)?;
    let mut submission = context
        .peer_copy_segments(stream, source_region, destination_region, &descriptors, &[])
        .map_err(error)?;
    let event = context.record_event(&submission).map_err(error)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    if poll_flush {
        if context.poll(&mut submission).map_err(error)? != RuntimePollV1::Pending {
            return Err("multi-segment poll completed more than its budget".into());
        }
        if context.cancel(&mut submission).map_err(error)? != RuntimeCancellationV1::TooLate {
            return Err("published sequence was cancellable".into());
        }
        loop {
            context.flush_stream(stream).map_err(error)?;
            match context.poll(&mut submission).map_err(error)? {
                RuntimePollV1::Succeeded => break,
                RuntimePollV1::Pending if Instant::now() < deadline => {}
                other => return Err(format!("sequence poll/flush: {other:?}").into()),
            }
        }
    } else if context
        .wait(&mut submission, Duration::from_secs(60))
        .map_err(error)?
        != RuntimePollV1::Succeeded
    {
        return Err("sequence wait did not succeed".into());
    }
    context.release_event(event).map_err(error)?;
    context.release_submission(submission).map_err(error)?;
    let mut observed = vec![0; BYTES];
    context
        .read_allocation(source, 0, &mut observed)
        .map_err(error)?;
    if observed != input {
        return Err("source changed".into());
    }
    context
        .read_allocation(destination, 0, &mut observed)
        .map_err(error)?;
    if observed != expected {
        return Err("destination data or canary mismatch".into());
    }
    context.release_allocation(destination).map_err(error)?;
    context.release_allocation(source).map_err(error)?;
    context.destroy_stream(stream).map_err(error)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: gfx942-runtime-xgmi-segments-smoke <unique-id-0> <unique-id-1>".into());
    }
    let ids = [unique_id(&args[0])?, unique_id(&args[1])?];
    if ids[0] == ids[1] {
        return Err("distinct GPUs required".into());
    }
    let backend = KfdNativeXgmiRuntimeBackendV1::open_default(ids[0], ids[1])?;
    let mut context =
        RuntimeContextV1::open_with_version_journal_v1(backend, 16, 8).map_err(error)?;
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for direction in 0..2 {
        let pair = [devices[direction], devices[1 - direction]];
        for (count, poll_flush) in [(1, false), (65, false), (4096, false), (4, true)] {
            run(&mut context, pair, count, poll_flush)?;
            println!(
                "direction={direction} segments={count} poll_flush={poll_flush} correctness=passed"
            );
        }
    }
    let mut backend = context.shutdown().map_err(error)?;
    backend.shutdown_native_v1().map_err(error)?;
    println!("native_shutdown=passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_smoke_descriptors_fit_the_unequal_envelopes() {
        for count in [1, 4, 65, 4096] {
            assert!(
                fe2o3_runtime_model::validate_ordered_peer_copy_segments_v1(
                    32,
                    32_768,
                    64,
                    49_152,
                    &descriptors(count)
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn unique_id_controls_reject_invalid_values_before_native_open() {
        assert_eq!(unique_id("0xab").unwrap(), 171);
        assert_eq!(unique_id("171").unwrap(), 171);
        assert!(unique_id("0").is_err());
        assert!(unique_id("0x").is_err());
    }
}
