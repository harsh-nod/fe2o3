//! Replicated exact vecadd on multiple GPUs, followed by host-staged peer copies.

use std::time::{Duration, Instant};

use fe2o3_runtime::qualification_gfx942_vecadd_v1::{
    GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1, GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1,
    GFX942_VECADD_QUALIFICATION_GEOMETRY_V1, GFX942_VECADD_QUALIFICATION_KERNEL_V1,
    Gfx942VecaddQualificationArgumentsV1, admit_gfx942_vecadd_qualification_v1,
};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, RuntimeAccessV1, RuntimeAllocationIdV1,
    RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncDrainOutcomeV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1, RuntimeContextV1,
    RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimePollV1, RuntimeStreamIdV1,
    RuntimeStreamObservationV1, TypedRuntimeKernelV1,
};

type ContextV1 = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const WAIT: Duration = Duration::from_secs(10);
const DRAIN_TICKS: usize = 256;

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let error = format!("stage={stage} {error:?}");
    eprintln!("multi-device qualification failed before complete cleanup: {error}");
    error
}

fn unique_ids(arguments: &[String]) -> ResultV1<Vec<u64>> {
    if !(2..=8).contains(&arguments.len()) {
        return Err("usage: gfx942-runtime-multi-device-smoke <0xunique-id> <0xunique-id> [up to eight total]".into());
    }
    arguments
        .iter()
        .map(|text| {
            let hex = text
                .strip_prefix("0x")
                .ok_or("hexadecimal unique ID required")?;
            let value = u64::from_str_radix(hex, 16).map_err(|_| "invalid unique ID")?;
            if value == 0 {
                return Err("nonzero unique ID required".into());
            }
            Ok(value)
        })
        .collect()
}

struct DeviceRun {
    stream: RuntimeStreamIdV1,
    kernel: TypedRuntimeKernelV1<Gfx942VecaddQualificationArgumentsV1>,
    allocations: [RuntimeAllocationIdV1; 3],
    arguments: Gfx942VecaddQualificationArgumentsV1,
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
    }
}

fn verify(
    context: &mut ContextV1,
    allocation: RuntimeAllocationIdV1,
    expected: &[u8],
) -> ResultV1<()> {
    let mut observed = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut observed)
        .map_err(|error| failure("readback", error))?;
    if observed != expected {
        return Err(failure("readback", "full-byte output verification failed"));
    }
    Ok(())
}

