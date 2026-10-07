//! Settled source lists -> pending preserved frame -> peer window/list -> D2H.

use super::*;

const FRAME_OFFSET: usize = 7;
const FRAME_SUFFIX: usize = 7;
const TARGET_OFFSET: usize = 37;
const TARGET_EXTRA: usize = 173;
const FORWARD_DOMAIN: &[u8] = b"fe2o3.segment-frame-peer-window.full-bytes.v1\0";
const SEGMENTS_DOMAIN: &[u8] = b"fe2o3.segment-frame-peer-segments.full-bytes.v1\0";

#[derive(Clone, Copy, Debug)]
struct Window {
    source: usize,
    frame: usize,
    target: usize,
    bytes: usize,
    host: usize,
}

impl Window {
    fn new(layout: &Layout) -> Self {
        let bytes = layout.destination - FRAME_OFFSET - FRAME_SUFFIX;
        Self {
            source: *layout.sources.iter().max().expect("two source extents"),
            frame: layout.destination,
            target: layout.destination + TARGET_EXTRA,
            bytes,
            host: HOST_BASE + layout.destination + TARGET_EXTRA + HOST_SUFFIX,
        }
    }
}

fn target_sentinel(round: usize) -> u8 {
    0xd3 ^ (round as u8).wrapping_mul(31)
}

fn source_bytes(window: Window, round: usize) -> Vec<u8> {
    (0..window.source as u64)
        .map(|index| pattern(index, 0, round))
        .collect()
}

fn forward_segments(window: Window) -> Vec<RuntimePeerCopySegmentV1> {
    descriptors(&[
        (0, 5, window.bytes as u64 - 17),
        (window.bytes as u64 - 31, 1, 23),
        (192, 14, 9),
        (192, 14, 9),
    ])
}

fn oracle(
    layout: &Layout,
    window: Window,
    source: &[u8],
    round: usize,
    segmented: bool,
) -> [Vec<u8>; 3] {
    let mut frame = vec![destination_sentinel(round); window.frame];
    for (list, source_base) in layout.lists.iter().zip(SOURCE_BASES) {
        apply_list(&mut frame, source, source_base, list);
    }
    let mut target = vec![target_sentinel(round); window.target];
    if segmented {
        for segment in forward_segments(window) {
            let src = FRAME_OFFSET + segment.source_offset as usize;
            let dst = TARGET_OFFSET + segment.destination_offset as usize;
            let len = segment.byte_len as usize;
            target[dst..dst + len].copy_from_slice(&frame[src..src + len]);
        }
    } else {
        target[TARGET_OFFSET..TARGET_OFFSET + window.bytes]
            .copy_from_slice(&frame[FRAME_OFFSET..FRAME_OFFSET + window.bytes]);
    }
    let mut host = vec![host_sentinel(round); window.host];
    host[HOST_BASE..HOST_BASE + window.target].copy_from_slice(&target);
    [frame, target, host]
}

