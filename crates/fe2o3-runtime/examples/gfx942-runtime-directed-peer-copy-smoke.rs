//! Native directed peers and optional dependent D2H with no compute authority.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_kfd::{GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942ComputeXgmiPacketPlanV1};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeAuthorityRequestV1, KfdRuntimeLaunchAuthorityV1,
    RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeCompletionStatusV1, RuntimeContextV1,
    RuntimeCopyV1, RuntimeDirectedScalarPeerCopyV1, RuntimeErrorV1, RuntimeMemoryKindV1,
    RuntimeMemoryRegionV1, RuntimePollV1, RuntimeStreamIdV1, RuntimeSubmissionIdV1,
    RuntimeSubmissionV1, RuntimeValidationErrorV1,
};
use sha2::{Digest, Sha256};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Peer = RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>;
type Copy = RuntimeSubmissionV1<RuntimeCopyV1>;
type ResultV1<T> = Result<T, String>;
const BYTES: u64 = 2 * GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as u64 + 37;
const ROUNDS: usize = 2;
const WAIT: Duration = Duration::from_secs(30);
const USAGE: &str = "usage: gfx942-runtime-directed-peer-copy-smoke [--subranges] [--fanout] [--readback [--late-peer]] <0xA-unique-id> <0xB-unique-id> <0xC-unique-id>";

#[derive(Debug)]
struct NoCompute;

// SAFETY: This authority rejects every invocation, including malformed requests.
unsafe impl KfdRuntimeLaunchAuthorityV1 for NoCompute {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    ids: [u64; 3],
    fanout: bool,
    readback: bool,
    late_peer: bool,
    subranges: bool,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    let (mut fanout, mut readback, mut late_peer, mut flags) = (false, false, false, 0);
    let mut subranges = false;
    for argument in arguments
        .iter()
        .take_while(|argument| argument.starts_with("--"))
    {
        match argument.as_str() {
            "--fanout" if !fanout => fanout = true,
            "--readback" if !readback => readback = true,
            "--late-peer" if !late_peer => late_peer = true,
            "--subranges" if !subranges => subranges = true,
            _ => return Err(USAGE.into()),
        }
        flags += 1;
    }
    let arguments = &arguments[flags..];
    if arguments.len() != 3 || (late_peer && !readback) {
        return Err(USAGE.into());
    }
    let mut ids = [0; 3];
    for (id, argument) in ids.iter_mut().zip(arguments) {
        let hex = argument
            .strip_prefix("0x")
            .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        *id = u64::from_str_radix(hex, 16).map_err(|_| USAGE)?;
    }
    if ids.contains(&0) || ids[0] == ids[1] || ids[0] == ids[2] || ids[1] == ids[2] {
        return Err(USAGE.into());
    }
    Ok(Options {
        ids,
        fanout,
        readback,
        late_peer,
        subranges,
    })
}

fn failure(stage: &str, error: impl std::fmt::Debug) -> String {
    let message = format!("stage={stage} {error:?}");
    eprintln!("directed peer-copy diagnostic: {message}");
    message
}

fn pattern(index: u64, round: usize) -> u8 {
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8 ^ (round as u8).wrapping_mul(73)
}

fn hex_digest(digest: [u8; 32]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in digest {
        write!(result, "{byte:02x}").expect("String formatting");
    }
    result
}

fn check_bytes(observed: &[u8], expected: &[u8]) -> ResultV1<()> {
    if observed.len() != expected.len() {
        return Err(format!("length {} != {}", observed.len(), expected.len()));
    }
    match observed
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        Some(offset) => Err(format!("byte {offset} differs")),
        None => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Layout {
    lengths: [u64; 3],
    offsets: [u64; 3],
    readback_offset: u64,
}

impl Layout {
    const fn new(subranges: bool) -> Self {
        if subranges {
            Self {
                lengths: [BYTES + 257, BYTES + 769, BYTES + 1153],
                offsets: [17, 131, 509],
                readback_offset: 97,
            }
        } else {
            Self {
                lengths: [BYTES; 3],
                offsets: [0; 3],
                readback_offset: 0,
            }
        }
    }
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    byte_offset: u64,
    byte_len: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset,
        byte_len,
    }
}

fn copy_oracle(source: &[u8], destination: &mut [u8], offsets: [u64; 2], bytes: u64) {
    let [source_offset, destination_offset] =
        offsets.map(|offset| usize::try_from(offset).unwrap());
    let bytes = usize::try_from(bytes).unwrap();
    destination[destination_offset..destination_offset + bytes]
        .copy_from_slice(&source[source_offset..source_offset + bytes]);
}

fn expected_devices(inputs: &[Vec<u8>; 3], layout: Layout, fanout: bool) -> [Vec<u8>; 3] {
    let mut expected = inputs.clone();
    let [source, middle, destination] = &mut expected;
    copy_oracle(
        source,
        middle,
        [layout.offsets[0], layout.offsets[1]],
        BYTES,
    );
    let source_index = usize::from(!fanout);
    copy_oracle(
        if fanout { source } else { middle },
        destination,
        [layout.offsets[source_index], layout.offsets[2]],
        BYTES,
    );
    expected
}

