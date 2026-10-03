//! Native host-preparation rejection while a disjoint pair retains its owners.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_runtime::*;
use sha2::{Digest, Sha256};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
type BackendResult<T> = Result<T, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>;
type Submit<A> = fn(
    &mut Context,
    RuntimeStreamIdV1,
    RuntimeAllocationIdV1,
    RuntimeAllocationIdV1,
) -> BackendResult<RuntimeSubmissionV1<A>>;
type Receipts = Arc<Mutex<Vec<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>>;

const SOURCE_BYTES: u64 = 65_536 + 97;
const DESTINATION_BYTES: u64 = 65_536 + 321;
const SOURCE_BASE: u64 = 17;
const DESTINATION_BASE: u64 = 53;
const SCALAR_BYTES: u64 = 32_769;
const SOURCE_ENVELOPE: u64 = 32_768;
const DESTINATION_ENVELOPE: u64 = 49_152;
const WAIT: Duration = Duration::from_secs(30);
const TICKS: usize = 30_000;
const USAGE: &str = "usage: gfx942-runtime-xgmi-recovery-smoke <--no-journal|--journal> <scalar|segments> <0xA-source> <0xA-destination> <0xB-source> <0xB-destination>";
const DOMAIN: &[u8] = b"fe2o3.native-xgmi-recovery.full-bytes.v1\0";

#[derive(Debug)]
struct NoCompute;

// SAFETY: Every launch is denied, independent of artifact, inputs, or device.
unsafe impl KfdRuntimeLaunchAuthorityV1 for NoCompute {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    Scalar,
    Segments,
}

impl Shape {
    fn name(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
            Self::Segments => "segments",
        }
    }

    fn descriptors(self) -> Vec<RuntimePeerCopySegmentV1> {
        let triples: &[(u64, u64, u64)] = match self {
            Self::Scalar => &[(0, 0, SCALAR_BYTES)],
            Self::Segments => &[(0, 0, 128), (128, 64, 128), (0, 0, 128), (512, 96, 68)],
        };
        triples
            .iter()
            .map(
                |&(source_offset, destination_offset, byte_len)| RuntimePeerCopySegmentV1 {
                    source_offset,
                    destination_offset,
                    byte_len,
                },
            )
            .collect()
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    journal: bool,
    shape: Shape,
    ids: [u64; 4],
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    if arguments.len() != 6 {
        return Err(USAGE.into());
    }
    let journal = match arguments[0].as_str() {
        "--no-journal" => false,
        "--journal" => true,
        _ => return Err(USAGE.into()),
    };
    let shape = match arguments[1].as_str() {
        "scalar" => Shape::Scalar,
        "segments" => Shape::Segments,
        _ => return Err(USAGE.into()),
    };
    let mut ids = [0; 4];
    for (index, text) in arguments[2..].iter().enumerate() {
        let hex = text
            .strip_prefix("0x")
            .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        ids[index] = u64::from_str_radix(hex, 16).map_err(|_| USAGE)?;
        if ids[index] == 0 || ids[..index].contains(&ids[index]) {
            return Err(USAGE.into());
        }
    }
    Ok(Options {
        journal,
        shape,
        ids,
    })
}

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let message = format!("stage={stage} {error:?}");
    eprintln!("xgmi recovery diagnostic: {message}");
    message
}

