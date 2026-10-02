//! Exact two-GPU R57 compute, native XGMI copy, and local consumer qualification.

use std::time::{Duration, Instant};

use fe2o3_runtime::qualification_gfx942_r57_n3_v1::{
    GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1, GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1,
    GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1, GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
    GFX942_R57_N3_QUALIFICATION_PROFILE_ID_V2, Gfx942R57N3QualificationArgumentsV2,
    admit_gfx942_r57_n3_qualification_v2,
};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeContextV1,
    RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimeModuleIdV1, RuntimePollV1,
    RuntimeStreamIdV1, TypedRuntimeKernelV1,
};

type ContextV1 = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const WAIT: Duration = Duration::from_secs(30);
const USAGE: &str =
    "usage: gfx942-runtime-compute-xgmi-smoke <0xsource-unique-id> <0xdestination-unique-id>";

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let error = format!("stage={stage} {error:?}");
    eprintln!("compute-XGMI qualification failed before complete cleanup: {error}");
    error
}

fn unique_ids(arguments: &[String]) -> ResultV1<[u64; 2]> {
    if arguments.len() != 2 {
        return Err(USAGE.into());
    }
    let mut ids = [0; 2];
    for (id, text) in ids.iter_mut().zip(arguments) {
        let hex = text
            .strip_prefix("0x")
            .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or("hexadecimal unique ID required")?;
        *id = u64::from_str_radix(hex, 16).map_err(|_| "invalid unique ID")?;
        if *id == 0 {
            return Err("nonzero unique ID required".into());
        }
    }
    if ids[0] == ids[1] {
        return Err("distinct source and destination unique IDs required".into());
    }
    Ok(ids)
}

fn full_region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
    }
}

fn sentinel() -> Vec<u8> {
    (-1.0_f32)
        .to_bits()
        .to_le_bytes()
        .repeat(GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 / size_of::<f32>())
}

fn exact_bytes(label: &str, observed: &[u8], expected: &[u8]) -> ResultV1<()> {
    if observed == expected {
        return Ok(());
    }
    let mismatch = observed
        .iter()
        .zip(expected)
        .position(|(observed, expected)| observed != expected)
        .unwrap_or(observed.len().min(expected.len()));
    Err(failure(
        label,
        format!(
            "full-byte mismatch at byte {mismatch}: observed_len={} expected_len={}",
            observed.len(),
            expected.len()
        ),
    ))
}

fn verify(
    context: &mut ContextV1,
    allocation: RuntimeAllocationIdV1,
    label: &str,
    expected: &[u8],
) -> ResultV1<()> {
    let mut observed = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut observed)
        .map_err(|error| failure(label, error))?;
    exact_bytes(label, &observed, expected)
}

fn upload_full_h2d(
    context: &mut ContextV1,
    stream: RuntimeStreamIdV1,
    upload: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    bytes: &[u8],
) -> ResultV1<()> {
    context
        .write_allocation(upload, 0, bytes)
        .map_err(|error| failure("upload-host-write", error))?;
    let mut submission = context
        .copy_async(
            stream,
            full_region(upload, RuntimeAccessV1::Read),
            full_region(destination, RuntimeAccessV1::Write),
            &[],
        )
        .map_err(|error| failure("upload-enqueue", error))?;
    context
        .flush_stream(stream)
        .map_err(|error| failure("upload-flush", error))?;
    if context
        .wait(&mut submission, WAIT)
        .map_err(|error| failure("upload-wait", error))?
        != RuntimePollV1::Succeeded
    {
        return Err(failure("upload-wait", "full H2D upload did not succeed"));
    }
    context
        .release_submission(submission)
        .map_err(|error| failure("upload-release", error))
}

struct DeviceRun {
    stream: RuntimeStreamIdV1,
    module: RuntimeModuleIdV1,
    kernel: TypedRuntimeKernelV1<Gfx942R57N3QualificationArgumentsV2>,
    upload: RuntimeAllocationIdV1,
    allocations: [RuntimeAllocationIdV1; 4],
}

