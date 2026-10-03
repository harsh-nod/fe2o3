//! Two settled sources, one ordered destination, and a prequeued full readback.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_kfd::{GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942ComputeXgmiPacketPlanV1};
use fe2o3_runtime::*;
use sha2::{Digest, Sha256};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
type Receipts = Arc<Mutex<Vec<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>>;
const SOURCE_BASES: [u64; 2] = [17, 29];
const DESTINATION_BASE: u64 = 53;
const HOST_BASE: usize = 19;
const HOST_SUFFIX: usize = 23;
const ROUNDS: usize = 2;
const WAIT: Duration = Duration::from_secs(180);
const TICKS: usize = 100_000;
const DOMAIN: &[u8] = b"fe2o3.destination-peer-segments.full-bytes.v1\0";
const USAGE: &str = "usage: gfx942-runtime-destination-segments-smoke [--dispose-source] <0xsource0> <0xsource1> <0xdestination> <disjoint|overlap|duplicates|packets>";

#[derive(Debug)]
struct NoCompute;

// SAFETY: No artifact or invocation is authorized to execute a kernel.
unsafe impl KfdRuntimeLaunchAuthorityV1 for NoCompute {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Disjoint,
    Overlap,
    Duplicates,
    Packets,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Disjoint => "disjoint",
            Self::Overlap => "overlap",
            Self::Duplicates => "duplicates",
            Self::Packets => "packets",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    ids: [u64; 3],
    case: Case,
    dispose_source: bool,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    let dispose_source = arguments
        .first()
        .is_some_and(|arg| arg == "--dispose-source");
    let arguments = if dispose_source {
        &arguments[1..]
    } else {
        arguments
    };
    if arguments.len() != 4 {
        return Err(USAGE.into());
    }
    let case = match arguments[3].as_str() {
        "disjoint" => Case::Disjoint,
        "overlap" => Case::Overlap,
        "duplicates" => Case::Duplicates,
        "packets" => Case::Packets,
        _ => return Err(USAGE.into()),
    };
    let mut ids = [0; 3];
    for (index, text) in arguments[..3].iter().enumerate() {
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
        ids,
        case,
        dispose_source,
    })
}

struct Layout {
    sources: [usize; 2],
    destination: usize,
    source_envelope: u64,
    destination_envelope: u64,
    lists: [Vec<RuntimePeerCopySegmentV1>; 2],
}