fn subrange_digest(devices: &[Vec<u8>; 3], readback: Option<&[u8]>) -> String {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.native-peer-subranges.v1\0");
    for contents in devices {
        digest.update((contents.len() as u64).to_le_bytes());
        digest.update(contents);
    }
    digest.update([u8::from(readback.is_some())]);
    if let Some(contents) = readback {
        digest.update((contents.len() as u64).to_le_bytes());
        digest.update(contents);
    }
    hex_digest(digest.finalize().into())
}

#[derive(Clone, Copy)]
struct Device {
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    data: RuntimeAllocationIdV1,
    bytes: u64,
}

#[derive(Default)]
struct Receipts {
    values: [Option<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>; 6],
    invalid: bool,
}

fn callback<A>(
    context: &mut Context,
    copy: &RuntimeSubmissionV1<A>,
    index: usize,
    receipts: &Arc<Mutex<Receipts>>,
) -> ResultV1<()> {
    let id = copy.id();
    let receipts = Arc::clone(receipts);
    context
        .on_completion(copy, move |status| {
            let mut receipts = receipts.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(slot) = receipts.values.get_mut(index) {
                if slot.replace((id, status)).is_some() {
                    receipts.invalid = true;
                }
            } else {
                receipts.invalid = true;
            }
        })
        .map_err(|error| failure("callback-admission", error))
}

fn verify(
    context: &mut Context,
    allocation: RuntimeAllocationIdV1,
    expected: &[u8],
) -> ResultV1<()> {
    let mut observed = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut observed)
        .map_err(|error| failure("full-readback", error))?;
    check_bytes(&observed, expected).map_err(|error| failure("full-byte-check", error))
}

fn upload(context: &mut Context, device: Device, contents: &[u8]) -> ResultV1<()> {
    context
        .write_allocation(device.host, 0, contents)
        .map_err(|error| failure("host-write", error))?;
    let mut copy = context
        .copy_async(
            device.stream,
            region(device.host, RuntimeAccessV1::Read, 0, device.bytes),
            region(device.data, RuntimeAccessV1::Write, 0, device.bytes),
            &[],
        )
        .map_err(|error| failure("upload-admission", error))?;
    let deadline = Instant::now() + WAIT;
    loop {
        if Instant::now() >= deadline {
            return Err(failure("upload-deadline", copy.id()));
        }
        context
            .flush_stream(device.stream)
            .map_err(|error| failure("upload-flush", error))?;
        match context
            .poll(&mut copy)
            .map_err(|error| failure("upload-poll", error))?
        {
            RuntimePollV1::Succeeded => break,
            RuntimePollV1::Pending => std::thread::sleep(Duration::from_micros(50)),
            other => return Err(failure("upload-result", other)),
        }
    }
    context
        .release_submission(copy)
        .map_err(|error| failure("upload-release", error))
}

fn require_counter(context: &Context, expected: u64) -> ResultV1<()> {
    let observed = context.backend().completed_compute_xgmi_copies_v1();
    if observed != expected {
        return Err(failure("native-counter", (expected, observed)));
    }
    Ok(())
}

fn require_pending<A>(context: &mut Context, copy: &mut RuntimeSubmissionV1<A>) -> ResultV1<()> {
    if context
        .poll(copy)
        .map_err(|error| failure("admission-observer", error))?
        != RuntimePollV1::Pending
        || context
            .wait(copy, Duration::ZERO)
            .map_err(|error| failure("admission-wait", error))?
            != RuntimePollV1::Pending
        || !matches!(
            context.drain(copy, Instant::now()),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidDeadline
            ))
        )
    {
        return Err(failure(
            "admission-state",
            "observer or expired drain advanced work",
        ));
    }
    Ok(())
}

fn seed_first(
    context: &mut Context,
    first: &mut Peer,
    deadline: Instant,
    before: u64,
) -> ResultV1<()> {
    for _ in 0..2 {
        if Instant::now() >= deadline {
            return Err(failure("fanout-seed-deadline", first.id()));
        }
        if context
            .progress_directed_peer_copy_v1(first)
            .map_err(|error| failure("fanout-seed", error))?
            != RuntimePollV1::Pending
        {
            return Err(failure("fanout-seed", "first copy unexpectedly conclusive"));
        }
        require_counter(context, before)?;
    }
    Ok(())
}