fn require(condition: bool, stage: &str) -> ResultV1<()> {
    if condition {
        Ok(())
    } else {
        Err(failure(stage, "qualification invariant failed"))
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

fn pattern(index: u64, pair: u8, round: u8) -> u8 {
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8 ^ pair.wrapping_mul(53) ^ round.wrapping_mul(73)
}

fn source_bytes(pair: u8, round: u8) -> Vec<u8> {
    (0..SOURCE_BYTES)
        .map(|index| pattern(index, pair, round))
        .collect()
}

fn destination_bytes(shape: Shape, source: &[u8], sentinel: u8) -> Vec<u8> {
    let mut destination = vec![sentinel; DESTINATION_BYTES as usize];
    for segment in shape.descriptors() {
        let start = (SOURCE_BASE + segment.source_offset) as usize;
        let target = (DESTINATION_BASE + segment.destination_offset) as usize;
        let len = segment.byte_len as usize;
        destination[target..target + len].copy_from_slice(&source[start..start + len]);
    }
    destination
}

fn digest(parts: &[Vec<u8>]) -> String {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    for bytes in parts {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn counters(context: &Context, completed: u64, retained: usize) -> ResultV1<()> {
    require(
        !context.is_terminal()
            && context.backend().completed_compute_xgmi_copies_v1() == completed
            && context.backend().retained_compute_xgmi_copies_v1() == retained,
        "native-counters-and-liveness",
    )
}

fn read_exact(
    context: &mut Context,
    allocation: RuntimeAllocationIdV1,
    expected: &[u8],
) -> ResultV1<Vec<u8>> {
    let mut actual = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut actual)
        .map_err(|error| failure("full-readback", error))?;
    if let Some(index) = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right)
    {
        return Err(failure(
            "full-byte-oracle",
            (index, actual[index], expected[index]),
        ));
    }
    Ok(actual)
}

fn upload(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    device: RuntimeAllocationIdV1,
    bytes: &[u8],
) -> ResultV1<()> {
    context
        .write_allocation(host, 0, bytes)
        .map_err(|error| failure("upload-write", error))?;
    let mut copy = context
        .copy_async(
            stream,
            region(host, RuntimeAccessV1::Read, 0, bytes.len() as u64),
            region(device, RuntimeAccessV1::Write, 0, bytes.len() as u64),
            &[],
        )
        .map_err(|error| failure("upload-admission", error))?;
    let deadline = Instant::now() + WAIT;
    for _ in 0..TICKS {
        context
            .flush_stream(stream)
            .map_err(|error| failure("upload-flush", error))?;
        match context
            .poll(&mut copy)
            .map_err(|error| failure("upload-poll", error))?
        {
            RuntimePollV1::Succeeded => {
                return context
                    .release_submission(copy)
                    .map_err(|error| failure("upload-release", error));
            }
            RuntimePollV1::Pending if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_micros(50));
            }
            status => return Err(failure("upload-status", status)),
        }
    }
    Err(failure("upload-bound", TICKS))
}

fn submit_scalar(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
) -> BackendResult<RuntimeSubmissionV1<RuntimePeerCopyV1>> {
    context.peer_copy(
        stream,
        region(source, RuntimeAccessV1::Read, SOURCE_BASE, SCALAR_BYTES),
        region(
            destination,
            RuntimeAccessV1::Write,
            DESTINATION_BASE,
            SCALAR_BYTES,
        ),
        &[],
    )
}

fn submit_segments(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
) -> BackendResult<RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>> {
    let mut descriptors = Shape::Segments.descriptors();
    let result = context.peer_copy_segments(
        stream,
        region(source, RuntimeAccessV1::Read, SOURCE_BASE, SOURCE_ENVELOPE),
        region(
            destination,
            RuntimeAccessV1::Write,
            DESTINATION_BASE,
            DESTINATION_ENVELOPE,
        ),
        &descriptors,
        &[],
    );
    descriptors.fill(RuntimePeerCopySegmentV1 {
        source_offset: u64::MAX,
        destination_offset: u64::MAX,
        byte_len: 0,
    });
    result
}

fn watch<A>(
    context: &mut Context,
    submission: &RuntimeSubmissionV1<A>,
    receipts: &Receipts,
) -> ResultV1<()> {
    let id = submission.id();
    let receipts = Arc::clone(receipts);
    context
        .on_completion(submission, move |status| {
            receipts
                .lock()
                .expect("receipt mutex poisoned")
                .push((id, status));
        })
        .map_err(|error| failure("callback-registration", error))
}

fn certified<A>(context: &mut Context, submission: &RuntimeSubmissionV1<A>) -> ResultV1<bool> {
    context
        .native_xgmi_host_preparation_rejected_for_qualification_v1(submission)
        .map_err(|error| failure("certified-rejection-observer", error))
}