fn descriptors(triples: &[(u64, u64, u64)]) -> Vec<RuntimePeerCopySegmentV1> {
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

fn layout(case: Case) -> Layout {
    let packet = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let lists = match case {
        Case::Disjoint => [
            descriptors(&[(0, 0, 8193), (12288, 10000, 4097)]),
            descriptors(&[(7, 20000, 8195), (15000, 32000, 4099)]),
        ],
        Case::Overlap => [
            descriptors(&[(0, 0, 8193), (12288, 10000, 4097)]),
            descriptors(&[(7, 4096, 8195), (15000, 11000, 4099)]),
        ],
        Case::Duplicates => [
            descriptors(&[
                (0, 0, 4096),
                (4096, 2048, 4096),
                (0, 0, 4096),
                (8192, 3072, 1025),
            ]),
            descriptors(&[
                (7, 1024, 4097),
                (5000, 4096, 2049),
                (7, 1024, 4097),
                (12288, 2048, 513),
            ]),
        ],
        Case::Packets => [
            descriptors(&[(3, 41, packet + 17), (packet + 53, 7, 31)]),
            descriptors(&[(19, packet + 13, packet + 5), (packet + 79, 13, 37)]),
        ],
    };
    if case == Case::Packets {
        Layout {
            sources: [
                (2 * packet + 4096 + 97) as usize,
                (2 * packet + 4096 + 193) as usize,
            ],
            destination: (2 * packet + 4096 + 321) as usize,
            source_envelope: 2 * packet + 2048,
            destination_envelope: 2 * packet + 3072,
            lists,
        }
    } else {
        Layout {
            sources: [65_536 + 97, 65_536 + 193],
            destination: 65_536 + 321,
            source_envelope: 32_768,
            destination_envelope: 49_152,
            lists,
        }
    }
}

fn pattern(index: u64, source: usize, round: usize) -> u8 {
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8 ^ (source as u8).wrapping_mul(53) ^ (round as u8).wrapping_mul(73)
}

fn inputs(layout: &Layout, round: usize) -> [Vec<u8>; 2] {
    std::array::from_fn(|source| {
        (0..layout.sources[source] as u64)
            .map(|index| pattern(index, source, round))
            .collect()
    })
}

fn destination_sentinel(round: usize) -> u8 {
    0xa5 ^ (round as u8).wrapping_mul(29)
}
fn host_sentinel(round: usize) -> u8 {
    0x6c ^ (round as u8).wrapping_mul(41)
}

fn apply_list(
    output: &mut [u8],
    input: &[u8],
    source_base: u64,
    list: &[RuntimePeerCopySegmentV1],
) {
    for segment in list {
        let source = (source_base + segment.source_offset) as usize;
        let destination = (DESTINATION_BASE + segment.destination_offset) as usize;
        let len = segment.byte_len as usize;
        output[destination..destination + len].copy_from_slice(&input[source..source + len]);
    }
}

fn expected(layout: &Layout, inputs: &[Vec<u8>; 2], round: usize) -> (Vec<u8>, Vec<u8>) {
    let mut destination = vec![destination_sentinel(round); layout.destination];
    for (source, &source_base) in SOURCE_BASES.iter().enumerate() {
        apply_list(
            &mut destination,
            &inputs[source],
            source_base,
            &layout.lists[source],
        );
    }
    let mut host = vec![host_sentinel(round); HOST_BASE + layout.destination + HOST_SUFFIX];
    host[HOST_BASE..HOST_BASE + layout.destination].copy_from_slice(&destination);
    (destination, host)
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

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let message = format!("stage={stage} {error:?}");
    eprintln!("destination segments diagnostic: {message}");
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

fn read_exact(
    context: &mut Context,
    allocation: RuntimeAllocationIdV1,
    expected: &[u8],
) -> ResultV1<Vec<u8>> {
    let mut observed = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut observed)
        .map_err(|error| failure("full-readback", error))?;
    if let Some(offset) = observed
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right)
    {
        return Err(failure(
            "full-byte-oracle",
            (offset, observed[offset], expected[offset]),
        ));
    }
    Ok(observed)
}

fn status<A>(
    context: &Context,
    submission: &RuntimeSubmissionV1<A>,
    expected: RuntimeCompletionStatusV1,
) -> ResultV1<()> {
    require(
        context
            .query_submission(submission)
            .map_err(|error| failure("submission-query", error))?
            == expected,
        "exact-submission-status",
    )
}

fn counters(context: &Context, completed: u64, retained: usize) -> ResultV1<()> {
    require(
        !context.is_terminal()
            && context.backend().completed_compute_xgmi_copies_v1() == completed
            && context.backend().retained_compute_xgmi_copies_v1() == retained,
        "native-counters-and-liveness",
    )
}

fn upload(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    data: RuntimeAllocationIdV1,
    contents: &[u8],
) -> ResultV1<()> {
    context
        .write_allocation(host, 0, contents)
        .map_err(|error| failure("upload-host-write", error))?;
    let mut copy = context
        .copy_async(
            stream,
            region(host, RuntimeAccessV1::Read, 0, contents.len() as u64),
            region(data, RuntimeAccessV1::Write, 0, contents.len() as u64),
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
                std::thread::sleep(Duration::from_micros(50))
            }
            value => return Err(failure("upload-status", value)),
        }
    }
    Err(failure("upload-bound", TICKS))
}

fn drive<A>(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    submission: &mut RuntimeSubmissionV1<A>,
    deadline: Instant,
) -> ResultV1<()> {
    for _ in 0..TICKS {
        require(Instant::now() < deadline, "absolute-progress-deadline")?;
        context
            .progress_stream_v1(stream)
            .map_err(|error| failure("one-quantum-progress", error))?;
        match context
            .poll(submission)
            .map_err(|error| failure("completion-poll", error))?
        {
            RuntimePollV1::Succeeded => {
                return status(context, submission, RuntimeCompletionStatusV1::Succeeded);
            }
            RuntimePollV1::Pending => std::thread::sleep(Duration::from_micros(50)),
            value => return Err(failure("completion-status", value)),
        }
    }
    Err(failure("progress-tick-bound", TICKS))
}