fn run(unique_ids: &[u64]) -> ResultV1<()> {
    let admitted = admit_gfx942_vecadd_qualification_v1()
        .map_err(|error| failure("fixture-admission", error))?;
    let (left, right, initial, expected) = admitted
        .host_buffers()
        .map_err(|error| failure("fixture-buffers", error))?
        .into_parts();
    let backend = KfdMultiDeviceRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(unique_ids)
        .map_err(|error| failure("device-admission", error))?;
    let mut context =
        RuntimeContextV1::open(backend).map_err(|error| failure("context-open", error))?;
    if context.devices().len() != unique_ids.len()
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure(
            "device-roster",
            "exact selected gfx942:xnack- device roster required",
        ));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut runs = Vec::with_capacity(devices.len());
    for &device in &devices {
        if context
            .execution_capabilities(device)
            .map_err(|error| failure("capabilities", error))?
            .native_peer_copy
        {
            return Err(failure(
                "capabilities",
                "this witness requires the explicitly host-staged multi-device backend",
            ));
        }
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("compute-stream", error))?;
        let module = context
            .load_module(device, admitted.hsaco())
            .map_err(|error| failure("module-load", error))?;
        let kernel = context
            .resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(
                module,
                GFX942_VECADD_QUALIFICATION_KERNEL_V1,
            )
            .map_err(|error| failure("kernel-resolve", error))?;
        let mut allocations = Vec::with_capacity(3);
        for bytes in [&left, &right, &initial] {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                    GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                )
                .map_err(|error| failure("compute-allocation", error))?;
            context
                .write_allocation(allocation, 0, bytes)
                .map_err(|error| failure("compute-initialize", error))?;
            allocations.push(allocation);
        }
        let allocations: [RuntimeAllocationIdV1; 3] = allocations.try_into().map_err(|_| {
            failure(
                "compute-allocation",
                "exact three-allocation fixture required",
            )
        })?;
        let arguments = Gfx942VecaddQualificationArgumentsV1::new(
            allocations[0],
            allocations[1],
            allocations[2],
        )
        .map_err(|error| failure("arguments", error))?;
        runs.push(DeviceRun {
            stream,
            kernel,
            allocations,
            arguments,
        });
    }

    // Publish every producer before observing any completion; physical overlap is not inferred.
    let mut producers = Vec::with_capacity(runs.len());
    for run in &runs {
        producers.push(
            context
                .launch(
                    run.stream,
                    &run.kernel,
                    &run.arguments,
                    GFX942_VECADD_QUALIFICATION_GEOMETRY_V1,
                    &[],
                )
                .map_err(|error| failure("producer-launch", error))?,
        );
        context
            .flush_stream(run.stream)
            .map_err(|error| failure("producer-flush", error))?;
    }
    for (run, producer) in runs.iter().zip(&mut producers) {
        if context
            .wait(producer, WAIT)
            .map_err(|error| failure("producer-wait", error))?
            != RuntimePollV1::Succeeded
        {
            return Err(failure(
                "producer-wait",
                "native producer did not complete successfully",
            ));
        }
        verify(&mut context, run.allocations[2], &expected)?;
    }
    // Ordinary cooperative copies require all native producer custody to be settled.
    for producer in producers {
        context
            .release_submission(producer)
            .map_err(|error| failure("producer-release", error))?;
    }

    let mut copy_streams = Vec::with_capacity(devices.len() - 1);
    let mut copied_outputs = Vec::with_capacity(devices.len() - 1);
    for &device in &devices[1..] {
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("copy-stream", error))?;
        let output = context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
            )
            .map_err(|error| failure("copy-allocation", error))?;
        context
            .write_allocation(output, 0, &initial)
            .map_err(|error| failure("copy-initialize", error))?;
        let mut copy = context
            .peer_copy(
                stream,
                region(runs[0].allocations[2], RuntimeAccessV1::Read),
                region(output, RuntimeAccessV1::Write),
                &[],
            )
            .map_err(|error| failure("peer-copy-enqueue", error))?;
        let deadline = Instant::now() + WAIT;
        loop {
            context
                .flush_stream(stream)
                .map_err(|error| failure("peer-copy-flush", error))?;
            match context
                .poll(&mut copy)
                .map_err(|error| failure("peer-copy-poll", error))?
            {
                RuntimePollV1::Succeeded => break,
                RuntimePollV1::Pending if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_micros(50));
                }
                status => {
                    return Err(failure(
                        "peer-copy-poll",
                        format!("host-staged peer transfer did not succeed: {status:?}"),
                    ));
                }
            }
        }
        verify(&mut context, output, &expected)?;
        context
            .release_submission(copy)
            .map_err(|error| failure("peer-copy-release", error))?;
        copy_streams.push(stream);
        copied_outputs.push(output);
    }

    // Keep the logical resources owned for the existing group-drain/shutdown path.
    for stream in runs
        .iter()
        .map(|run| run.stream)
        .chain(copy_streams.iter().copied())
    {
        if context
            .query_stream(stream)
            .map_err(|error| failure("submission-roster", error))?
            .total_submissions
            != 0
        {
            return Err(failure(
                "submission-roster",
                "completed work still retains a submission record",
            ));
        }
    }
    let retained_allocations =
        runs.iter().flat_map(|run| run.allocations).count() + copied_outputs.len();
    let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || Ok::<_, String>(context),
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .map_err(|error| failure("owner-open", error))?;
    let mut draining = Box::pin(
        handle
            .begin_drain(DRAIN_TICKS)
            .map_err(|error| failure("drain-admission", error))?,
    );
    let drain_result = engine.drive_until_ready(draining.as_mut(), Instant::now() + WAIT);
    // Shutdown explicitly invokes logical cleanup and the multi-device native shutdown hook.
    let shutdown = engine.shutdown();
    let drain = drain_result
        .map_err(|error| failure("drain-drive", error))?
        .map_err(|error| failure("drain-result", error))?;
    if drain.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
        || drain.ticks == 0
        || drain.ticks > DRAIN_TICKS
        || drain.retained_submissions != RuntimeStreamObservationV1::default()
        || !drain.queued_commands_exhausted
        || drain.operations_remaining != 0
        || drain.graph_active
    {
        return Err(failure(
            "drain-result",
            format!("completed group did not drain cleanly: {drain:?}"),
        ));
    }
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|report| !report.is_complete() || !report.failures().is_empty())
    {
        return Err(failure(
            "native-shutdown",
            format!("explicit native shutdown did not release custody: {shutdown:?}"),
        ));
    }
    println!(
        "PASS schema=fe2o3.multi-device-vecadd-smoke.v1 devices={} compute={} peer_copies={} bytes={} transport=HOST-STAGED fixture=replicated-exact-vecadd publication=all-before-wait physical_overlap=unmeasured sharding=false native_xgmi=false modules={} allocations={} output=full-byte-pass drain=completed-group-quiescent cleanup=logical-and-native-explicit",
        devices.len(),
        runs.len(),
        copied_outputs.len(),
        expected.len(),
        runs.len(),
        retained_allocations
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ids = unique_ids(&std::env::args().skip(1).collect::<Vec<_>>())?;
    run(&ids).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_requires_two_to_eight_nonzero_hexadecimal_ids() {
        assert_eq!(unique_ids(&["0x1".into(), "0xff".into()]), Ok(vec![1, 255]));
        for arguments in [
            vec![],
            vec!["0x1".into()],
            vec!["0x1".into(); 9],
            vec!["1".into(), "0x2".into()],
            vec!["0x0".into(), "0x2".into()],
            vec!["0xg".into(), "0x2".into()],
        ] {
            assert!(unique_ids(&arguments).is_err());
        }
    }
}