fn drive<A>(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    submission: &mut RuntimeSubmissionV1<A>,
    rejection: bool,
) -> ResultV1<RuntimeCompletionStatusV1> {
    let deadline = Instant::now() + WAIT;
    for _ in 0..TICKS {
        let progress = context.progress_stream_v1(stream);
        if let Err(error) = &progress
            && (!rejection || !matches!(error, RuntimeErrorV1::BackendQuiescent(_)))
        {
            return Err(failure("peer-progress", error));
        }
        let polled = context
            .poll(submission)
            .map_err(|error| failure("peer-poll", error))?;
        let stored = context
            .query_submission(submission)
            .map_err(|error| failure("peer-query", error))?;
        match (polled, stored) {
            (RuntimePollV1::Succeeded, RuntimeCompletionStatusV1::Succeeded) if !rejection => {
                return Ok(stored);
            }
            (
                RuntimePollV1::Failed { code },
                RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(
                    observed,
                )),
            ) if rejection && code == observed && certified(context, submission)? => {
                return Ok(stored);
            }
            (RuntimePollV1::Pending, RuntimeCompletionStatusV1::Pending)
                if progress.is_ok() && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_micros(50));
            }
            state => return Err(failure("peer-status", state)),
        }
    }
    Err(failure("peer-progress-bound", TICKS))
}

#[derive(Clone, Copy)]
struct Storage {
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    device: RuntimeAllocationIdV1,
}

fn require_reserved<T>(result: BackendResult<T>, stage: &str) -> ResultV1<()> {
    match result {
        Err(RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextReserved)) => Ok(()),
        Err(error) => Err(failure(stage, error)),
        Ok(_) => Err(failure(stage, "unexpected admission")),
    }
}