fn watch<A>(
    context: &mut Context,
    submission: &RuntimeSubmissionV1<A>,
    receipts: &Receipts,
) -> ResultV1<()> {
    let id = submission.id();
    let receipts = Arc::clone(receipts);
    context
        .on_completion(submission, move |value| {
            receipts
                .lock()
                .expect("receipt mutex poisoned")
                .push((id, value))
        })
        .map_err(|error| failure("completion-registration", error))
}

#[derive(Clone, Copy)]
struct Storage {
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    data: Option<RuntimeAllocationIdV1>,
}

fn exercise(context: &mut Context, options: &Options, layout: &Layout) -> ResultV1<String> {
    require(
        context.devices().len() == 3
            && context
                .devices()
                .iter()
                .all(|device| device.target() == "gfx942:xnack-"),
        "device-roster",
    )?;
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let extents = [layout.sources[0], layout.sources[1], layout.destination];
    let mut storage = Vec::new();
    for (index, extent) in extents.into_iter().enumerate() {
        let stream = context
            .create_stream(devices[index])
            .map_err(|error| failure("setup-stream", error))?;
        let host = context
            .allocate(
                devices[index],
                RuntimeMemoryKindV1::HostVisible,
                extent as u64,
                4096,
            )
            .map_err(|error| failure("setup-host", error))?;
        let data = context
            .allocate(
                devices[index],
                RuntimeMemoryKindV1::DeviceLocal,
                extent as u64,
                4096,
            )
            .map_err(|error| failure("setup-device", error))?;
        storage.push(Storage {
            stream,
            host,
            data: Some(data),
        });
    }
    let peer_stream = context
        .create_stream(devices[2])
        .map_err(|error| failure("peer-stream", error))?;
    let readback_stream = context
        .create_stream(devices[2])
        .map_err(|error| failure("readback-stream", error))?;
    let host_bytes = HOST_BASE + layout.destination + HOST_SUFFIX;
    let output = context
        .allocate(
            devices[2],
            RuntimeMemoryKindV1::HostVisible,
            host_bytes as u64,
            4096,
        )
        .map_err(|error| failure("readback-host", error))?;
    let receipts = Arc::new(Mutex::new(Vec::with_capacity(ROUNDS * 3)));
    let mut original_ids = Vec::with_capacity(ROUNDS * 3);
    let mut digests = Vec::with_capacity(ROUNDS);
    let mut disposed_source = None;
    for round in 0..ROUNDS {
        let input = inputs(layout, round);
        let initial_destination = vec![destination_sentinel(round); layout.destination];
        let (expected_destination, expected_host) = expected(layout, &input, round);
        if storage[0].data.is_none() {
            storage[0].data = Some(
                context
                    .allocate(
                        devices[0],
                        RuntimeMemoryKindV1::DeviceLocal,
                        layout.sources[0] as u64,
                        4096,
                    )
                    .map_err(|error| failure("replacement-source", error))?,
            );
            require(
                storage[0].data != disposed_source,
                "replacement-source-identity",
            )?;
        }
        for (item, bytes) in storage
            .iter()
            .zip([&input[0], &input[1], &initial_destination])
        {
            upload(context, item.stream, item.host, item.data.unwrap(), bytes)?;
            read_exact(context, item.data.unwrap(), bytes)?;
        }
        let initial_host = vec![host_sentinel(round); host_bytes];
        context
            .write_allocation(output, 0, &initial_host)
            .map_err(|error| failure("readback-sentinel", error))?;
        counters(context, (round * 2) as u64, 0)?;
        let sources = [storage[0].data.unwrap(), storage[1].data.unwrap()];
        let destination = storage[2].data.unwrap();
        let mut caller_lists = layout.lists.clone();
        let mut first = context
            .peer_copy_segments(
                peer_stream,
                region(
                    sources[0],
                    RuntimeAccessV1::Read,
                    SOURCE_BASES[0],
                    layout.source_envelope,
                ),
                region(
                    destination,
                    RuntimeAccessV1::Write,
                    DESTINATION_BASE,
                    layout.destination_envelope,
                ),
                &caller_lists[0],
                &[],
            )
            .map_err(|error| failure("first-list-admission", error))?;
        let first_event = context
            .record_event(&first)
            .map_err(|error| failure("first-event", error))?;
        let second = context
            .peer_copy_segments(
                peer_stream,
                region(
                    sources[1],
                    RuntimeAccessV1::Read,
                    SOURCE_BASES[1],
                    layout.source_envelope,
                ),
                region(
                    destination,
                    RuntimeAccessV1::Write,
                    DESTINATION_BASE,
                    layout.destination_envelope,
                ),
                &caller_lists[1],
                &[first_event],
            )
            .map_err(|error| failure("second-list-admission", error))?;
        let second_event = context
            .record_event(&second)
            .map_err(|error| failure("second-event", error))?;
        let mut readback = context
            .copy_async(
                readback_stream,
                region(
                    destination,
                    RuntimeAccessV1::Read,
                    0,
                    layout.destination as u64,
                ),
                region(
                    output,
                    RuntimeAccessV1::Write,
                    HOST_BASE as u64,
                    layout.destination as u64,
                ),
                &[second_event],
            )
            .map_err(|error| failure("full-frame-readback-admission", error))?;
        watch(context, &first, &receipts)?;
        watch(context, &second, &receipts)?;
        watch(context, &readback, &receipts)?;
        original_ids.extend([first.id(), second.id(), readback.id()]);
        status(context, &first, RuntimeCompletionStatusV1::Pending)?;
        status(context, &second, RuntimeCompletionStatusV1::Pending)?;
        status(context, &readback, RuntimeCompletionStatusV1::Pending)?;
        counters(context, (round * 2) as u64, 0)?;
        context
            .release_event(first_event)
            .map_err(|error| failure("first-event-release", error))?;
        context
            .release_event(second_event)
            .map_err(|error| failure("second-event-release", error))?;
        for list in &mut caller_lists {
            list.fill(RuntimePeerCopySegmentV1 {
                source_offset: u64::MAX,
                destination_offset: u64::MAX,
                byte_len: 0,
            });
        }
        drop(caller_lists);
        let deadline = Instant::now() + WAIT;
        let early_source = if options.dispose_source {
            drive(context, peer_stream, &mut first, deadline)?;
            counters(context, (round * 2 + 1) as u64, 0)?;
            status(context, &second, RuntimeCompletionStatusV1::Pending)?;
            status(context, &readback, RuntimeCompletionStatusV1::Pending)?;
            // Only the completed source is inspected. The destination still has a queued writer.
            let snapshot = read_exact(context, sources[0], &input[0])?;
            context
                .release_allocation(sources[0])
                .map_err(|error| failure("settled-first-source-dispose", error))?;
            storage[0].data = None;
            disposed_source = Some(sources[0]);
            status(context, &first, RuntimeCompletionStatusV1::Succeeded)?;
            status(context, &second, RuntimeCompletionStatusV1::Pending)?;
            Some(snapshot)
        } else {
            None
        };
        drive(context, readback_stream, &mut readback, deadline)?;
        status(context, &first, RuntimeCompletionStatusV1::Succeeded)?;
        status(context, &second, RuntimeCompletionStatusV1::Succeeded)?;
        status(context, &readback, RuntimeCompletionStatusV1::Succeeded)?;
        counters(context, (round * 2 + 2) as u64, 0)?;
        let source0 = match early_source {
            Some(snapshot) => snapshot,
            None => read_exact(context, sources[0], &input[0])?,
        };
        let snapshots = vec![
            source0,
            read_exact(context, sources[1], &input[1])?,
            read_exact(context, destination, &expected_destination)?,
            read_exact(context, output, &expected_host)?,
        ];
        digests.push(digest(&snapshots));
        let actual = receipts
            .lock()
            .map_err(|error| failure("receipt-lock", error))?;
        require(
            actual.len() == original_ids.len()
                && original_ids.iter().all(|id| {
                    actual
                        .iter()
                        .filter(|(observed, value)| {
                            observed == id && *value == RuntimeCompletionStatusV1::Succeeded
                        })
                        .count()
                        == 1
                }),
            "exact-successful-callbacks",
        )?;
        drop(actual);
        require(
            context
                .drain(&mut readback, deadline)
                .map_err(|error| failure("completed-tail-drain", error))?
                == RuntimePollV1::Succeeded,
            "completed-tail-drain-status",
        )?;
        context
            .release_submission(readback)
            .map_err(|error| failure("readback-release", error))?;
        context
            .release_submission(second)
            .map_err(|error| failure("second-list-release", error))?;
        context
            .release_submission(first)
            .map_err(|error| failure("first-list-release", error))?;
        for stream in [peer_stream, readback_stream] {
            require(
                context
                    .query_stream(stream)
                    .map_err(|error| failure("settled-stream-query", error))?
                    == RuntimeStreamObservationV1::default(),
                "all-round-results-released",
            )?;
        }
    }
    require(
        context.completion_callback_panic_count() == 0,
        "callbacks-no-panic",
    )?;
    require(
        digests.len() == ROUNDS && digests[0] != digests[1],
        "changed-round-digests",
    )?;
    context
        .release_allocation(output)
        .map_err(|error| failure("readback-host-release", error))?;
    for item in storage {
        if let Some(data) = item.data {
            context
                .release_allocation(data)
                .map_err(|error| failure("device-release", error))?;
        }
        context
            .release_allocation(item.host)
            .map_err(|error| failure("host-release", error))?;
        context
            .destroy_stream(item.stream)
            .map_err(|error| failure("setup-stream-release", error))?;
    }
    context
        .destroy_stream(readback_stream)
        .map_err(|error| failure("readback-stream-release", error))?;
    context
        .destroy_stream(peer_stream)
        .map_err(|error| failure("peer-stream-release", error))?;
    counters(context, 4, 0)?;
    Ok(report(options, layout, &digests))
}

