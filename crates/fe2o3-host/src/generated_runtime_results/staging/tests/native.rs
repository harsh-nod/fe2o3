//! Synthetic charged host data over genuine native transport, not Worker execution.

use super::*;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;

#[derive(Debug)]
struct NoCompute;

// SAFETY: No artifact, invocation, or argument can authorize compute.
unsafe impl KfdRuntimeLaunchAuthorityV1 for NoCompute {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

fn settle<A>(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    submission: &mut RuntimeSubmissionV1<A>,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(Instant::now() < deadline, "native transport deadline");
        context.flush_stream(stream).unwrap();
        if context.poll(submission).unwrap() == RuntimePollV1::Succeeded {
            break;
        }
        std::thread::sleep(Duration::from_micros(50));
    }
}

#[test]
#[ignore = "requires two explicitly selected idle gfx942 GPUs and isolated process"]
fn charged_host_staging_upload_and_native_peer_copy() {
    assert_eq!(
        std::env::var("FE2O3_TEST_NATIVE_ISOLATED").as_deref(),
        Ok("1")
    );
    let ids: Vec<u64> = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_IDS")
        .unwrap()
        .split(',')
        .map(|text| u64::from_str_radix(text.strip_prefix("0x").unwrap(), 16).unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(ids[0] != 0 && ids[1] != 0 && ids[0] != ids[1]);
    let authorities = ids
        .into_iter()
        .map(|id| {
            (
                id,
                Box::new(NoCompute) as Box<dyn KfdRuntimeLaunchAuthorityV1>,
            )
        })
        .collect();
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_default_with_native_peer_copy_v1(authorities).unwrap();
    // Failure cannot run native custody destructors while effects may still be live.
    // The isolated process controller bounds failure; success explicitly closes all resources.
    let mut context =
        ManuallyDrop::new(Context::open_with_version_journal_v1(backend, 8, 8).unwrap());
    assert_eq!(context.devices().len(), 2);
    assert!(
        context
            .devices()
            .iter()
            .all(|device| device.target() == "gfx942:xnack-")
    );
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let streams: Vec<_> = devices
        .iter()
        .map(|device| context.create_stream(*device).unwrap())
        .collect();

    let values: Box<[u32]> = (0..4097u32)
        .map(|index| index.wrapping_mul(0x9e3779b9) ^ 0x12345678)
        .collect();
    let expected: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let mut output = output(values);
    output.custody.decode(&expected, &output.gate).unwrap();
    output.gate.commit();
    let result = output.observer.try_take().unwrap().unwrap();
    let pointer = result.as_slice().as_ptr();
    let usage = output.budget.usage();
    let bytes = expected.len() as u64;
    let host = context
        .allocate(devices[0], RuntimeMemoryKindV1::HostVisible, bytes, 4096)
        .unwrap();
    let source = context
        .allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, bytes, 4096)
        .unwrap();
    let guard_host = context
        .allocate(
            devices[1],
            RuntimeMemoryKindV1::HostVisible,
            bytes + 64,
            4096,
        )
        .unwrap();
    let destination = context
        .allocate(
            devices[1],
            RuntimeMemoryKindV1::DeviceLocal,
            bytes + 64,
            4096,
        )
        .unwrap();
    let mut scratch = vec![0; expected.len()];
    result
        .write_staging_v1(&mut context, host, &mut scratch)
        .unwrap();
    assert_eq!(scratch, expected);
    let mut guarded = vec![0xa5; expected.len() + 64];
    context
        .write_host_visible_allocation_v1(guard_host, &guarded)
        .unwrap();
    for (stream, staging, data, len) in [
        (streams[0], host, source, bytes),
        (streams[1], guard_host, destination, bytes + 64),
    ] {
        let mut upload = context
            .copy_async(
                stream,
                region(staging, RuntimeAccessV1::Read, 0, len),
                region(data, RuntimeAccessV1::Write, 0, len),
                &[],
            )
            .unwrap();
        settle(&mut context, stream, &mut upload);
        context.release_submission(upload).unwrap();
    }
    assert_eq!(context.backend().completed_compute_xgmi_copies_v1(), 0);
    let mut peer = context
        .peer_copy(
            streams[1],
            region(source, RuntimeAccessV1::Read, 0, bytes),
            region(destination, RuntimeAccessV1::Write, 32, bytes),
            &[],
        )
        .unwrap();
    settle(&mut context, streams[1], &mut peer);
    assert_eq!(context.backend().completed_compute_xgmi_copies_v1(), 1);
    context.release_submission(peer).unwrap();
    let mut observed = vec![0; expected.len()];
    context.read_allocation(source, 0, &mut observed).unwrap();
    assert_eq!(observed, expected);
    guarded[32..32 + expected.len()].copy_from_slice(&expected);
    observed.resize(guarded.len(), 0);
    context
        .read_allocation(destination, 0, &mut observed)
        .unwrap();
    assert_eq!(observed, guarded);
    assert_eq!(result.as_slice().as_ptr(), pointer);
    assert_eq!(output.budget.usage(), usage);
    result.encode_into_v1(&mut scratch).unwrap();
    assert_eq!(scratch, expected);
    for allocation in [destination, guard_host, source, host] {
        context.release_allocation(allocation).unwrap();
    }
    for stream in streams.into_iter().rev() {
        context.destroy_stream(stream).unwrap();
    }
    let mut backend = ManuallyDrop::new(ManuallyDrop::into_inner(context).shutdown().unwrap());
    backend.shutdown_native_v1().unwrap();
    drop(ManuallyDrop::into_inner(backend));
    drop(result);
    assert_eq!(output.budget.usage().reserved_peak_bytes, 0);
    eprintln!(
        "STAGED RESULT PEER PASS: synthetic charged host data, 2 uploads, 1 native peer, full source and guard readback, explicit shutdown, result credit refunded"
    );
}