fn compute_round(context: &mut ContextV1, runs: &[DeviceRun], consumer: bool) -> ResultV1<()> {
    let mut submissions = Vec::with_capacity(runs.len());
    for run in runs {
        let [a, b, c, d] = run.allocations;
        let arguments = Gfx942R57N3QualificationArgumentsV2::new(
            if consumer { c } else { a },
            b,
            if consumer { d } else { c },
        )
        .map_err(|error| failure("compute-arguments", error))?;
        submissions.push(
            context
                .launch(
                    run.stream,
                    &run.kernel,
                    &arguments,
                    GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
                    &[],
                )
                .map_err(|error| failure("compute-launch", error))?,
        );
        context
            .flush_stream(run.stream)
            .map_err(|error| failure("compute-flush", error))?;
    }
    for mut submission in submissions {
        if context
            .wait(&mut submission, WAIT)
            .map_err(|error| failure("compute-wait", error))?
            != RuntimePollV1::Succeeded
        {
            return Err(failure("compute-wait", "exact R57 launch did not succeed"));
        }
        context
            .release_submission(submission)
            .map_err(|error| failure("compute-release", error))?;
    }
    Ok(())
}

fn run(unique_ids: &[u64; 2]) -> ResultV1<()> {
    let admitted = admit_gfx942_r57_n3_qualification_v2()
        .map_err(|error| failure("fixture-admission", error))?;
    let [a, b, c_initial, d_initial, expected_c, expected_d] = admitted
        .host_buffers()
        .map_err(|error| failure("fixture-buffers", error))?
        .into_parts();
    let sentinel = sentinel();
    if sentinel.len() != expected_c.len()
        || sentinel
            .chunks_exact(size_of::<f32>())
            .zip(expected_c.chunks_exact(size_of::<f32>()))
            .any(|(sentinel, expected)| sentinel == expected)
    {
        return Err(failure(
            "fixture-sentinel",
            "every destination sentinel element must differ from the source result",
        ));
    }
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_peer_qualification_v2(unique_ids)
            .map_err(|error| failure("device-admission", error))?;
    let mut context =
        RuntimeContextV1::open(backend).map_err(|error| failure("context-open", error))?;
    if context.devices().len() != 2
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure(
            "device-roster",
            "exact selected two-device gfx942:xnack- roster required",
        ));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut runs = Vec::with_capacity(2);
    for device in devices {
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("stream-create", error))?;
        let module = context
            .load_module(device, admitted.hsaco())
            .map_err(|error| failure("module-load", error))?;
        let kernel = context
            .resolve_kernel::<Gfx942R57N3QualificationArgumentsV2>(
                module,
                GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
            )
            .map_err(|error| failure("kernel-resolve", error))?;
        let upload = context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
            )
            .map_err(|error| failure("upload-allocation", error))?;
        let mut allocations = Vec::with_capacity(4);
        for bytes in [&a, &b, &c_initial, &d_initial] {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                    GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                )
                .map_err(|error| failure("compute-allocation", error))?;
            upload_full_h2d(&mut context, stream, upload, allocation, bytes)?;
            allocations.push(allocation);
        }
        let allocations: [RuntimeAllocationIdV1; 4] = allocations.try_into().map_err(|_| {
            failure(
                "compute-allocation",
                "exact four-allocation roster required",
            )
        })?;
        runs.push(DeviceRun {
            stream,
            module,
            kernel,
            upload,
            allocations,
        });
    }

    compute_round(&mut context, &runs, false)?;
    for run in &runs {
        verify(&mut context, run.allocations[2], "producer-C", &expected_c)?;
    }
    // Reuse the destination's exact C identity so its unchanged second gate still applies.
    upload_full_h2d(
        &mut context,
        runs[1].stream,
        runs[1].upload,
        runs[1].allocations[2],
        &sentinel,
    )?;
    verify(
        &mut context,
        runs[1].allocations[2],
        "destination-sentinel",
        &sentinel,
    )?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure(
            "native-transport",
            "unexpected prior native peer transfer",
        ));
    }
    let mut copy = context
        .peer_copy(
            runs[1].stream,
            full_region(runs[0].allocations[2], RuntimeAccessV1::Read),
            full_region(runs[1].allocations[2], RuntimeAccessV1::Write),
            &[],
        )
        .map_err(|error| failure("peer-copy-enqueue", error))?;
    let deadline = Instant::now() + WAIT;
    loop {
        context
            .flush_stream(runs[1].stream)
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
                    format!("native peer transfer did not succeed: {status:?}"),
                ));
            }
        }
    }
    let native_copies = context.backend().completed_compute_xgmi_copies_v1();
    if native_copies != 1 {
        return Err(failure(
            "native-transport",
            format!("expected one completed native XGMI copy, observed {native_copies}"),
        ));
    }
    context
        .release_submission(copy)
        .map_err(|error| failure("peer-copy-release", error))?;
    verify(
        &mut context,
        runs[0].allocations[2],
        "source-C-unchanged",
        &expected_c,
    )?;
    verify(
        &mut context,
        runs[1].allocations[2],
        "destination-C-copied",
        &expected_c,
    )?;

    // Finish both independent two-launch authorities, including the source child.
    compute_round(&mut context, &runs, true)?;
    for run in &runs {
        for ((allocation, expected), label) in run
            .allocations
            .into_iter()
            .zip([&a, &b, &expected_c, &expected_d])
            .zip(["final-A", "final-B", "final-C", "final-D"])
        {
            verify(&mut context, allocation, label, expected)?;
        }
    }
    for run in runs.into_iter().rev() {
        for allocation in run.allocations.into_iter().rev() {
            context
                .release_allocation(allocation)
                .map_err(|error| failure("allocation-release", error))?;
        }
        context
            .release_allocation(run.upload)
            .map_err(|error| failure("upload-release", error))?;
        context
            .unload_module(run.module)
            .map_err(|error| failure("module-unload", error))?;
        context
            .destroy_stream(run.stream)
            .map_err(|error| failure("stream-destroy", error))?;
    }
    let mut backend = context
        .shutdown()
        .map_err(|error| failure("context-shutdown", error))?;
    backend
        .shutdown_native_v1()
        .map_err(|error| failure("native-shutdown", error))?;
    println!(
        "PASS schema=fe2o3.compute-xgmi-smoke.v1 fixture={} devices=2 launches=4 launches_per_device=2 peer_copies=1 bytes={} transport=NATIVE-XGMI observed_native_copies={} source_unique_id=0x{:016x} destination_unique_id=0x{:016x} destination_sentinel=full-byte-pass source_unchanged=full-byte-pass output=full-byte-pass readbacks=13 modules=2 allocations=10 cleanup=logical-and-native-explicit performance_acceptance=false formal_refinement=false",
        GFX942_R57_N3_QUALIFICATION_PROFILE_ID_V2,
        expected_c.len(),
        native_copies,
        unique_ids[0],
        unique_ids[1],
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
    fn cli_requires_exactly_two_distinct_nonzero_hexadecimal_ids() {
        assert_eq!(unique_ids(&["0x1".into(), "0xff".into()]), Ok([1, 255]));
        for arguments in [
            vec![],
            vec!["0x1".into()],
            vec!["0x1".into(), "0x2".into(), "0x3".into()],
            vec!["1".into(), "0x2".into()],
            vec!["0x0".into(), "0x2".into()],
            vec!["0x1".into(), "0x01".into()],
            vec!["0x".into(), "0x2".into()],
            vec!["0xg".into(), "0x2".into()],
            vec!["0x+1".into(), "0x2".into()],
            vec!["0x10000000000000000".into(), "0x2".into()],
        ] {
            assert!(unique_ids(&arguments).is_err());
        }
    }

    #[test]
    fn sentinel_distinguishes_every_element_from_exact_r57_output() {
        let admitted = admit_gfx942_r57_n3_qualification_v2().unwrap();
        let buffers = admitted.host_buffers().unwrap();
        let sentinel = sentinel();
        assert_eq!(sentinel.len(), GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1);
        assert_eq!(sentinel.len(), buffers.expected_c().len());
        for (sentinel, expected) in sentinel
            .chunks_exact(4)
            .zip(buffers.expected_c().chunks_exact(4))
        {
            assert_eq!(sentinel, (-1.0_f32).to_bits().to_le_bytes());
            assert_ne!(sentinel, expected);
        }
        assert!(exact_bytes("no-op-copy", &sentinel, buffers.expected_c()).is_err());
        assert!(exact_bytes("correct-copy", buffers.expected_c(), buffers.expected_c()).is_ok());
    }
}