fn exercise<A>(context: &mut Context, options: &Options, submit: Submit<A>) -> ResultV1<String> {
    require(
        context.devices().len() == 4
            && context
                .devices()
                .iter()
                .all(|device| device.target() == "gfx942:xnack-"),
        "device-roster",
    )?;
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let a0 = source_bytes(0, 0);
    let a1 = source_bytes(0, 1);
    let b0 = source_bytes(1, 0);
    let initial_a = vec![0xa5; DESTINATION_BYTES as usize];
    let initial_b = vec![0x6c; DESTINATION_BYTES as usize];
    let initial = [&a0, &initial_a, &b0, &initial_b];
    let mut storage = Vec::new();
    for (device, bytes) in devices.iter().zip(initial) {
        let stream = context
            .create_stream(*device)
            .map_err(|error| failure("setup-stream", error))?;
        let host = context
            .allocate(
                *device,
                RuntimeMemoryKindV1::HostVisible,
                bytes.len() as u64,
                4096,
            )
            .map_err(|error| failure("host-allocation", error))?;
        let data = context
            .allocate(
                *device,
                RuntimeMemoryKindV1::DeviceLocal,
                bytes.len() as u64,
                4096,
            )
            .map_err(|error| failure("device-allocation", error))?;
        upload(context, stream, host, data, bytes)?;
        read_exact(context, data, bytes)?;
        storage.push(Storage {
            stream,
            host,
            device: data,
        });
    }
    counters(context, 0, 0)?;
    let a_stream = context
        .create_stream(devices[1])
        .map_err(|error| failure("a-stream", error))?;
    let b_stream = context
        .create_stream(devices[3])
        .map_err(|error| failure("b-stream", error))?;
    let retry_stream = context
        .create_stream(devices[1])
        .map_err(|error| failure("retry-stream", error))?;
    let receipts = Arc::new(Mutex::new(Vec::with_capacity(3)));
    let mut b = submit(context, b_stream, storage[2].device, storage[3].device)
        .map_err(|error| failure("b-admission", error))?;
    watch(context, &b, &receipts)?;
    let deadline = Instant::now() + WAIT;
    for _ in 0..TICKS {
        context
            .progress_stream_v1(b_stream)
            .map_err(|error| failure("b-publication", error))?;
        require(
            context
                .query_submission(&b)
                .map_err(|error| failure("b-query", error))?
                == RuntimeCompletionStatusV1::Pending,
            "b-not-completed-at-seed",
        )?;
        if context.backend().retained_compute_xgmi_copies_v1() == 1 {
            break;
        }
        require(Instant::now() < deadline, "b-seed-deadline")?;
        std::thread::sleep(Duration::from_micros(50));
    }
    counters(context, 0, 1)?;
    require(!certified(context, &b)?, "b-unarmed-before-a")?;
    require(
        context
            .reject_native_xgmi_host_preparation_once_for_qualification_v1(&b)
            .is_err(),
        "published-b-arm-denied",
    )?;
    let mut a = submit(context, a_stream, storage[0].device, storage[1].device)
        .map_err(|error| failure("a-admission", error))?;
    watch(context, &a, &receipts)?;
    require(!certified(context, &a)?, "a-not-rejected-before-arm")?;
    context
        .reject_native_xgmi_host_preparation_once_for_qualification_v1(&a)
        .map_err(|error| failure("a-arm", error))?;
    require(
        context
            .reject_native_xgmi_host_preparation_once_for_qualification_v1(&a)
            .is_err(),
        "a-rearm-denied",
    )?;
    counters(context, 0, 1)?;
    let failed = drive(context, a_stream, &mut a, true)?;
    counters(context, 0, 1)?;
    require(
        certified(context, &a)? && !certified(context, &b)?,
        "exact-a-only-rejection",
    )?;
    require(
        context
            .query_submission(&b)
            .map_err(|error| failure("b-retained-query", error))?
            == RuntimeCompletionStatusV1::Pending,
        "b-retained-through-a-failure",
    )?;
    drive(context, b_stream, &mut b, false)?;
    counters(context, 1, 0)?;
    require(
        certified(context, &a)? && !certified(context, &b)?,
        "certified-after-b-restoration",
    )?;
    read_exact(context, storage[0].device, &a0)?;
    read_exact(context, storage[2].device, &b0)?;
    let b_expected = destination_bytes(options.shape, &b0, 0x6c);
    read_exact(context, storage[3].device, &b_expected)?;
    let journal_before = (
        context.version_journal_usage_v1(),
        context.version_journal_writer_records_v1(),
        context.version_journal_read_records_v1(),
    );
    // Synchronous inspection does not acquire a stable reader or promote lineage.
    read_exact(context, storage[1].device, &initial_a)?;
    require(
        journal_before
            == (
                context.version_journal_usage_v1(),
                context.version_journal_writer_records_v1(),
                context.version_journal_read_records_v1(),
            ),
        "inspection-preserves-journal",
    )?;
    let a_id = a.id();
    let b_id = b.id();
    if options.journal {
        require_reserved(
            context.copy_async(
                retry_stream,
                region(
                    storage[1].device,
                    RuntimeAccessV1::Read,
                    0,
                    DESTINATION_BYTES,
                ),
                region(
                    storage[1].host,
                    RuntimeAccessV1::Write,
                    0,
                    DESTINATION_BYTES,
                ),
                &[],
            ),
            "unknown-read-denied",
        )?;
        require_reserved(
            submit(context, retry_stream, storage[0].device, storage[1].device),
            "unknown-retry-denied",
        )?;
        require(
            journal_before
                == (
                    context.version_journal_usage_v1(),
                    context.version_journal_writer_records_v1(),
                    context.version_journal_read_records_v1(),
                ),
            "unknown-read-retry-preserves-journal",
        )?;
        counters(context, 1, 0)?;
        require(
            context
                .query_stream(retry_stream)
                .map_err(|error| failure("rejected-retry-stream", error))?
                == RuntimeStreamObservationV1::default(),
            "unknown-retry-no-submission",
        )?;
        let old = storage[1].device;
        context
            .release_allocation(old)
            .map_err(|error| failure("unknown-destination-dispose", error))?;
        context
            .release_submission(a)
            .map_err(|error| failure("failed-a-release", error))?;
        storage[1].device = context
            .allocate(
                devices[1],
                RuntimeMemoryKindV1::DeviceLocal,
                DESTINATION_BYTES,
                4096,
            )
            .map_err(|error| failure("fresh-destination", error))?;
        require(storage[1].device != old, "fresh-destination-identity")?;
        upload(
            context,
            storage[1].stream,
            storage[1].host,
            storage[1].device,
            &initial_a,
        )?;
        read_exact(context, storage[1].device, &initial_a)?;
    } else {
        context
            .release_submission(a)
            .map_err(|error| failure("failed-a-release", error))?;
    }
    context
        .release_submission(b)
        .map_err(|error| failure("b-release", error))?;
    upload(
        context,
        storage[0].stream,
        storage[0].host,
        storage[0].device,
        &a1,
    )?;
    read_exact(context, storage[0].device, &a1)?;
    let mut retry = submit(context, retry_stream, storage[0].device, storage[1].device)
        .map_err(|error| failure("a-retry", error))?;
    let retry_id = retry.id();
    watch(context, &retry, &receipts)?;
    require(!certified(context, &retry)?, "retry-unarmed")?;
    drive(context, retry_stream, &mut retry, false)?;
    counters(context, 2, 0)?;
    require(!certified(context, &retry)?, "retry-not-injected")?;
    let expected = [
        a1,
        destination_bytes(options.shape, &source_bytes(0, 1), 0xa5),
        b0,
        b_expected,
    ];
    let mut observed = Vec::with_capacity(4);
    for (item, bytes) in storage.iter().zip(&expected) {
        observed.push(read_exact(context, item.device, bytes)?);
    }
    let expected_receipts = [
        (a_id, failed),
        (b_id, RuntimeCompletionStatusV1::Succeeded),
        (retry_id, RuntimeCompletionStatusV1::Succeeded),
    ];
    let actual_receipts = receipts
        .lock()
        .map_err(|error| failure("receipt-lock", error))?;
    require(
        actual_receipts.len() == 3
            && expected_receipts.iter().all(|expected| {
                actual_receipts
                    .iter()
                    .filter(|actual| *actual == expected)
                    .count()
                    == 1
            }),
        "exact-three-callbacks",
    )?;
    drop(actual_receipts);
    require(
        context.completion_callback_panic_count() == 0,
        "callbacks-no-panic",
    )?;
    require(
        context
            .drain(&mut retry, Instant::now() + WAIT)
            .map_err(|error| failure("completed-drain", error))?
            == RuntimePollV1::Succeeded,
        "completed-drain-success",
    )?;
    context
        .release_submission(retry)
        .map_err(|error| failure("retry-release", error))?;
    for stream in [a_stream, b_stream, retry_stream] {
        require(
            context
                .query_stream(stream)
                .map_err(|error| failure("final-stream-query", error))?
                == RuntimeStreamObservationV1::default(),
            "all-results-released",
        )?;
        context
            .destroy_stream(stream)
            .map_err(|error| failure("peer-stream-release", error))?;
    }
    for item in storage {
        context
            .release_allocation(item.device)
            .map_err(|error| failure("device-release", error))?;
        context
            .release_allocation(item.host)
            .map_err(|error| failure("host-release", error))?;
        context
            .destroy_stream(item.stream)
            .map_err(|error| failure("setup-stream-release", error))?;
    }
    counters(context, 2, 0)?;
    Ok(report(options, &digest(&observed)))
}