fn report(options: &Options, layout: &Layout, digests: &[String]) -> String {
    let packet_counts: [usize; 2] = std::array::from_fn(|index| {
        layout.lists[index]
            .iter()
            .map(|segment| {
                Gfx942ComputeXgmiPacketPlanV1::new(segment.byte_len)
                    .expect("bounded witness descriptor")
                    .count()
            })
            .sum()
    });
    let copied: [u64; 2] = std::array::from_fn(|index| {
        layout.lists[index]
            .iter()
            .map(|segment| segment.byte_len)
            .sum()
    });
    format!(
        "PASS schema=fe2o3.destination-peer-segments.v1 authority=production-deny-all transport=NATIVE-XGMI devices=3 unique_ids=0x{:016x},0x{:016x},0x{:016x} case={} source_disposal={} contexts=1 rounds=2 kernels=0 modules=0 source_bytes={},{} destination_bytes={} source_offsets=17,29 destination_offset=53 source_envelope={} destination_envelope={} descriptors_per_round={},{} copied_bytes_per_round={},{} packets_per_round={},{} host_offset=19 host_suffix=23 host_bytes={} lists=4 d2h_copies=2 native_counter=0,2,4 completion_receipts=6 callbacks=exact-original-ids-once-successful admission=all-three-before-progress dependency=exact-latest-predecessor-event events=released-before-progress descriptor_snapshot=caller-overwritten-after-admission progress={} source_disposal_native_counts={} first_source_checks={} sources=full-byte-pass destination=full-byte-pass initialized_complement=full-byte-pass host_guards=full-byte-pass digest=domain-and-u64le-length-prefixed-S0-S1-D-host round_sha256={},{} payloads_changed=true first_source_allocation={} remaining_allocations=reused results=released-readback-second-first-before-refresh drain=completed-tail-only cleanup=logical-and-native-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
        options.ids[0],
        options.ids[1],
        options.ids[2],
        options.case.name(),
        options.dispose_source,
        layout.sources[0],
        layout.sources[1],
        layout.destination,
        layout.source_envelope,
        layout.destination_envelope,
        layout.lists[0].len(),
        layout.lists[1].len(),
        copied[0],
        copied[1],
        packet_counts[0],
        packet_counts[1],
        HOST_BASE + layout.destination + HOST_SUFFIX,
        if options.dispose_source {
            "first-list-settlement-then-final-readback-only"
        } else {
            "final-readback-only"
        },
        if options.dispose_source {
            "1,3"
        } else {
            "not-applicable"
        },
        if options.dispose_source {
            "full-after-first-before-disposal"
        } else {
            "full-after-tail"
        },
        digests[0],
        digests[1],
        if options.dispose_source {
            "disposed-after-first-replaced-next-round"
        } else {
            "reused"
        }
    )
}

