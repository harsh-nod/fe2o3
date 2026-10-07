//! Exact two-GPU R57 compute, native XGMI copy, and local consumer qualification.

use std::time::{Duration, Instant};

use fe2o3_kfd::{
    GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, GFX942_SDMA_RING_BYTES_V1,
    GFX942_SDMA_SUBMISSION_BYTES_V1, Gfx942ComputeXgmiPacketPlanV1,
};
use fe2o3_runtime::qualification_gfx942_r57_n3_v1::{
    GFX942_R57_N3_QUALIFICATION_BUFFER_ALIGNMENT_V1, GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1,
    GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1, GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
    GFX942_R57_N3_QUALIFICATION_PROFILE_ID_V2, Gfx942R57N3QualificationArgumentsV2,
    admit_gfx942_r57_n3_qualification_v2,
};
use fe2o3_runtime::{
    KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1, KfdMultiDeviceRuntimeBackendV1, RuntimeAccessV1,
    RuntimeAllocationIdV1, RuntimeContextV1, RuntimeErrorV1, RuntimeMemoryKindV1,
    RuntimeMemoryRegionV1, RuntimeModuleIdV1, RuntimePollV1, RuntimeStreamIdV1,
    RuntimeValidationErrorV1, TypedRuntimeKernelV1,
};

type ContextV1 = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const WAIT: Duration = Duration::from_secs(30);
const USAGE: &str = "usage: gfx942-runtime-compute-xgmi-smoke [--queued-consumer] [--packetized-copy] [--ring-reuse] <0xsource-unique-id> <0xdestination-unique-id>";
const PACKETIZED_EXTENTS: [u64; 2] = [
    GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64 + 1,
    2 * GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64 + 37,
];
const RING_SLOTS: usize = GFX942_SDMA_RING_BYTES_V1 as usize / GFX942_SDMA_SUBMISSION_BYTES_V1;
const RING_REUSE_EXTENT: u64 = RING_SLOTS as u64 * GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64 + 1;
const RING_REUSE_ITERATIONS: usize = 2;