fn report(options: &Options, digest: &str) -> String {
    format!(
        "PASS schema=fe2o3.native-xgmi-recovery.v1 authority=production-deny-all transport=NATIVE-XGMI devices=4 unique_ids=0x{:016x},0x{:016x},0x{:016x},0x{:016x} mode={} shape={} source_bytes={SOURCE_BYTES} destination_bytes={DESTINATION_BYTES} source_offset={SOURCE_BASE} destination_offset={DESTINATION_BASE} descriptors={} copied_bytes={} source_envelope={} destination_envelope={} kernels=0 modules=0 contexts=1 attempts=3 injected_failures=1 successful_peers=2 completion_receipts=3 native_counter=0,1,2 b_publication=retained1-completed0-exact-only-root a_failure=certified-host-preparation-capacity-before-native-arm a_owners=certified-originals-restored b_during_a_failure=pending-retained-unarmed terminal=false rearm=rejected source_retry=changed-full-payload-same-allocation destination_retry={} failed_destination_bytes=full-byte-unchanged journal_unknown_read={} journal_unknown_retry={} source_preservation=full-byte-pass output=full-byte-pass guards=full-byte-pass digest=domain-and-u64le-length-prefixed-A-source-A-destination-B-source-B-destination sha256={digest} descriptor_snapshot={} callbacks=exact-once-per-original-submission events=none drain=completed-only cleanup=logical-and-native-explicit gpu_fault_injected=false physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
        options.ids[0],
        options.ids[1],
        options.ids[2],
        options.ids[3],
        if options.journal {
            "journal"
        } else {
            "no-journal"
        },
        options.shape.name(),
        options.shape.descriptors().len(),
        options
            .shape
            .descriptors()
            .iter()
            .map(|segment| segment.byte_len)
            .sum::<u64>(),
        if options.shape == Shape::Scalar {
            SCALAR_BYTES
        } else {
            SOURCE_ENVELOPE
        },
        if options.shape == Shape::Scalar {
            SCALAR_BYTES
        } else {
            DESTINATION_ENVELOPE
        },
        if options.journal {
            "disposed-and-fresh-initialized-allocation"
        } else {
            "same-original-allocation"
        },
        if options.journal {
            "rejected-before-effect"
        } else {
            "not-applicable"
        },
        if options.journal {
            "rejected-before-effect"
        } else {
            "not-applicable"
        },
        if options.shape == Shape::Segments {
            "caller-overwritten-after-admission"
        } else {
            "not-applicable"
        },
    )
}