fn publish_only_peer(
    context: &mut Context,
    first: &mut Peer,
    deadline: Instant,
    before: u64,
) -> ResultV1<()> {
    if context.backend().retained_compute_xgmi_copies_v1() != 0 {
        return Err(failure(
            "publication-baseline",
            "another native root is retained",
        ));
    }
    loop {
        if Instant::now() >= deadline {
            return Err(failure("publication-deadline", first.id()));
        }
        if context
            .progress_directed_peer_copy_v1(first)
            .map_err(|error| failure("publication-progress", error))?
            != RuntimePollV1::Pending
        {
            return Err(failure(
                "publication-progress",
                "peer retired before observation",
            ));
        }
        require_counter(context, before)?;
        match context.backend().retained_compute_xgmi_copies_v1() {
            0 => {}
            // Only this peer has been admitted. The root retains published native
            // ownership; its hardware fence may already be complete but is not sampled.
            1 => return Ok(()),
            count => return Err(failure("publication-root-count", count)),
        }
    }
}

fn tail_seed_steps(fanout: bool) -> usize {
    if fanout { 2 } else { 3 }
}

fn seed_tail_for_readback(
    context: &mut Context,
    second: &mut Peer,
    fanout: bool,
    deadline: Instant,
    before: u64,
) -> ResultV1<()> {
    loop {
        let observed = context.backend().completed_compute_xgmi_copies_v1();
        if observed == before + 1 {
            break;
        }
        if observed != before || Instant::now() >= deadline {
            return Err(failure("readback-seed-counter-deadline", observed));
        }
        if context
            .progress_directed_peer_copy_v1(second)
            .map_err(|error| failure("readback-seed-progress", error))?
            != RuntimePollV1::Pending
        {
            return Err(failure(
                "readback-seed-result",
                "tail completed before readback admission",
            ));
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    // Consume the chain dependency, if any, leave gating, and seed the native tail.
    // This legacy seed does not inspect the retained native-publication diagnostic.
    for _ in 0..tail_seed_steps(fanout) {
        if Instant::now() >= deadline {
            return Err(failure("readback-tail-seed-deadline", second.id()));
        }
        if context
            .progress_directed_peer_copy_v1(second)
            .map_err(|error| failure("readback-tail-seed", error))?
            != RuntimePollV1::Pending
        {
            return Err(failure(
                "readback-tail-seed",
                "tail unexpectedly conclusive",
            ));
        }
        require_counter(context, before + 1)?;
    }
    Ok(())
}

fn rounds(context: &mut Context, settings: &Options) -> ResultV1<[String; ROUNDS]> {
    if context.devices().len() != 3
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure(
            "device-roster",
            "three gfx942:xnack- devices required",
        ));
    }
    let ids: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let layout = Layout::new(settings.subranges);
    let mut devices = Vec::with_capacity(3);
    for (id, bytes) in ids.iter().zip(layout.lengths) {
        let stream = context
            .create_stream(*id)
            .map_err(|error| failure("stream-create", error))?;
        let host = context
            .allocate(*id, RuntimeMemoryKindV1::HostVisible, bytes, 4096)
            .map_err(|error| failure("host-allocation", error))?;
        let data = context
            .allocate(*id, RuntimeMemoryKindV1::DeviceLocal, bytes, 4096)
            .map_err(|error| failure("device-allocation", error))?;
        devices.push(Device {
            stream,
            host,
            data,
            bytes,
        });
    }
    let readback_stream = if settings.readback {
        Some(
            context
                .create_stream(ids[2])
                .map_err(|error| failure("readback-stream-create", error))?,
        )
    } else {
        None
    };
    let per_round = 2 + usize::from(settings.readback);
    let receipts = Arc::new(Mutex::new(Receipts::default()));
    let mut hashes: [String; ROUNDS] = std::array::from_fn(|_| String::new());
    for (round, hash) in hashes.iter_mut().enumerate() {
        let inputs: [Vec<u8>; 3] = std::array::from_fn(|device| {
            (0..layout.lengths[device])
                .map(|index| pattern(index, round) ^ [0, 0xa5, 0x5a][device])
                .collect()
        });
        let expected = expected_devices(&inputs, layout, settings.fanout);
        let readback_sentinel: Vec<_> = (0..layout.lengths[2])
            .map(|index| pattern(index, round) ^ 0xa5)
            .collect();
        let mut expected_readback = readback_sentinel.clone();
        copy_oracle(
            &expected[2],
            &mut expected_readback,
            [layout.offsets[2], layout.readback_offset],
            BYTES,
        );
        for (device, contents) in devices.iter().zip(&inputs) {
            upload(context, *device, contents)?;
            verify(context, device.data, contents)?;
        }
        if settings.readback {
            context
                .write_allocation(devices[2].host, 0, &readback_sentinel)
                .map_err(|error| failure("readback-sentinel-write", error))?;
            verify(context, devices[2].host, &readback_sentinel)?;
        }
        let before = (round * 2) as u64;
        require_counter(context, before)?;
        let mut first = context
            .directed_peer_copy_v1(
                devices[1].stream,
                region(
                    devices[0].data,
                    RuntimeAccessV1::Read,
                    layout.offsets[0],
                    BYTES,
                ),
                region(
                    devices[1].data,
                    RuntimeAccessV1::Write,
                    layout.offsets[1],
                    BYTES,
                ),
                &[],
            )
            .map_err(|error| failure("first-peer-admission", error))?;
        let event = context
            .record_event(&first)
            .map_err(|error| failure("first-event", error))?;
        let deadline = Instant::now() + WAIT;
        if settings.late_peer {
            publish_only_peer(context, &mut first, deadline, before)?;
        }
        let mut second = context
            .directed_peer_copy_v1(
                devices[2].stream,
                region(
                    devices[usize::from(!settings.fanout)].data,
                    RuntimeAccessV1::Read,
                    layout.offsets[usize::from(!settings.fanout)],
                    BYTES,
                ),
                region(
                    devices[2].data,
                    RuntimeAccessV1::Write,
                    layout.offsets[2],
                    BYTES,
                ),
                if settings.fanout {
                    &[]
                } else {
                    std::slice::from_ref(&event)
                },
            )
            .map_err(|error| failure("second-peer-admission", error))?;
        if settings.late_peer && context.backend().retained_compute_xgmi_copies_v1() != 1 {
            return Err(failure(
                "late-admission",
                "admission changed retained native roots",
            ));
        }
        let second_event = context
            .record_event(&second)
            .map_err(|error| failure("second-event", error))?;
        if !settings.readback {
            context
                .release_event(second_event)
                .map_err(|error| failure("second-event-release", error))?;
            context
                .release_event(event)
                .map_err(|error| failure("first-event-release", error))?;
        }
        callback(context, &first, round * per_round, &receipts)?;
        callback(context, &second, round * per_round + 1, &receipts)?;
        let mut expected_ids = vec![first.id(), second.id()];
        for copy in [&mut first, &mut second] {
            require_pending(context, copy)?;
        }
        require_counter(context, before)?;
        let mut readback: Option<Copy> = None;
        if let Some(stream) = readback_stream {
            if round == 1 {
                if settings.fanout && !settings.late_peer {
                    seed_first(context, &mut first, deadline, before)?;
                }
                seed_tail_for_readback(context, &mut second, settings.fanout, deadline, before)?;
            }
            let mut copy = context
                .copy_async(
                    stream,
                    region(
                        devices[2].data,
                        RuntimeAccessV1::Read,
                        layout.offsets[2],
                        BYTES,
                    ),
                    region(
                        devices[2].host,
                        RuntimeAccessV1::Write,
                        layout.readback_offset,
                        BYTES,
                    ),
                    &[second_event],
                )
                .map_err(|error| failure("readback-admission", error))?;
            callback(context, &copy, round * per_round + 2, &receipts)?;
            expected_ids.push(copy.id());
            require_pending(context, &mut copy)?;
            require_counter(context, before + u64::from(round == 1))?;
            context
                .release_event(second_event)
                .map_err(|error| failure("second-event-release", error))?;
            context
                .release_event(event)
                .map_err(|error| failure("first-event-release", error))?;
            readback = Some(copy);
        }
        if settings.fanout && !settings.late_peer && (!settings.readback || round == 0) {
            seed_first(context, &mut first, deadline, before)?;
        }
        loop {
            if Instant::now() >= deadline {
                return Err(failure("final-operation-deadline", second.id()));
            }
            let status = if let Some(copy) = readback.as_mut() {
                context
                    .flush_stream(readback_stream.unwrap())
                    .map_err(|error| failure("readback-only-flush", error))?;
                context
                    .drain(copy, deadline)
                    .map_err(|error| failure("readback-only-drain", error))?
            } else {
                context
                    .progress_directed_peer_copy_v1(&mut second)
                    .map_err(|error| failure("final-peer-progress", error))?
            };
            match status {
                RuntimePollV1::Succeeded => break,
                RuntimePollV1::Pending => std::thread::sleep(Duration::from_micros(50)),
                other => return Err(failure("final-operation-result", other)),
            }
        }
        require_counter(context, before + 2)?;
        if context
            .poll(&mut first)
            .map_err(|error| failure("first-peer-observer", error))?
            != RuntimePollV1::Succeeded
        {
            return Err(failure(
                "first-peer-result",
                "first copy was not settled by final-operation progress",
            ));
        }
        for copy in [&mut first, &mut second] {
            if context
                .drain(copy, deadline)
                .map_err(|error| failure("completed-drain", error))?
                != RuntimePollV1::Succeeded
            {
                return Err(failure("completed-drain", "settled copy changed status"));
            }
        }
        {
            let receipts = receipts
                .lock()
                .map_err(|_| failure("callbacks", "poisoned"))?;
            if receipts.invalid
                || receipts.values[..(round + 1) * per_round]
                    .iter()
                    .any(Option::is_none)
                || expected_ids.into_iter().enumerate().any(|(index, id)| {
                    receipts.values[round * per_round + index]
                        != Some((id, RuntimeCompletionStatusV1::Succeeded))
                })
            {
                return Err(failure(
                    "callbacks",
                    "missing, duplicate or unsuccessful exact receipt",
                ));
            }
        }
        if let Some(copy) = readback {
            if context
                .query_submission(&copy)
                .map_err(|error| failure("readback-status", error))?
                != RuntimeCompletionStatusV1::Succeeded
            {
                return Err(failure(
                    "readback-status",
                    "original result is not successful",
                ));
            }
            verify(context, devices[2].host, &expected_readback)?;
            context
                .release_submission(copy)
                .map_err(|error| failure("readback-release", error))?;
        }
        context
            .release_submission(second)
            .map_err(|error| failure("second-peer-release", error))?;
        context
            .release_submission(first)
            .map_err(|error| failure("first-peer-release", error))?;
        for (device, expected) in devices.iter().zip(&expected) {
            verify(context, device.data, expected)?;
        }
        *hash = if settings.subranges {
            subrange_digest(
                &expected,
                settings.readback.then_some(expected_readback.as_slice()),
            )
        } else {
            hex_digest(Sha256::digest(&inputs[0]).into())
        };
    }
    if hashes[0] == hashes[1] {
        return Err(failure("round-digests", "payload did not change"));
    }
    if let Some(stream) = readback_stream {
        context
            .destroy_stream(stream)
            .map_err(|error| failure("readback-stream-destroy", error))?;
    }
    for device in devices.into_iter().rev() {
        context
            .release_allocation(device.data)
            .map_err(|error| failure("device-release", error))?;
        context
            .release_allocation(device.host)
            .map_err(|error| failure("host-release", error))?;
        context
            .destroy_stream(device.stream)
            .map_err(|error| failure("stream-destroy", error))?;
    }
    Ok(hashes)
}