fn forward_digest(parts: &[Vec<u8>], segmented: bool) -> String {
    let mut hash = Sha256::new();
    hash.update(if segmented {
        SEGMENTS_DOMAIN
    } else {
        FORWARD_DOMAIN
    });
    for bytes in parts {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn tail_published<A>(
    context: &Context,
    tail: &RuntimeSubmissionV1<A>,
    completed: u64,
) -> ResultV1<bool> {
    status(context, tail, RuntimeCompletionStatusV1::Pending)?;
    require(!context.is_terminal(), "forward-publication-nonterminal")?;
    let actual = context.backend().completed_compute_xgmi_copies_v1();
    require(actual <= completed, "forward-publication-not-missed")?;
    Ok(actual == completed && context.backend().retained_compute_xgmi_copies_v1() == 1)
}

fn seed_tail<A>(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    tail: &RuntimeSubmissionV1<A>,
    completed: u64,
    deadline: Instant,
) -> ResultV1<()> {
    for _ in 0..TICKS {
        if tail_published(context, tail, completed)? {
            return Ok(());
        }
        require(Instant::now() < deadline, "forward-publication-deadline")?;
        context
            .progress_stream_v1(stream)
            .map_err(|error| failure("forward-seed-one-quantum", error))?;
        if !tail_published(context, tail, completed)? {
            std::thread::sleep(Duration::from_micros(50));
        }
    }
    Err(failure("forward-publication-tick-bound", TICKS))
}

pub(super) fn exercise(
    context: &mut Context,
    options: &Options,
    layout: &Layout,
    late: bool,
) -> ResultV1<String> {
    if options.forward_segments {
        exercise_profile(
            context,
            options,
            layout,
            late,
            |context, stream, source, destination, segments, events| {
                context.peer_copy_segments(stream, source, destination, segments, events)
            },
        )
    } else {
        exercise_profile(
            context,
            options,
            layout,
            late,
            |context, stream, source, destination, _, events| {
                context.peer_copy(stream, source, destination, events)
            },
        )
    }
}

fn exercise_profile<M>(
    context: &mut Context,
    options: &Options,
    layout: &Layout,
    late: bool,
    submit: impl Fn(
        &mut Context,
        RuntimeStreamIdV1,
        RuntimeMemoryRegionV1,
        RuntimeMemoryRegionV1,
        &[RuntimePeerCopySegmentV1],
        &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<M>, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
) -> ResultV1<String> {
    require(
        context.devices().len() == 3
            && context
                .devices()
                .iter()
                .all(|device| device.target() == "gfx942:xnack-"),
        "forward-device-roster",
    )?;
    let window = Window::new(layout);
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut storage = Vec::with_capacity(3);
    for (index, bytes) in [window.source, window.frame, window.target]
        .into_iter()
        .enumerate()
    {
        let stream = context
            .create_stream(devices[index])
            .map_err(|error| failure("forward-setup-stream", error))?;
        let host = context
            .allocate(
                devices[index],
                RuntimeMemoryKindV1::HostVisible,
                bytes as u64,
                4096,
            )
            .map_err(|error| failure("forward-setup-host", error))?;
        let data = context
            .allocate(
                devices[index],
                RuntimeMemoryKindV1::DeviceLocal,
                bytes as u64,
                4096,
            )
            .map_err(|error| failure("forward-setup-device", error))?;
        storage.push(Storage {
            stream,
            host,
            data: Some(data),
        });
    }
    let list_stream = context
        .create_stream(devices[1])
        .map_err(|error| failure("forward-list-stream", error))?;
    let peer_stream = context
        .create_stream(devices[2])
        .map_err(|error| failure("forward-peer-stream", error))?;
    let readback_stream = context
        .create_stream(devices[2])
        .map_err(|error| failure("forward-readback-stream", error))?;
    let host = context
        .allocate(
            devices[2],
            RuntimeMemoryKindV1::HostVisible,
            window.host as u64,
            4096,
        )
        .map_err(|error| failure("forward-output-host", error))?;
    let source = storage[0].data.unwrap();
    let frame = storage[1].data.unwrap();
    let target = storage[2].data.unwrap();
    let receipts = Arc::new(Mutex::new(Vec::with_capacity(ROUNDS * 4)));
    let mut original_ids = Vec::with_capacity(ROUNDS * 4);
    let mut digests = Vec::with_capacity(ROUNDS);
    for round in 0..ROUNDS {
        let source_initial = source_bytes(window, round);
        let frame_initial = vec![destination_sentinel(round); window.frame];
        let target_initial = vec![target_sentinel(round); window.target];
        let wanted = oracle(
            layout,
            window,
            &source_initial,
            round,
            options.forward_segments,
        );
        for (item, bytes) in storage
            .iter()
            .zip([&source_initial, &frame_initial, &target_initial])
        {
            upload(context, item.stream, item.host, item.data.unwrap(), bytes)?;
            read_exact(context, item.data.unwrap(), bytes)?;
        }
        context
            .write_allocation(host, 0, &vec![host_sentinel(round); window.host])
            .map_err(|error| failure("forward-host-sentinel", error))?;
        let completed = (round * 3) as u64;
        counters(context, completed, 0)?;
        let mut caller_lists = layout.lists.clone();
        let first = context
            .peer_copy_segments(
                list_stream,
                region(
                    source,
                    RuntimeAccessV1::Read,
                    SOURCE_BASES[0],
                    layout.source_envelope,
                ),
                region(
                    frame,
                    RuntimeAccessV1::Write,
                    DESTINATION_BASE,
                    layout.destination_envelope,
                ),
                &caller_lists[0],
                &[],
            )
            .map_err(|error| failure("forward-first-list-admission", error))?;
        let first_event = context
            .record_event(&first)
            .map_err(|error| failure("forward-first-event", error))?;
        let second = context
            .peer_copy_segments(
                list_stream,
                region(
                    source,
                    RuntimeAccessV1::Read,
                    SOURCE_BASES[1],
                    layout.source_envelope,
                ),
                region(
                    frame,
                    RuntimeAccessV1::Write,
                    DESTINATION_BASE,
                    layout.destination_envelope,
                ),
                &caller_lists[1],
                &[first_event],
            )
            .map_err(|error| failure("forward-second-list-admission", error))?;
        context
            .release_event(first_event)
            .map_err(|error| failure("forward-first-event-release", error))?;
        let second_event = context
            .record_event(&second)
            .map_err(|error| failure("forward-second-event", error))?;
        watch(context, &first, &receipts)?;
        watch(context, &second, &receipts)?;
        status(context, &first, RuntimeCompletionStatusV1::Pending)?;
        status(context, &second, RuntimeCompletionStatusV1::Pending)?;
        counters(context, completed, 0)?;
        for list in &mut caller_lists {
            list.fill(RuntimePeerCopySegmentV1 {
                source_offset: u64::MAX,
                destination_offset: u64::MAX,
                byte_len: 0,
            });
        }
        drop(caller_lists);
        let deadline = Instant::now() + WAIT;
        if late {
            seed_tail(context, list_stream, &second, completed + 1, deadline)?;
            require(
                tail_published(context, &second, completed + 1)?,
                "forward-publication-before-admission",
            )?;
        }
        let mut caller_segments = forward_segments(window);
        let peer = submit(
            context,
            peer_stream,
            region(
                frame,
                RuntimeAccessV1::Read,
                FRAME_OFFSET as u64,
                window.bytes as u64,
            ),
            region(
                target,
                RuntimeAccessV1::Write,
                TARGET_OFFSET as u64,
                if options.forward_segments {
                    (window.target - TARGET_OFFSET - FRAME_SUFFIX) as u64
                } else {
                    window.bytes as u64
                },
            ),
            &caller_segments,
            &[second_event],
        )
        .map_err(|error| failure("forward-peer-admission", error))?;
        caller_segments.fill(RuntimePeerCopySegmentV1 {
            source_offset: u64::MAX,
            destination_offset: u64::MAX,
            byte_len: 0,
        });
        drop(caller_segments);
        if late {
            require(
                tail_published(context, &second, completed + 1)?,
                "forward-publication-after-admission",
            )?;
        }
        context
            .release_event(second_event)
            .map_err(|error| failure("forward-second-event-release", error))?;
        let peer_event = context
            .record_event(&peer)
            .map_err(|error| failure("forward-scalar-event", error))?;
        let mut readback = context
            .copy_async(
                readback_stream,
                region(target, RuntimeAccessV1::Read, 0, window.target as u64),
                region(
                    host,
                    RuntimeAccessV1::Write,
                    HOST_BASE as u64,
                    window.target as u64,
                ),
                &[peer_event],
            )
            .map_err(|error| failure("forward-readback-admission", error))?;
        context
            .release_event(peer_event)
            .map_err(|error| failure("forward-scalar-event-release", error))?;
        watch(context, &peer, &receipts)?;
        watch(context, &readback, &receipts)?;
        original_ids.extend([first.id(), second.id(), peer.id(), readback.id()]);
        status(context, &second, RuntimeCompletionStatusV1::Pending)?;
        status(context, &peer, RuntimeCompletionStatusV1::Pending)?;
        status(context, &readback, RuntimeCompletionStatusV1::Pending)?;
        if late {
            counters(context, completed + 1, 1)?;
        } else {
            status(context, &first, RuntimeCompletionStatusV1::Pending)?;
            counters(context, completed, 0)?;
        }
        drive(context, readback_stream, &mut readback, deadline)?;
        status(context, &first, RuntimeCompletionStatusV1::Succeeded)?;
        status(context, &second, RuntimeCompletionStatusV1::Succeeded)?;
        status(context, &peer, RuntimeCompletionStatusV1::Succeeded)?;
        counters(context, completed + 3, 0)?;
        let snapshots = vec![
            read_exact(context, source, &source_initial)?,
            read_exact(context, frame, &wanted[0])?,
            read_exact(context, target, &wanted[1])?,
            read_exact(context, host, &wanted[2])?,
        ];
        digests.push(forward_digest(&snapshots, options.forward_segments));
        let actual = receipts
            .lock()
            .map_err(|error| failure("forward-receipt-lock", error))?;
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
            "forward-exact-successful-callbacks",
        )?;
        drop(actual);
        require(
            context
                .drain(&mut readback, deadline)
                .map_err(|error| failure("forward-completed-drain", error))?
                == RuntimePollV1::Succeeded,
            "forward-completed-drain-status",
        )?;
        context
            .release_submission(readback)
            .map_err(|error| failure("forward-readback-release", error))?;
        context
            .release_submission(peer)
            .map_err(|error| failure("forward-scalar-release", error))?;
        context
            .release_submission(second)
            .map_err(|error| failure("forward-second-release", error))?;
        context
            .release_submission(first)
            .map_err(|error| failure("forward-first-release", error))?;
        for stream in [list_stream, peer_stream, readback_stream] {
            require(
                context
                    .query_stream(stream)
                    .map_err(|error| failure("forward-stream-query", error))?
                    == RuntimeStreamObservationV1::default(),
                "forward-results-released-before-refresh",
            )?;
        }
    }
    require(
        context.completion_callback_panic_count() == 0,
        "forward-callbacks-no-panic",
    )?;
    require(
        digests.len() == ROUNDS && digests[0] != digests[1],
        "forward-rounds-changed",
    )?;
    context
        .release_allocation(host)
        .map_err(|error| failure("forward-host-release", error))?;
    for item in storage {
        context
            .release_allocation(item.data.unwrap())
            .map_err(|error| failure("forward-device-release", error))?;
        context
            .release_allocation(item.host)
            .map_err(|error| failure("forward-upload-host-release", error))?;
        context
            .destroy_stream(item.stream)
            .map_err(|error| failure("forward-upload-stream-release", error))?;
    }
    for stream in [readback_stream, peer_stream, list_stream] {
        context
            .destroy_stream(stream)
            .map_err(|error| failure("forward-stream-release", error))?;
    }
    counters(context, 6, 0)?;
    Ok(report(options, layout, window, late, &digests))
}

fn report(
    options: &Options,
    layout: &Layout,
    window: Window,
    late: bool,
    digests: &[String],
) -> String {
    let packets: [usize; 2] = std::array::from_fn(|index| {
        layout.lists[index]
            .iter()
            .map(|segment| {
                Gfx942ComputeXgmiPacketPlanV1::new(segment.byte_len)
                    .expect("bounded list descriptor")
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
    let segmented = options.forward_segments;
    let peer_packets = if segmented {
        forward_segments(window)
            .iter()
            .map(|segment| {
                Gfx942ComputeXgmiPacketPlanV1::new(segment.byte_len)
                    .unwrap()
                    .count()
            })
            .sum()
    } else {
        Gfx942ComputeXgmiPacketPlanV1::new(window.bytes as u64)
            .unwrap()
            .count()
    };
    let schema = if segmented {
        "fe2o3.segment-frame-peer-segments.v1"
    } else {
        "fe2o3.segment-frame-peer-window.v1"
    };
    let lists = if segmented { 6 } else { 4 };
    let scalar_peers = if segmented { 0 } else { 2 };
    let dependency = if segmented {
        "exact-latest-list-events"
    } else {
        "exact-latest-list-and-scalar-events"
    };
    let results = if segmented {
        "released-readback-third-second-first-before-refresh"
    } else {
        "released-readback-scalar-second-first-before-refresh"
    };
    let extra = if segmented {
        format!(
            " peer_descriptors=4 peer_target_envelope={} peer_copied_bytes={}",
            window.target - TARGET_OFFSET - FRAME_SUFFIX,
            forward_segments(window)
                .iter()
                .map(|segment| segment.byte_len)
                .sum::<u64>()
        )
    } else {
        String::new()
    };
    format!(
        "PASS schema={schema} authority=production-deny-all transport=NATIVE-XGMI devices=3 unique_ids=0x{:016x},0x{:016x},0x{:016x} case={} late={} contexts=1 rounds=2 kernels=0 modules=0 allocations=7 streams=6 source_bytes={} frame_bytes={} target_bytes={} list_source_offsets=17,29 list_destination_offset=53 source_envelope={} destination_envelope={} descriptors_per_round={},{} list_copied_bytes_per_round={},{} packets_per_round={},{},{} peer_source_offset=7 peer_target_offset=37 peer_bytes={} readback_source_offset=0 readback_bytes={} host_offset=19 host_suffix=23 host_bytes={} lists={lists} scalar_peers={scalar_peers} d2h_copies=2 native_counter=0,3,6 completion_receipts=8 callbacks=exact-original-ids-once-successful admission={} dependency={dependency} events=released-after-dependent-admission descriptor_snapshot=caller-overwritten-after-admission progress={} publication_observed={} publication_identity={} native_at_peer_admission={} retained_at_peer_admission={} source=full-byte-pass frame=full-byte-pass initialized_complement=full-byte-pass target_guards=full-byte-pass host_guards=full-byte-pass digest=domain-and-u64le-length-prefixed-source-frame-target-host round_sha256={},{} payloads_changed=true allocations_reused=true results={results} source_disposal=after-all-rounds-settled drain=completed-tail-only cleanup=logical-and-native-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false{extra}",
        options.ids[0],
        options.ids[1],
        options.ids[2],
        options.case.name(),
        late,
        window.source,
        window.frame,
        window.target,
        layout.source_envelope,
        layout.destination_envelope,
        layout.lists[0].len(),
        layout.lists[1].len(),
        copied[0],
        copied[1],
        packets[0],
        packets[1],
        peer_packets,
        window.bytes,
        window.target,
        window.host,
        if late {
            if segmented {
                "parents-before-seed-list-after-latest-publication"
            } else {
                "lists-before-seed-scalar-after-latest-publication"
            }
        } else {
            "all-four-before-progress"
        },
        if late {
            "latest-list-seed-then-final-readback-only"
        } else {
            "final-readback-only"
        },
        late,
        if late {
            "ordered-roster-inference"
        } else {
            "not-sampled"
        },
        if late { "1,4" } else { "0,3" },
        if late { "1,1" } else { "0,0" },
        digests[0],
        digests[1],
    )
}

#[cfg(test)]
#[path = "forward_window/tests.rs"]
mod tests;