fn run<A>(options: Options, submit: Submit<A>) -> ResultV1<()> {
    let devices = options
        .ids
        .into_iter()
        .map(|id| {
            (
                id,
                Box::new(NoCompute) as Box<dyn KfdRuntimeLaunchAuthorityV1>,
            )
        })
        .collect();
    let backend = KfdMultiDeviceRuntimeBackendV1::open_default_with_native_peer_copy_v1(devices)
        .map_err(|error| failure("device-admission", error))?;
    let opened = if options.journal {
        Context::open_with_version_journal_members_v1(backend, 32, 16, 64)
            .map_err(|error| failure("context-open", error))
    } else {
        Context::open(backend).map_err(|error| failure("context-open", error))
    };
    let mut context = ManuallyDrop::new(opened?);
    let result = exercise(&mut context, &options, submit);
    match result {
        Ok(report) => {
            let mut backend = ManuallyDrop::into_inner(context)
                .shutdown()
                .map_err(|error| failure("logical-shutdown", error))?;
            backend
                .shutdown_native_v1()
                .map_err(|error| failure("native-shutdown", error))?;
            println!("{report}");
            Ok(())
        }
        Err(error) => {
            // Never discard live native custody merely to make diagnostic cleanup succeed.
            let cleanup = context.cleanup();
            eprintln!("xgmi recovery error cleanup: {cleanup:?}");
            Err(error)
        }
    }
}