fn run(settings: Options) -> ResultV1<()> {
    let plan = Gfx942ComputeXgmiPacketPlanV1::new(BYTES).ok_or("invalid packet plan")?;
    if plan.count() != 3 || plan.packet(2).map(|packet| packet.bytes) != Some(37) {
        return Err("unexpected packet extent".into());
    }
    let authorities = settings
        .ids
        .into_iter()
        .map(|id| {
            (
                id,
                Box::new(NoCompute) as Box<dyn KfdRuntimeLaunchAuthorityV1>,
            )
        })
        .collect();
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_default_with_native_peer_copy_v1(authorities)
            .map_err(|error| failure("device-admission", error))?;
    // On an error, retain uncertain native custody until this bounded witness process exits.
    let mut context = ManuallyDrop::new(
        Context::open_with_version_journal_members_v1(backend, 16, 16, 16)
            .map_err(|error| failure("context-open", error))?,
    );
    let hashes = rounds(&mut context, &settings)?;
    require_counter(&context, 4)?;
    let cleanup = context.cleanup();
    if !cleanup.is_complete() || !cleanup.failures().is_empty() {
        return Err(failure("logical-cleanup", cleanup));
    }
    let mut backend = match ManuallyDrop::into_inner(context).shutdown() {
        Ok(backend) => ManuallyDrop::new(backend),
        Err(error) => {
            let error = ManuallyDrop::new(error);
            return Err(failure("context-shutdown", &*error));
        }
    };
    backend
        .shutdown_native_v1()
        .map_err(|error| failure("native-shutdown", error))?;
    drop(ManuallyDrop::into_inner(backend));
    if settings.subranges {
        let layout = Layout::new(true);
        println!(
            "PASS schema=fe2o3.native-peer-subranges.v1 authority=production-deny-all transport=NATIVE-XGMI mode={} devices=3 unique_ids=0x{:016x},0x{:016x},0x{:016x} rounds=2 bytes_per_copy={BYTES} logical_extents={},{},{} peer_offsets={},{},{} readback_offset={} readback={} late_peer={} peer_copies=4 native_packets=12 observed_native_copies=4 native_counter=0,2,4 allocations=6 streams={} launches=0 modules=0 completion_receipts={} readbacks={} d2h_copies={} progress={} first_publication_observed={} public_events=released-before-tail-drive observers=pending expired_drain=rejected journal=enabled source_unchanged=full-byte-pass outside_destination_ranges=full-byte-pass output=full-byte-pass readback_guards={} digest=length-prefixed-devices-and-optional-readback round_sha256={},{} rounds_changed=true allocation_reuse=true contexts=1 cleanup=logical-and-native-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
            if settings.fanout { "fanout" } else { "chain" },
            settings.ids[0],
            settings.ids[1],
            settings.ids[2],
            layout.lengths[0],
            layout.lengths[1],
            layout.lengths[2],
            layout.offsets[0],
            layout.offsets[1],
            layout.offsets[2],
            layout.readback_offset,
            settings.readback,
            settings.late_peer,
            3 + usize::from(settings.readback),
            4 + 2 * usize::from(settings.readback),
            12 + 4 * usize::from(settings.readback),
            2 * usize::from(settings.readback),
            if settings.readback {
                "readback-only-after-seed"
            } else {
                "final-directed-peer-only-after-seed"
            },
            settings.late_peer,
            if settings.readback {
                "full-byte-pass"
            } else {
                "not-requested"
            },
            hashes[0],
            hashes[1],
        );
        return Ok(());
    }
    if settings.readback {
        let readback_admission = if settings.late_peer {
            "before-tail-progress,after-bounded-seed"
        } else {
            "before-progress,after-bounded-seed"
        };
        let publication_field = if settings.late_peer {
            "readback_publication_observed"
        } else {
            "publication_observed"
        };
        let mut evidence = format!(
            "PASS schema={} authority=production-deny-all transport=NATIVE-XGMI mode={} devices=3 unique_ids=0x{:016x},0x{:016x},0x{:016x} rounds=2 bytes_per_copy={BYTES} peer_copies=4 native_packets=12 observed_native_copies=4 native_counter=0,2,4 allocations=6 streams=4 launches=0 modules=0 completion_receipts=6 readbacks=16 admission={} progress=readback-only-after-seed first_seed={} public_events=released-after-readback-admission-before-tail-drive {}=pending expired_drain=rejected journal=enabled source_unchanged=full-byte-pass destination_sentinels=full-byte-pass output=full-byte-pass round_sha256={},{} rounds_changed=true allocation_reuse=true contexts=1 drain=pending-readback-then-completed-peers cleanup=logical-and-native-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false d2h_copies=2 readback_admission={readback_admission} readback_admission_native_counts=0,3 tail_seed_steps={} {publication_field}=false readback_sentinels=full-byte-pass",
            if settings.late_peer {
                "fe2o3.late-directed-peer-readback-smoke.v1"
            } else {
                "fe2o3.directed-peer-readback-smoke.v1"
            },
            if settings.fanout { "fanout" } else { "chain" },
            settings.ids[0],
            settings.ids[1],
            settings.ids[2],
            if settings.late_peer {
                "second-after-first-publication"
            } else {
                "peers-before-progress"
            },
            if settings.late_peer {
                "observed-native-publication"
            } else if settings.fanout {
                "two-bounded-steps"
            } else {
                "none"
            },
            if settings.late_peer {
                "pre_tail_observers"
            } else {
                "pre_progress_observers"
            },
            hashes[0],
            hashes[1],
            tail_seed_steps(settings.fanout),
        );
        if settings.late_peer {
            evidence.push_str(" first_publication_observed=true retained_at_peer_admission=1");
        }
        println!("{evidence}");
    } else {
        println!(
            "PASS schema=fe2o3.directed-peer-copy-smoke.v1 authority=production-deny-all transport=NATIVE-XGMI mode={} devices=3 unique_ids=0x{:016x},0x{:016x},0x{:016x} rounds=2 bytes_per_copy={BYTES} peer_copies=4 native_packets=12 observed_native_copies=4 native_counter=0,2,4 allocations=6 streams=3 launches=0 modules=0 completion_receipts=4 readbacks=12 admission=both-before-progress progress=final-directed-peer-only first_seed={} public_events=released-before-progress pre_progress_observers=pending expired_drain=rejected journal=enabled source_unchanged=full-byte-pass destination_sentinels=full-byte-pass output=full-byte-pass round_sha256={},{} rounds_changed=true allocation_reuse=true contexts=1 drain=completed-only cleanup=logical-and-native-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
            if settings.fanout { "fanout" } else { "chain" },
            settings.ids[0],
            settings.ids[1],
            settings.ids[2],
            if settings.fanout {
                "two-bounded-steps"
            } else {
                "none"
            },
            hashes[0],
            hashes[1],
        );
    }
    Ok(())
}