fn run(options: Options) -> ResultV1<()> {
    let layout = layout(options.case);
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
    let context = Context::open_with_version_journal_members_v1(backend, 32, 16, 64)
        .map_err(|error| failure("context-open", error))?;
    let mut context = ManuallyDrop::new(context);
    match exercise(&mut context, &options, &layout) {
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
            let cleanup = context.cleanup();
            eprintln!("destination segments error cleanup: {cleanup:?}");
            Err(error)
        }
    }
}

fn main() {
    if let Err(error) = options(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(run) {
        eprintln!("destination segments failed before complete cleanup: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(case: &str, dispose: bool) -> Vec<String> {
        let mut result = Vec::new();
        if dispose {
            result.push("--dispose-source".into());
        }
        result.extend(["0x1", "0x2", "0x3", case].map(str::to_owned));
        result
    }

    #[test]
    fn cli_accepts_all_shapes_and_both_source_lifetimes() {
        for case in ["disjoint", "overlap", "duplicates", "packets"] {
            for dispose in [false, true] {
                let parsed = options(&arguments(case, dispose)).unwrap();
                assert_eq!(parsed.ids, [1, 2, 3]);
                assert_eq!(parsed.case.name(), case);
                assert_eq!(parsed.dispose_source, dispose);
            }
        }
    }

    #[test]
    fn cli_rejects_zero_duplicate_overflow_and_malformed_uids_or_modes() {
        let valid = arguments("overlap", false);
        for index in 0..4 {
            let mut missing = valid.clone();
            missing.remove(index);
            assert!(options(&missing).is_err());
        }
        for invalid in [
            "0x0",
            "0x1",
            "3",
            "0X3",
            "0x",
            "0xz",
            "0x10000000000000000",
            "--dispose-source",
        ] {
            let mut changed = valid.clone();
            changed[2] = invalid.into();
            assert!(options(&changed).is_err(), "{invalid}");
        }
        assert!(options(&arguments("unknown", false)).is_err());
        let mut repeated = arguments("packets", true);
        repeated.insert(0, "--dispose-source".into());
        assert!(options(&repeated).is_err());
    }

    #[test]
    fn every_descriptor_fits_independent_logical_envelopes_and_exact_packet_tails() {
        for case in [
            Case::Disjoint,
            Case::Overlap,
            Case::Duplicates,
            Case::Packets,
        ] {
            let layout = layout(case);
            for (source, &source_base) in SOURCE_BASES.iter().enumerate() {
                assert!(source_base + layout.source_envelope < layout.sources[source] as u64);
                assert!(DESTINATION_BASE + layout.destination_envelope < layout.destination as u64);
                for segment in &layout.lists[source] {
                    assert!(segment.byte_len > 0);
                    assert!(segment.source_offset + segment.byte_len <= layout.source_envelope);
                    assert!(
                        segment.destination_offset + segment.byte_len
                            <= layout.destination_envelope
                    );
                    let plan = Gfx942ComputeXgmiPacketPlanV1::new(segment.byte_len).unwrap();
                    assert_eq!(
                        (0..plan.count())
                            .map(|index| plan.packet(index).unwrap().bytes as u64)
                            .sum::<u64>(),
                        segment.byte_len
                    );
                }
            }
            if case == Case::Packets {
                assert_eq!(
                    Gfx942ComputeXgmiPacketPlanV1::new(layout.lists[0][0].byte_len)
                        .unwrap()
                        .packet(1)
                        .unwrap()
                        .bytes,
                    17
                );
                assert_eq!(
                    Gfx942ComputeXgmiPacketPlanV1::new(layout.lists[1][0].byte_len)
                        .unwrap()
                        .packet(1)
                        .unwrap()
                        .bytes,
                    5
                );
            }
        }
    }

    #[test]
    fn complete_oracle_matches_independent_last_writer_lookup_and_preserves_guards() {
        for case in [Case::Disjoint, Case::Overlap, Case::Duplicates] {
            let layout = layout(case);
            for round in 0..ROUNDS {
                let input = inputs(&layout, round);
                let (output, host) = expected(&layout, &input, round);
                for (index, byte) in output.iter().enumerate() {
                    let mut wanted = destination_sentinel(round);
                    'latest: for (source, &source_base) in SOURCE_BASES.iter().enumerate().rev() {
                        for segment in layout.lists[source].iter().rev() {
                            let start = DESTINATION_BASE + segment.destination_offset;
                            if (start..start + segment.byte_len).contains(&(index as u64)) {
                                wanted = input[source][(source_base
                                    + segment.source_offset
                                    + index as u64
                                    - start)
                                    as usize];
                                break 'latest;
                            }
                        }
                    }
                    assert_eq!(*byte, wanted);
                }
                assert!(
                    output[..DESTINATION_BASE as usize]
                        .iter()
                        .all(|byte| *byte == destination_sentinel(round))
                );
                assert!(
                    output[(DESTINATION_BASE + layout.destination_envelope) as usize..]
                        .iter()
                        .all(|byte| *byte == destination_sentinel(round))
                );
                assert_eq!(&host[HOST_BASE..HOST_BASE + output.len()], output);
                assert!(
                    host[..HOST_BASE]
                        .iter()
                        .chain(&host[HOST_BASE + output.len()..])
                        .all(|byte| *byte == host_sentinel(round))
                );
            }
        }
    }

    #[test]
    fn overlaps_and_duplicates_observe_both_descriptor_and_list_order() {
        let layout = layout(Case::Duplicates);
        assert_eq!(layout.lists[0][0], layout.lists[0][2]);
        assert_eq!(layout.lists[1][0], layout.lists[1][2]);
        let input = inputs(&layout, 0);
        let expected = expected(&layout, &input, 0).0;
        let mut reversed = vec![destination_sentinel(0); layout.destination];
        for (source, &source_base) in SOURCE_BASES.iter().enumerate().rev() {
            apply_list(
                &mut reversed,
                &input[source],
                source_base,
                &layout.lists[source],
            );
        }
        assert_ne!(expected, reversed);
        let mut reversed = vec![destination_sentinel(0); layout.destination];
        for (source, &source_base) in SOURCE_BASES.iter().enumerate() {
            let mut list = layout.lists[source].clone();
            list.reverse();
            apply_list(&mut reversed, &input[source], source_base, &list);
        }
        assert_ne!(expected, reversed);
    }

    #[test]
    fn changed_payloads_and_length_framing_cover_complete_original_owners() {
        let layout = layout(Case::Overlap);
        let before = inputs(&layout, 0);
        let after = inputs(&layout, 1);
        for (first, second) in before.iter().zip(&after) {
            assert!(
                first
                    .iter()
                    .zip(second)
                    .all(|(left, right)| left ^ right == 73)
            );
        }
        assert_ne!(
            digest(&[vec![1], vec![2, 3]]),
            digest(&[vec![1, 2], vec![3]])
        );
        assert_ne!(digest(&[vec![1], vec![2]]), digest(&[vec![2], vec![1]]));
        let (destination, host) = expected(&layout, &before, 0);
        let mut parts = vec![before[0].clone(), before[1].clone(), destination, host];
        let hash = digest(&parts);
        *parts[3].last_mut().unwrap() ^= 1;
        assert_ne!(hash, digest(&parts));
    }

    #[test]
    fn exact_report_schema_distinguishes_seeded_disposal_from_final_only_progress() {
        for dispose in [false, true] {
            let options = options(&arguments("disjoint", dispose)).unwrap();
            let report = report(
                &options,
                &layout(options.case),
                &["0".repeat(64), "1".repeat(64)],
            );
            let fields: Vec<_> = report
                .split_whitespace()
                .skip(1)
                .map(|field| field.split_once('=').unwrap())
                .collect();
            let keys: std::collections::BTreeSet<_> = fields.iter().map(|(key, _)| key).collect();
            assert_eq!(fields.len(), keys.len());
            assert!(report.contains("admission=all-three-before-progress"));
            assert!(report.contains("native_counter=0,2,4 completion_receipts=6"));
            assert!(report.contains(if dispose {
                "progress=first-list-settlement-then-final-readback-only"
            } else {
                "progress=final-readback-only"
            }));
            assert!(report.contains("performance_acceptance=false formal_refinement=false"));
        }
    }
}