fn main() {
    let result = options(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(|options| {
        match options.shape {
            Shape::Scalar => run(options, submit_scalar),
            Shape::Segments => run(options, submit_segments),
        }
    });
    if let Err(error) = result {
        eprintln!("xgmi recovery failed before complete cleanup: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(journal: &str, shape: &str) -> Vec<String> {
        [journal, shape, "0x1", "0x2", "0x3", "0x4"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn exact_cli_accepts_four_modes_and_preserves_uid_order() {
        for journal in ["--journal", "--no-journal"] {
            for shape in ["scalar", "segments"] {
                let parsed = options(&arguments(journal, shape)).unwrap();
                assert_eq!(parsed.ids, [1, 2, 3, 4]);
                assert_eq!(parsed.journal, journal == "--journal");
                assert_eq!(parsed.shape.name(), shape);
            }
        }
    }

    #[test]
    fn invalid_cli_rosters_and_flags_are_rejected() {
        let valid = arguments("--journal", "segments");
        for index in 0..valid.len() {
            let mut missing = valid.clone();
            missing.remove(index);
            assert!(options(&missing).is_err());
        }
        for invalid in [
            "0x0",
            "0x1",
            "1",
            "0X5",
            "0x",
            "0xg",
            "0x10000000000000000",
            "-1",
        ] {
            let mut changed = valid.clone();
            changed[5] = invalid.into();
            assert!(options(&changed).is_err(), "{invalid}");
        }
        assert!(options(&arguments("--semantic", "scalar")).is_err());
        assert!(options(&arguments("--journal", "unknown")).is_err());
        let mut extra = valid;
        extra.push("0x5".into());
        assert!(options(&extra).is_err());
    }

    #[test]
    fn changed_sources_have_full_extent_and_every_byte_changes() {
        let a0 = source_bytes(0, 0);
        let a1 = source_bytes(0, 1);
        let b0 = source_bytes(1, 0);
        assert_eq!(a0.len(), SOURCE_BYTES as usize);
        assert!(
            a0.iter()
                .zip(a1)
                .all(|(first, second)| *first ^ second == 73)
        );
        assert!(
            a0.iter()
                .zip(b0)
                .all(|(first, second)| *first ^ second == 53)
        );
    }

    #[test]
    fn ordered_descriptor_oracle_preserves_holes_and_observes_overlap_order() {
        let source = source_bytes(0, 1);
        let actual = destination_bytes(Shape::Segments, &source, 0xa5);
        let segments = Shape::Segments.descriptors();
        assert_eq!(segments[0], segments[2]);
        let mut reverse = vec![0xa5; DESTINATION_BYTES as usize];
        for segment in segments.iter().rev() {
            let start = (SOURCE_BASE + segment.source_offset) as usize;
            let target = (DESTINATION_BASE + segment.destination_offset) as usize;
            reverse[target..target + segment.byte_len as usize]
                .copy_from_slice(&source[start..start + segment.byte_len as usize]);
        }
        assert_ne!(actual, reverse);
        for (index, byte) in actual.iter().enumerate() {
            let writer = segments.iter().rev().find(|segment| {
                let start = DESTINATION_BASE + segment.destination_offset;
                (start..start + segment.byte_len).contains(&(index as u64))
            });
            let expected = writer.map_or(0xa5, |segment| {
                source[(SOURCE_BASE + segment.source_offset + index as u64
                    - DESTINATION_BASE
                    - segment.destination_offset) as usize]
            });
            assert_eq!(*byte, expected);
        }
        assert!(
            actual[..DESTINATION_BASE as usize]
                .iter()
                .all(|byte| *byte == 0xa5)
        );
        assert!(
            actual[(DESTINATION_BASE + DESTINATION_ENVELOPE) as usize..]
                .iter()
                .all(|byte| *byte == 0xa5)
        );
    }

    #[test]
    fn scalar_oracle_checks_both_sentinels_and_exact_tail() {
        let source = source_bytes(1, 0);
        let output = destination_bytes(Shape::Scalar, &source, 0x6c);
        assert_eq!(
            &output[DESTINATION_BASE as usize..(DESTINATION_BASE + SCALAR_BYTES) as usize],
            &source[SOURCE_BASE as usize..(SOURCE_BASE + SCALAR_BYTES) as usize]
        );
        assert!(
            output[..DESTINATION_BASE as usize]
                .iter()
                .all(|byte| *byte == 0x6c)
        );
        assert!(
            output[(DESTINATION_BASE + SCALAR_BYTES) as usize..]
                .iter()
                .all(|byte| *byte == 0x6c)
        );
    }

    #[test]
    fn length_framed_digest_distinguishes_order_lengths_and_full_guards() {
        assert_ne!(
            digest(&[vec![1], vec![2, 3]]),
            digest(&[vec![1, 2], vec![3]])
        );
        assert_ne!(digest(&[vec![1], vec![2]]), digest(&[vec![2], vec![1]]));
        let mut parts = vec![
            source_bytes(0, 1),
            destination_bytes(Shape::Segments, &source_bytes(0, 1), 0xa5),
            source_bytes(1, 0),
            destination_bytes(Shape::Segments, &source_bytes(1, 0), 0x6c),
        ];
        let original = digest(&parts);
        parts[3][DESTINATION_BYTES as usize - 1] ^= 1;
        assert_ne!(original, digest(&parts));
    }

    #[test]
    fn reports_distinguish_unknown_destination_from_observed_unchanged_bytes() {
        for journal in ["--journal", "--no-journal"] {
            let options = options(&arguments(journal, "segments")).unwrap();
            let line = report(&options, &"0".repeat(64));
            let fields: Vec<_> = line
                .split_whitespace()
                .skip(1)
                .map(|field| field.split_once('=').unwrap())
                .collect();
            let keys: std::collections::BTreeSet<_> = fields.iter().map(|(key, _)| key).collect();
            assert_eq!(keys.len(), fields.len());
            assert!(
                line.contains("a_failure=certified-host-preparation-capacity-before-native-arm")
            );
            assert!(line.contains("kernels=0 modules=0"));
            assert!(line.contains("performance_acceptance=false formal_refinement=false"));
            assert!(line.contains("failed_destination_bytes=full-byte-unchanged"));
            assert!(line.contains(if options.journal {
                "journal_unknown_read=rejected-before-effect journal_unknown_retry=rejected-before-effect"
            } else {
                "journal_unknown_read=not-applicable journal_unknown_retry=not-applicable"
            }));
        }
    }
}