#[derive(Debug, Eq, PartialEq)]
struct Options {
    unique_ids: [u64; 2],
    queued_consumer: bool,
    packetized_copy: bool,
    ring_reuse: bool,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    let mut queued_consumer = false;
    let mut packetized_copy = false;
    let mut ring_reuse = false;
    let mut ids = arguments;
    while let Some(flag) = ids.first().filter(|arg| arg.starts_with("--")) {
        match flag.as_str() {
            "--queued-consumer" if !queued_consumer => queued_consumer = true,
            "--packetized-copy" if !packetized_copy => packetized_copy = true,
            "--ring-reuse" if !ring_reuse => ring_reuse = true,
            _ => return Err(USAGE.into()),
        }
        ids = &ids[1..];
    }
    Ok(Options {
        unique_ids: unique_ids(ids)?,
        queued_consumer,
        packetized_copy,
        ring_reuse,
    })
}

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
    region(
        allocation,
        access,
        GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64,
    )
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    byte_len: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len,
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
            region(upload, RuntimeAccessV1::Read, bytes.len() as u64),
            region(destination, RuntimeAccessV1::Write, bytes.len() as u64),
            &[],
        )
        .map_err(|error| failure("upload-enqueue", error))?;
    let deadline = Instant::now() + WAIT;
    let mut status = RuntimePollV1::Pending;
    loop {
        if Instant::now() >= deadline {
            return Err(failure(
                "upload-progress",
                format!("full H2D upload deadline expired: status={status:?}"),
            ));
        }
        // Completed windows retain Pending custody until an explicit continuation flush.
        context
            .flush_stream(stream)
            .map_err(|error| failure("upload-flush", error))?;
        status = context
            .poll(&mut submission)
            .map_err(|error| failure("upload-poll", error))?;
        match status {
            RuntimePollV1::Pending => std::thread::sleep(Duration::from_micros(50)),
            RuntimePollV1::Succeeded => break,
            _ => {
                return Err(failure(
                    "upload-progress",
                    format!("full H2D upload did not succeed: status={status:?}"),
                ));
            }
        }
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

fn packetized_pattern_byte(index: u64) -> u8 {
    // Mix the absolute offset so replaying packet zero is observable across boundaries.
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8
}

fn packetized_copies(context: &mut ContextV1, runs: &[DeviceRun]) -> ResultV1<()> {
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for (case, bytes) in PACKETIZED_EXTENTS.into_iter().enumerate() {
        let plan = Gfx942ComputeXgmiPacketPlanV1::new(bytes)
            .ok_or_else(|| failure("packet-plan", "invalid witness extent"))?;
        if plan.count() != case + 2 {
            return Err(failure("packet-plan", "unexpected witness packet count"));
        }
        let source_bytes: Vec<_> = (0..bytes).map(packetized_pattern_byte).collect();
        let destination_bytes: Vec<_> = source_bytes.iter().map(|byte| !byte).collect();
        // These auxiliary data allocations do not extend either exact kernel authority.
        let mut pairs = Vec::with_capacity(2);
        for (index, contents) in [&source_bytes, &destination_bytes].into_iter().enumerate() {
            let upload = context
                .allocate(
                    devices[index],
                    RuntimeMemoryKindV1::HostVisible,
                    bytes,
                    4096,
                )
                .map_err(|error| failure("packet-upload-allocation", error))?;
            let data = context
                .allocate(
                    devices[index],
                    RuntimeMemoryKindV1::DeviceLocal,
                    bytes,
                    4096,
                )
                .map_err(|error| failure("packet-data-allocation", error))?;
            upload_full_h2d(context, runs[index].stream, upload, data, contents)?;
            verify(context, data, "packet-initial-contents", contents)?;
            pairs.push((upload, data));
        }
        let expected_before = case as u64 + 1;
        if context.backend().completed_compute_xgmi_copies_v1() != expected_before {
            return Err(failure(
                "packet-native-before",
                "unexpected native copy count",
            ));
        }
        let mut copy = context
            .peer_copy(
                runs[1].stream,
                region(pairs[0].1, RuntimeAccessV1::Read, bytes),
                region(pairs[1].1, RuntimeAccessV1::Write, bytes),
                &[],
            )
            .map_err(|error| failure("packet-copy-enqueue", error))?;
        if context
            .poll(&mut copy)
            .map_err(|error| failure("packet-copy-poll", error))?
            != RuntimePollV1::Pending
            || context.backend().completed_compute_xgmi_copies_v1() != expected_before
        {
            return Err(failure(
                "packet-copy-observer",
                "unflushed copy must stay pending",
            ));
        }
        if context
            .drain(&mut copy, Instant::now() + WAIT)
            .map_err(|error| failure("packet-copy-drain", error))?
            != RuntimePollV1::Succeeded
            || context.backend().completed_compute_xgmi_copies_v1() != expected_before + 1
        {
            return Err(failure(
                "packet-copy-drain",
                "expected one completed native logical copy",
            ));
        }
        context
            .release_submission(copy)
            .map_err(|error| failure("packet-copy-release", error))?;
        verify(
            context,
            pairs[0].1,
            "packet-source-unchanged",
            &source_bytes,
        )?;
        verify(
            context,
            pairs[1].1,
            "packet-destination-copied",
            &source_bytes,
        )?;
        for (upload, data) in pairs.into_iter().rev() {
            context
                .release_allocation(data)
                .map_err(|error| failure("packet-data-release", error))?;
            context
                .release_allocation(upload)
                .map_err(|error| failure("packet-upload-release", error))?;
        }
    }
    Ok(())
}

fn ring_reuse_pattern_byte(index: u64, iteration: usize) -> u8 {
    packetized_pattern_byte(index) ^ (iteration as u8).wrapping_mul(0x5b)
}

fn verify_ring_reuse(
    context: &mut ContextV1,
    allocation: RuntimeAllocationIdV1,
    label: &str,
    expected: &[u8],
) -> ResultV1<()> {
    let mut observed = vec![0; GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize];
    for (index, expected) in expected.chunks(observed.len()).enumerate() {
        let offset = index as u64 * u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        let observed = &mut observed[..expected.len()];
        context
            .read_allocation(allocation, offset, observed)
            .map_err(|error| failure(label, error))?;
        if observed != expected {
            let mismatch = observed
                .iter()
                .zip(expected)
                .position(|(a, b)| a != b)
                .unwrap();
            return Err(failure(
                label,
                format!("full-byte mismatch at {}", offset + mismatch as u64),
            ));
        }
    }
    Ok(())
}

fn ring_reuse_copies(context: &mut ContextV1, runs: &[DeviceRun]) -> ResultV1<()> {
    // The native full-extent route retains one physical ring across all packets.
    // This binds the completed 65-packet copy to its 64 slots, not raw ring telemetry.
    let plan = Gfx942ComputeXgmiPacketPlanV1::new(RING_REUSE_EXTENT)
        .ok_or_else(|| failure("ring-plan", "invalid witness extent"))?;
    if RING_SLOTS != 64
        || plan.count() != RING_SLOTS + 1
        || plan
            .packet(RING_SLOTS)
            .is_none_or(|packet| packet.bytes != 1)
        || RING_REUSE_EXTENT > KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1
    {
        return Err(failure(
            "ring-plan",
            "65 packets within the allocation cap required",
        ));
    }
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let mut pairs = Vec::with_capacity(2);
    for device in devices {
        let upload = context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                RING_REUSE_EXTENT,
                4096,
            )
            .map_err(|error| failure("ring-upload-allocation", error))?;
        let data = context
            .allocate(
                device,
                RuntimeMemoryKindV1::DeviceLocal,
                RING_REUSE_EXTENT,
                4096,
            )
            .map_err(|error| failure("ring-data-allocation", error))?;
        pairs.push((upload, data));
    }
    // Refill one host vector and reuse both device identities; no large compute authority.
    let mut contents = vec![0; RING_REUSE_EXTENT as usize];
    let initial_copies = context.backend().completed_compute_xgmi_copies_v1();
    for iteration in 0..RING_REUSE_ITERATIONS {
        for (index, byte) in contents.iter_mut().enumerate() {
            *byte = ring_reuse_pattern_byte(index as u64, iteration);
        }
        for (index, (run, &(upload, data))) in runs.iter().zip(&pairs).enumerate() {
            if index == 1 {
                for byte in &mut contents {
                    *byte = !*byte;
                }
            }
            upload_full_h2d(context, run.stream, upload, data, &contents)?;
            verify_ring_reuse(context, data, "ring-initial-contents", &contents)?;
        }
        for byte in &mut contents {
            *byte = !*byte;
        }
        let expected_before = initial_copies + iteration as u64;
        if context.backend().completed_compute_xgmi_copies_v1() != expected_before {
            return Err(failure(
                "ring-native-before",
                "unexpected native logical-copy count",
            ));
        }
        let mut copy = context
            .peer_copy(
                runs[1].stream,
                region(pairs[0].1, RuntimeAccessV1::Read, RING_REUSE_EXTENT),
                region(pairs[1].1, RuntimeAccessV1::Write, RING_REUSE_EXTENT),
                &[],
            )
            .map_err(|error| failure("ring-copy-enqueue", error))?;
        if context
            .poll(&mut copy)
            .map_err(|error| failure("ring-unflushed-poll", error))?
            != RuntimePollV1::Pending
            || context
                .wait(&mut copy, Duration::ZERO)
                .map_err(|error| failure("ring-unflushed-wait", error))?
                != RuntimePollV1::Pending
            || context.backend().completed_compute_xgmi_copies_v1() != expected_before
        {
            return Err(failure(
                "ring-unflushed-observers",
                "copy must remain pending and uncounted",
            ));
        }
        let deadline = Instant::now() + WAIT;
        loop {
            if Instant::now() >= deadline {
                return Err(failure(
                    "ring-copy-progress",
                    "bounded native copy deadline expired",
                ));
            }
            context
                .flush_stream(runs[1].stream)
                .map_err(|error| failure("ring-copy-flush", error))?;
            let status = context
                .poll(&mut copy)
                .map_err(|error| failure("ring-copy-poll", error))?;
            let completed = context.backend().completed_compute_xgmi_copies_v1();
            match status {
                RuntimePollV1::Pending if completed == expected_before => {
                    std::thread::sleep(Duration::from_micros(50));
                }
                RuntimePollV1::Succeeded if completed == expected_before + 1 => break,
                _ => {
                    return Err(failure(
                        "ring-copy-logical-count",
                        format!(
                            "status={status:?} expected_before={expected_before} observed={completed}"
                        ),
                    ));
                }
            }
        }
        context
            .release_submission(copy)
            .map_err(|error| failure("ring-copy-release", error))?;
        verify_ring_reuse(context, pairs[0].1, "ring-source-unchanged", &contents)?;
        verify_ring_reuse(context, pairs[1].1, "ring-destination-copied", &contents)?;
    }
    for (upload, data) in pairs.into_iter().rev() {
        context
            .release_allocation(data)
            .map_err(|error| failure("ring-data-release", error))?;
        context
            .release_allocation(upload)
            .map_err(|error| failure("ring-upload-release", error))?;
    }
    Ok(())
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

fn run(options: Options) -> ResultV1<()> {
    let unique_ids = &options.unique_ids;
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
    let mut context = if options.queued_consumer {
        RuntimeContextV1::open_with_version_journal_v1(backend, 16, 16)
            .map_err(|error| failure("context-open-with-journal", error))?
    } else {
        RuntimeContextV1::open(backend).map_err(|error| failure("context-open", error))?
    };
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
    if context
        .poll(&mut copy)
        .map_err(|error| failure("peer-copy-before-flush-poll", error))?
        != RuntimePollV1::Pending
        || context
            .wait(&mut copy, Duration::ZERO)
            .map_err(|error| failure("peer-copy-before-flush-wait", error))?
            != RuntimePollV1::Pending
        || context.backend().completed_compute_xgmi_copies_v1() != 0
    {
        return Err(failure(
            "peer-copy-before-flush",
            "observers must not publish or complete the unflushed copy",
        ));
    }
    if !matches!(
        context.drain(&mut copy, Instant::now()),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidDeadline
        ))
    ) || context.backend().completed_compute_xgmi_copies_v1() != 0
    {
        return Err(failure(
            "peer-copy-expired-drain",
            "expired drain must reject without completing the copy",
        ));
    }
    let status = if options.queued_consumer {
        let event = context
            .record_event(&copy)
            .map_err(|error| failure("peer-copy-event", error))?;
        let destination = &runs[1];
        let [_, b, c, d] = destination.allocations;
        let arguments = Gfx942R57N3QualificationArgumentsV2::new(c, b, d)
            .map_err(|error| failure("queued-consumer-arguments", error))?;
        let mut consumer = context
            .launch_producer_aware_v1(
                destination.stream,
                &destination.kernel,
                &arguments,
                GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
                &[event],
            )
            .map_err(|error| failure("queued-consumer-enqueue", error))?;
        // The accepted consumer retains its producer independently of this event.
        context
            .release_event(event)
            .map_err(|error| failure("peer-copy-event-release", error))?;
        if context
            .poll(&mut consumer)
            .map_err(|error| failure("queued-consumer-before-flush-poll", error))?
            != RuntimePollV1::Pending
            || context
                .wait(&mut consumer, Duration::ZERO)
                .map_err(|error| failure("queued-consumer-before-flush-wait", error))?
                != RuntimePollV1::Pending
            || !matches!(
                context.drain(&mut consumer, Instant::now()),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidDeadline
                ))
            )
            || context.backend().completed_compute_xgmi_copies_v1() != 0
        {
            return Err(failure(
                "queued-consumer-before-flush",
                "observers and expired drain must retain the unflushed dependency",
            ));
        }
        let deadline = Instant::now() + WAIT;
        // One bounded call may yield while reconciling the producer's journal entry.
        let status = loop {
            let status = context
                .drain(&mut consumer, deadline)
                .map_err(|error| failure("queued-consumer-drain", error))?;
            if status != RuntimePollV1::Pending || Instant::now() >= deadline {
                break status;
            }
            std::thread::sleep(Duration::from_micros(50));
        };
        if status != RuntimePollV1::Succeeded {
            return Err(failure(
                "queued-consumer-drain",
                format!("native dependent consumer did not succeed: {status:?}"),
            ));
        }
        context
            .release_submission(consumer)
            .map_err(|error| failure("queued-consumer-release", error))?;
        context
            .poll(&mut copy)
            .map_err(|error| failure("peer-copy-reconciled-poll", error))?
    } else {
        context
            .drain(&mut copy, Instant::now() + WAIT)
            .map_err(|error| failure("peer-copy-drain", error))?
    };
    if status != RuntimePollV1::Succeeded {
        return Err(failure(
            "peer-copy-drain",
            format!("native peer transfer did not succeed: {status:?}"),
        ));
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
    compute_round(
        &mut context,
        if options.queued_consumer {
            &runs[..1]
        } else {
            &runs
        },
        true,
    )?;
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
    if options.packetized_copy {
        packetized_copies(&mut context, &runs)?;
    }
    if options.ring_reuse {
        ring_reuse_copies(&mut context, &runs)?;
    }
    let native_copies = context.backend().completed_compute_xgmi_copies_v1();
    let packetized_copies = if options.packetized_copy {
        PACKETIZED_EXTENTS.len()
    } else {
        0
    };
    let ring_copies = if options.ring_reuse {
        RING_REUSE_ITERATIONS
    } else {
        0
    };
    let extra_copies = packetized_copies + ring_copies;
    if native_copies != 1 + extra_copies as u64 {
        return Err(failure(
            "final-native-count",
            "unexpected final native logical-copy count",
        ));
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
    let consumer_admission = if options.queued_consumer {
        "before-copy-progress"
    } else {
        "after-copy-drain"
    };
    let copy_progress = if options.queued_consumer {
        "consumer-drain"
    } else {
        "explicit-drain"
    };
    println!(
        "PASS schema=fe2o3.compute-xgmi-smoke.v1 fixture={} devices=2 launches=4 launches_per_device=2 peer_copies={} bytes={} transport=NATIVE-XGMI observed_native_copies={} source_unique_id=0x{:016x} destination_unique_id=0x{:016x} pre_flush_observers=pending expired_drain=rejected copy_progress={} consumer_admission={} destination_sentinel=full-byte-pass source_unchanged=full-byte-pass output=full-byte-pass readbacks={} modules=2 allocations={} packetized_copies={} packetized_packets={} packetized_bytes={} ring_reuse_copies={} ring_reuse_packets={} ring_reuse_bytes={} ring_reuse_extent={} ring_slots={} ring_reuse_evidence={} cleanup=logical-and-native-explicit performance_acceptance=false formal_refinement=false",
        GFX942_R57_N3_QUALIFICATION_PROFILE_ID_V2,
        1 + extra_copies,
        expected_c.len(),
        native_copies,
        unique_ids[0],
        unique_ids[1],
        copy_progress,
        consumer_admission,
        13 + 4 * extra_copies,
        10 + 4 * packetized_copies + if options.ring_reuse { 4 } else { 0 },
        packetized_copies,
        if options.packetized_copy { 5 } else { 0 },
        if options.packetized_copy {
            PACKETIZED_EXTENTS.iter().sum::<u64>()
        } else {
            0
        },
        ring_copies,
        ring_copies * (RING_SLOTS + 1),
        ring_copies as u64 * RING_REUSE_EXTENT,
        if options.ring_reuse {
            RING_REUSE_EXTENT
        } else {
            0
        },
        RING_SLOTS,
        if options.ring_reuse {
            "single-native-queue-full-extent"
        } else {
            "disabled"
        },
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = options(&std::env::args().skip(1).collect::<Vec<_>>())?;
    run(options).map_err(Into::into)
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

    #[test]
    fn packetized_witness_covers_two_and_three_packets_with_nonperiodic_boundaries() {
        let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        for (index, bytes) in PACKETIZED_EXTENTS.into_iter().enumerate() {
            let plan = Gfx942ComputeXgmiPacketPlanV1::new(bytes).unwrap();
            assert_eq!(plan.count(), index + 2);
            let last = plan.packet(plan.count() - 1).unwrap();
            assert_eq!(last.offset + u64::from(last.bytes), bytes);
            assert_eq!(last.bytes, if index == 0 { 1 } else { 37 });
        }
        for offset in [cap - 1, cap, 2 * cap - 1, 2 * cap] {
            let window: Vec<_> = (offset..offset + 32).map(packetized_pattern_byte).collect();
            let first: Vec<_> = (0..32).map(packetized_pattern_byte).collect();
            assert_ne!(window, first);
            assert!(window.iter().all(|byte| *byte != !byte));
        }
    }

    #[test]
    fn packetized_cli_accepts_both_flag_orders_and_rejects_duplicates() {
        for flags in [
            vec!["--packetized-copy"],
            vec!["--queued-consumer", "--packetized-copy"],
            vec!["--packetized-copy", "--queued-consumer"],
        ] {
            let mut arguments: Vec<String> = flags.iter().map(|flag| (*flag).into()).collect();
            arguments.extend(["0x1".into(), "0x2".into()]);
            assert_eq!(
                options(&arguments),
                Ok(Options {
                    unique_ids: [1, 2],
                    queued_consumer: flags.len() == 2,
                    packetized_copy: true,
                    ring_reuse: false,
                })
            );
        }
        for arguments in [
            vec!["--packetized-copy", "--packetized-copy", "0x1", "0x2"],
            vec!["0x1", "0x2", "--packetized-copy"],
            vec!["--unknown", "0x1", "0x2"],
            vec!["--packetized-copy", "0x1"],
        ] {
            assert!(options(&arguments.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn ring_reuse_extent_requires_physical_slot_reuse_with_one_byte_tail() {
        assert_eq!(RING_SLOTS, 64);
        assert_eq!(RING_REUSE_EXTENT, 268_433_409);
        assert_eq!(RING_REUSE_ITERATIONS, 2);
        let plan = Gfx942ComputeXgmiPacketPlanV1::new(RING_REUSE_EXTENT).unwrap();
        assert_eq!(plan.count(), 65);
        assert_eq!(plan.total_bytes(), RING_REUSE_EXTENT);
        assert_eq!(plan.packet(64).unwrap().bytes, 1);
        assert_eq!(plan.packet(64).unwrap().offset + 1, RING_REUSE_EXTENT);
        assert_eq!(
            RING_REUSE_EXTENT.div_ceil(4096) * 4096,
            KFD_RUNTIME_MAX_STAGED_ALLOCATION_BYTES_V1
        );
        assert_eq!(
            RING_REUSE_ITERATIONS as u64 * RING_REUSE_EXTENT,
            536_866_818
        );
    }

    #[test]
    fn ring_reuse_upload_requires_two_directional_windows() {
        use fe2o3_kfd::{
            GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_BYTES_V1,
            GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1,
        };

        let window_packets = GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1;
        let window_bytes = GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_BYTES_V1;
        let plan = Gfx942ComputeXgmiPacketPlanV1::new(RING_REUSE_EXTENT).unwrap();
        assert_eq!(window_packets, 63);
        assert_eq!(plan.count().div_ceil(window_packets), 2);
        assert_eq!(RING_REUSE_EXTENT.div_ceil(window_bytes), 2);
        assert_eq!(plan.count() - window_packets, 2);
        assert_eq!(
            RING_REUSE_EXTENT - window_bytes,
            u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) + 1
        );
    }

    #[test]
    fn ring_reuse_payloads_change_every_byte_and_distinguish_replayed_packets() {
        let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
        for packet in 0..=RING_SLOTS {
            let offset = packet as u64 * cap;
            let end = (offset + 32).min(RING_REUSE_EXTENT);
            let first: Vec<_> = (0..end - offset)
                .map(|index| ring_reuse_pattern_byte(index, 0))
                .collect();
            let window: Vec<_> = (offset..end)
                .map(|index| ring_reuse_pattern_byte(index, 0))
                .collect();
            if packet != 0 {
                assert_ne!(window, first);
            }
            for index in offset..end {
                let previous = ring_reuse_pattern_byte(index, 0);
                let next = ring_reuse_pattern_byte(index, 1);
                assert_ne!(previous, next);
                assert_ne!(!previous, next);
                assert_ne!(!next, previous);
                assert_ne!(!next, next);
            }
        }
    }

    #[test]
    fn ring_reuse_cli_accepts_composed_modes_and_rejects_duplicates() {
        for flags in [
            vec!["--ring-reuse"],
            vec!["--ring-reuse", "--queued-consumer"],
            vec!["--queued-consumer", "--ring-reuse"],
            vec!["--ring-reuse", "--packetized-copy"],
            vec!["--packetized-copy", "--ring-reuse"],
            vec!["--ring-reuse", "--queued-consumer", "--packetized-copy"],
            vec!["--ring-reuse", "--packetized-copy", "--queued-consumer"],
            vec!["--queued-consumer", "--ring-reuse", "--packetized-copy"],
            vec!["--queued-consumer", "--packetized-copy", "--ring-reuse"],
            vec!["--packetized-copy", "--ring-reuse", "--queued-consumer"],
            vec!["--packetized-copy", "--queued-consumer", "--ring-reuse"],
        ] {
            let mut arguments: Vec<String> = flags.iter().map(|flag| (*flag).into()).collect();
            arguments.extend(["0x1".into(), "0x2".into()]);
            assert_eq!(
                options(&arguments),
                Ok(Options {
                    unique_ids: [1, 2],
                    queued_consumer: flags.contains(&"--queued-consumer"),
                    packetized_copy: flags.contains(&"--packetized-copy"),
                    ring_reuse: true,
                })
            );
        }
        for flags in [
            vec!["--ring-reuse", "--ring-reuse", "0x1", "0x2"],
            vec![
                "--ring-reuse",
                "--queued-consumer",
                "--ring-reuse",
                "0x1",
                "0x2",
            ],
            vec!["0x1", "0x2", "--ring-reuse"],
            vec!["--ring-reuse", "0x1"],
        ] {
            assert!(options(&flags.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
    }
}
#[test]
fn queued_consumer_is_an_explicit_leading_option() {
    for queued_consumer in [false, true] {
        let mut arguments = Vec::new();
        if queued_consumer {
            arguments.push("--queued-consumer".into());
        }
        arguments.extend(["0x1".into(), "0x2".into()]);
        assert_eq!(
            options(&arguments),
            Ok(Options {
                unique_ids: [1, 2],
                queued_consumer,
                packetized_copy: false,
                ring_reuse: false,
            }),
        );
    }
    for arguments in [
        vec!["--queued-consumer".into()],
        vec!["--queued-consumer".into(), "0x1".into()],
        vec!["0x1".into(), "0x2".into(), "--queued-consumer".into()],
        vec![
            "--queued-consumer".into(),
            "--queued-consumer".into(),
            "0x1".into(),
            "0x2".into(),
        ],
    ] {
        assert!(options(&arguments).is_err());
    }
}