fn main() {
    if let Err(error) = options(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(run) {
        eprintln!("directed peer-copy smoke failed before complete cleanup: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> ResultV1<Options> {
        options(
            &arguments
                .iter()
                .map(|text| text.to_string())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn cli_preserves_three_distinct_devices_and_exact_mode() {
        assert_eq!(
            parse(&["0x3", "0x2", "0x1"]).unwrap(),
            Options {
                ids: [3, 2, 1],
                fanout: false,
                readback: false,
                late_peer: false,
                subranges: false,
            }
        );
        assert_eq!(
            parse(&["--fanout", "0x1", "0x2", "0x3"]).unwrap(),
            Options {
                ids: [1, 2, 3],
                fanout: true,
                readback: false,
                late_peer: false,
                subranges: false,
            }
        );
        for arguments in [
            vec![],
            vec!["0x1", "0x2"],
            vec!["0x1", "0x2", "0x3", "0x4"],
            vec!["0x0", "0x2", "0x3"],
            vec!["0x1", "0x2", "0x1"],
            vec!["0x1", "0x1", "0x3"],
            vec!["0x1", "0x3", "0x3"],
            vec!["1", "0x2", "0x3"],
            vec!["0x+1", "0x2", "0x3"],
            vec!["0x10000000000000000", "0x2", "0x3"],
            vec!["--chain", "0x1", "0x2", "0x3"],
            vec!["--fanout", "--fanout", "0x1", "0x2", "0x3"],
            vec!["0x1", "--fanout", "0x2", "0x3"],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn readback_cli_composes_modes_but_rejects_duplicate_and_misplaced_flags() {
        for flags in [
            vec!["--readback"],
            vec!["--readback", "--fanout"],
            vec!["--fanout", "--readback"],
        ] {
            let mut arguments = flags.clone();
            arguments.extend(["0x3", "0x2", "0x1"]);
            assert_eq!(
                parse(&arguments).unwrap(),
                Options {
                    ids: [3, 2, 1],
                    fanout: flags.contains(&"--fanout"),
                    readback: true,
                    late_peer: false,
                    subranges: false,
                }
            );
        }
        for arguments in [
            vec!["--readback", "--readback", "0x1", "0x2", "0x3"],
            vec!["--readback", "--fanout", "--fanout", "0x1", "0x2", "0x3"],
            vec!["--readback", "--chain", "0x1", "0x2", "0x3"],
            vec!["--readback", "0x1", "0x2"],
            vec!["0x1", "--readback", "0x2", "0x3"],
            vec!["--readback", "0x1", "0x1", "0x3"],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn late_peer_requires_readback_and_accepts_flag_orders() {
        for flags in [
            vec!["--late-peer", "--readback"],
            vec!["--readback", "--late-peer"],
            vec!["--fanout", "--late-peer", "--readback"],
            vec!["--readback", "--fanout", "--late-peer"],
        ] {
            let mut arguments = flags.clone();
            arguments.extend(["0x3", "0x2", "0x1"]);
            assert_eq!(
                parse(&arguments).unwrap(),
                Options {
                    ids: [3, 2, 1],
                    fanout: flags.contains(&"--fanout"),
                    readback: true,
                    late_peer: true,
                    subranges: false,
                }
            );
        }
        for arguments in [
            vec!["--late-peer", "0x1", "0x2", "0x3"],
            vec![
                "--readback",
                "--late-peer",
                "--late-peer",
                "0x1",
                "0x2",
                "0x3",
            ],
            vec!["--readback", "0x1", "--late-peer", "0x2", "0x3"],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn subrange_cli_composes_modes_without_changing_defaults() {
        for flags in [
            vec!["--subranges"],
            vec!["--fanout", "--subranges"],
            vec!["--readback", "--subranges"],
            vec!["--subranges", "--fanout", "--readback", "--late-peer"],
        ] {
            let mut arguments = flags.clone();
            arguments.extend(["0x3", "0x2", "0x1"]);
            assert_eq!(
                parse(&arguments).unwrap(),
                Options {
                    ids: [3, 2, 1],
                    fanout: flags.contains(&"--fanout"),
                    readback: flags.contains(&"--readback"),
                    late_peer: flags.contains(&"--late-peer"),
                    subranges: true,
                }
            );
        }
        for arguments in [
            vec!["--subranges", "--subranges", "0x1", "0x2", "0x3"],
            vec!["0x1", "--subranges", "0x2", "0x3"],
            vec!["--subranges", "--late-peer", "0x1", "0x2", "0x3"],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?}");
        }
        assert_eq!(
            Layout::new(false),
            Layout {
                lengths: [BYTES; 3],
                offsets: [0; 3],
                readback_offset: 0,
            }
        );
    }

    #[test]
    fn subrange_oracle_preserves_both_guards_and_detects_corruption() {
        let layout = Layout::new(true);
        assert_eq!(layout.lengths, [8_388_838, 8_389_350, 8_389_734]);
        let inputs: [Vec<u8>; 3] = std::array::from_fn(|device| {
            (0..layout.lengths[device])
                .map(|index| pattern(index, 0) ^ [0, 0xa5, 0x5a][device])
                .collect()
        });
        let expected = expected_devices(&inputs, layout, false);
        assert_eq!(expected, expected_devices(&inputs, layout, true));
        assert_eq!(expected[0], inputs[0]);
        for device in 1..3 {
            let start = layout.offsets[device];
            let end = start + BYTES;
            assert!(start > 0 && end < layout.lengths[device]);
            for (index, &observed) in expected[device].iter().enumerate() {
                let index = index as u64;
                let byte = if (start..end).contains(&index) {
                    pattern(layout.offsets[0] + index - start, 0)
                } else {
                    pattern(index, 0) ^ [0, 0xa5, 0x5a][device]
                };
                assert_eq!(observed, byte);
            }
            let mut changed = expected[device].clone();
            for index in [
                0,
                start - 1,
                start,
                end - 1,
                end,
                layout.lengths[device] - 1,
            ] {
                changed[index as usize] ^= 1;
                assert!(check_bytes(&changed, &expected[device]).is_err());
                changed[index as usize] ^= 1;
            }
        }
        let mut host: Vec<_> = (0..layout.lengths[2])
            .map(|index| pattern(index, 0) ^ 0xa5)
            .collect();
        copy_oracle(
            &expected[2],
            &mut host,
            [layout.offsets[2], layout.readback_offset],
            BYTES,
        );
        for (index, &observed) in host.iter().enumerate() {
            let index = index as u64;
            let byte = if (layout.readback_offset..layout.readback_offset + BYTES).contains(&index)
            {
                pattern(layout.offsets[0] + index - layout.readback_offset, 0)
            } else {
                pattern(index, 0) ^ 0xa5
            };
            assert_eq!(observed, byte);
        }
    }

    #[test]
    fn subrange_digest_binds_lengths_all_devices_and_optional_readback() {
        let original = [vec![1, 2], vec![3], vec![4, 5]];
        let first = subrange_digest(&original, None);
        assert_ne!(
            first,
            subrange_digest(&[vec![1], vec![2, 3], vec![4, 5]], None)
        );
        assert_ne!(first, subrange_digest(&original, Some(&[])));
        for device in 0..3 {
            let mut changed = original.clone();
            changed[device][0] ^= 1;
            assert_ne!(first, subrange_digest(&changed, None));
        }
        assert_ne!(
            subrange_digest(&original, Some(&[7])),
            subrange_digest(&original, Some(&[8]))
        );
    }

    #[test]
    fn readback_seed_and_receipt_bounds_are_finite() {
        assert_eq!(tail_seed_steps(false), 3);
        assert_eq!(tail_seed_steps(true), 2);
        assert_eq!(Receipts::default().values.len(), ROUNDS * 3);
        for round in 0..ROUNDS {
            let before = (round * 2) as u64;
            assert_eq!(before + u64::from(round == 1), [0, 3][round]);
        }
    }

    #[test]
    fn payload_rounds_and_both_sentinels_differ_at_every_byte() {
        assert_eq!(pattern(0, 0), 0xaf);
        for index in [0, 1, 255, 4095, 4194271, 4194272, BYTES - 1, u64::MAX] {
            let first = pattern(index, 0);
            assert_eq!(pattern(index, 1), first ^ 73);
            assert_ne!(first, first ^ 0xa5);
            assert_ne!(first, first ^ 0x5a);
            assert_ne!(first ^ 0xa5, first ^ 0x5a);
        }
    }

    #[test]
    fn full_byte_oracle_rejects_partial_copy_corruption_and_wrong_extent() {
        let expected: Vec<_> = (0..129).map(|index| pattern(index, 0)).collect();
        assert!(check_bytes(&expected, &expected).is_ok());
        for offset in [0, 64, 128] {
            let mut corrupted = expected.clone();
            corrupted[offset] ^= 1;
            assert!(check_bytes(&corrupted, &expected).is_err());
        }
        for xor in [0xa5, 0x5a, 73] {
            let wrong: Vec<_> = expected.iter().map(|byte| byte ^ xor).collect();
            assert!(check_bytes(&wrong, &expected).is_err());
        }
        assert!(check_bytes(&expected[..128], &expected).is_err());
        assert!(check_bytes(&[expected.as_slice(), &[0]].concat(), &expected).is_err());
    }

    #[test]
    fn extent_has_three_packets_with_an_odd_tail_without_large_allocation() {
        assert_eq!(BYTES, 8_388_581);
        let plan = Gfx942ComputeXgmiPacketPlanV1::new(BYTES).unwrap();
        assert_eq!(plan.count(), 3);
        assert_eq!(plan.packet(2).unwrap().offset, 8_388_544);
        assert_eq!(plan.packet(2).unwrap().bytes, 37);
        assert!(plan.packet(3).is_none());
    }

    #[test]
    fn copy_only_authority_rejects_empty_and_arbitrary_kernel_requests() {
        for image in [&[][..], &[0xff; 16][..]] {
            assert!(
                !NoCompute.authorize_launch_v1(KfdRuntimeAuthorityRequestV1 {
                    module_image: image,
                    module_sha256: [0; 32],
                    kernel_name: "not-authorized",
                    signature: [0; 32],
                    explicit_kernarg: image,
                    complete_kernarg_template: image,
                    bindings: &[],
                    dispatch_abi: &[],
                    allocations: &[],
                    geometry: fe2o3_runtime::RuntimeLaunchGeometryV1 {
                        grid: [1; 3],
                        workgroup: [1; 3],
                        dynamic_shared_bytes: 0,
                    },
                    semantic_launch: fe2o3_runtime::KfdRuntimeSemanticLaunchV1::Ordinary,
                })
            );
        }
    }
}
