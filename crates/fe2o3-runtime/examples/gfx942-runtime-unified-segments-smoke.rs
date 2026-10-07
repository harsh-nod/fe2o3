//! Native ordered-list qualification on the unified compute/XGMI backend.
//! The runner must admit idle endpoints; this does not reserve GPUs or benchmark them.

#![forbid(unsafe_code)]

use fe2o3_kfd::{GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1, Gfx942ComputeXgmiPacketPlanV1};
use fe2o3_runtime::*;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

type ContextV1 = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const WAIT: Duration = Duration::from_secs(180);
const SOURCE_BASE: u64 = 17;
const DESTINATION_BASE: u64 = 53;

#[derive(Debug, Eq, PartialEq)]
struct Options {
    ids: [u64; 2],
    case: String,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    if arguments.len() != 3
        || !matches!(arguments[2].as_str(), "1" | "4" | "65" | "4096" | "packets")
    {
        return Err("expected <0xsource-id> <0xdestination-id> <1|4|65|4096|packets>".into());
    }
    let mut ids = [0; 2];
    for (id, text) in ids.iter_mut().zip(arguments) {
        let hex = text
            .strip_prefix("0x")
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or("hexadecimal unique ID required")?;
        *id = u64::from_str_radix(hex, 16).map_err(|_| "invalid unique ID")?;
        if *id == 0 {
            return Err("nonzero unique ID required".into());
        }
    }
    if ids[0] == ids[1] {
        return Err("distinct GPUs required".into());
    }
    Ok(Options {
        ids,
        case: arguments[2].clone(),
    })
}

struct Layout {
    source_bytes: usize,
    destination_bytes: usize,
    source_len: u64,
    destination_len: u64,
    segments: Vec<RuntimePeerCopySegmentV1>,
}

fn layout(case: &str) -> Layout {
    let packet = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    if case == "packets" {
        return Layout {
            source_bytes: (2 * packet + 4096 + 97) as usize,
            destination_bytes: (2 * packet + 4096 + 321) as usize,
            source_len: 2 * packet + 2048,
            destination_len: 2 * packet + 3072,
            segments: vec![
                RuntimePeerCopySegmentV1 {
                    source_offset: 3,
                    destination_offset: 41,
                    byte_len: packet + 17,
                },
                RuntimePeerCopySegmentV1 {
                    source_offset: 19,
                    destination_offset: packet + 13,
                    byte_len: packet + 5,
                },
                RuntimePeerCopySegmentV1 {
                    source_offset: packet + 53,
                    destination_offset: 7,
                    byte_len: 31,
                },
            ],
        };
    }
    let count: usize = case.parse().expect("validated witness case");
    let first = RuntimePeerCopySegmentV1 {
        source_offset: 0,
        destination_offset: 0,
        byte_len: 64,
    };
    let segments = (0..count)
        .map(|index| match index {
            0 | 2 => first,
            1 => RuntimePeerCopySegmentV1 {
                source_offset: 128,
                destination_offset: 32,
                byte_len: 64,
            },
            _ => RuntimePeerCopySegmentV1 {
                source_offset: 512 + (index as u64 * 13) % 32_000,
                destination_offset: 512 + (index as u64 * 17) % 48_000,
                byte_len: index as u64 % 19 + 1,
            },
        })
        .collect();
    Layout {
        source_bytes: 65_536 + 97,
        destination_bytes: 65_536 + 321,
        source_len: 32_768,
        destination_len: 49_152,
        segments,
    }
}

fn pattern(index: u64) -> u8 {
    let mut value = index.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    (value ^ (value >> 31)) as u8
}

fn expected(layout: &Layout, input: &[u8]) -> Vec<u8> {
    let mut output = vec![0xa5; layout.destination_bytes];
    for segment in &layout.segments {
        let source = (SOURCE_BASE + segment.source_offset) as usize;
        let destination = (DESTINATION_BASE + segment.destination_offset) as usize;
        let bytes = segment.byte_len as usize;
        output[destination..destination + bytes].copy_from_slice(&input[source..source + bytes]);
    }
    output
}

fn error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
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

fn upload(
    context: &mut ContextV1,
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    device: RuntimeAllocationIdV1,
    data: &[u8],
) -> ResultV1<()> {
    context.write_allocation(host, 0, data).map_err(error)?;
    let mut copy = context
        .copy_async(
            stream,
            region(host, RuntimeAccessV1::Read, 0, data.len() as u64),
            region(device, RuntimeAccessV1::Write, 0, data.len() as u64),
            &[],
        )
        .map_err(error)?;
    let deadline = Instant::now() + WAIT;
    loop {
        context.flush_stream(stream).map_err(error)?;
        match context.poll(&mut copy).map_err(error)? {
            RuntimePollV1::Succeeded => break,
            RuntimePollV1::Pending if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_micros(50))
            }
            status => return Err(format!("H2D initialization did not complete: {status:?}")),
        }
    }
    context.release_submission(copy).map_err(error)
}

fn verify(
    context: &mut ContextV1,
    allocation: RuntimeAllocationIdV1,
    expected: &[u8],
) -> ResultV1<()> {
    let mut actual = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut actual)
        .map_err(error)?;
    if actual != expected {
        return Err("full-byte data or guard mismatch".into());
    }
    Ok(())
}

fn run(options: Options) -> ResultV1<()> {
    let layout = layout(&options.case);
    let input: Vec<_> = (0..layout.source_bytes as u64).map(pattern).collect();
    let initial = vec![0xa5; layout.destination_bytes];
    let output = expected(&layout, &input);
    // Reuse the existing finite qualification constructor without loading or launching kernels.
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_peer_qualification_v2(&options.ids)
            .map_err(error)?;
    let mut context =
        RuntimeContextV1::open_with_version_journal_v1(backend, 16, 8).map_err(error)?;
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let mut allocations = Vec::new();
    let mut streams = Vec::new();
    for (index, contents) in [&input, &initial].into_iter().enumerate() {
        let stream = context.create_stream(devices[index]).map_err(error)?;
        let host = context
            .allocate(
                devices[index],
                RuntimeMemoryKindV1::HostVisible,
                contents.len() as u64,
                4096,
            )
            .map_err(error)?;
        let device = context
            .allocate(
                devices[index],
                RuntimeMemoryKindV1::DeviceLocal,
                contents.len() as u64,
                4096,
            )
            .map_err(error)?;
        upload(&mut context, stream, host, device, contents)?;
        verify(&mut context, device, contents)?;
        streams.push(stream);
        allocations.push((host, device));
    }
    let source = region(
        allocations[0].1,
        RuntimeAccessV1::Read,
        SOURCE_BASE,
        layout.source_len,
    );
    let destination = region(
        allocations[1].1,
        RuntimeAccessV1::Write,
        DESTINATION_BASE,
        layout.destination_len,
    );
    let stream = streams[1];
    let mut cancelled = context
        .peer_copy_segments(stream, source, destination, &layout.segments, &[])
        .map_err(error)?;
    if context.cancel(&mut cancelled).map_err(error)? != RuntimeCancellationV1::Cancelled {
        return Err("prepublication cancellation failed".into());
    }
    context.release_submission(cancelled).map_err(error)?;
    verify(&mut context, source.allocation, &input)?;
    verify(&mut context, destination.allocation, &initial)?;
    let mut descriptors = layout.segments.clone();
    let mut copy = context
        .peer_copy_segments(stream, source, destination, &descriptors, &[])
        .map_err(error)?;
    let event = context.record_event(&copy).map_err(error)?;
    descriptors.fill(RuntimePeerCopySegmentV1 {
        source_offset: u64::MAX,
        destination_offset: u64::MAX,
        byte_len: 0,
    });
    if context.poll(&mut copy).map_err(error)? != RuntimePollV1::Pending
        || context.query_event(event).map_err(error)? != RuntimeCompletionStatusV1::Pending
    {
        return Err("unpublished list completed".into());
    }
    for _ in 0..8 {
        context.progress_stream_v1(stream).map_err(error)?;
        if context.backend().retained_compute_xgmi_copies_v1() == 1 {
            break;
        }
    }
    if context.backend().retained_compute_xgmi_copies_v1() != 1
        || context.backend().completed_compute_xgmi_copies_v1() != 0
        || context.cancel(&mut copy).map_err(error)? != RuntimeCancellationV1::TooLate
        || context.query_event(event).map_err(error)? != RuntimeCompletionStatusV1::Pending
    {
        return Err("published list did not retain one irreversible native operation".into());
    }
    let deadline = Instant::now() + WAIT;
    loop {
        context.progress_stream_v1(stream).map_err(error)?;
        let status = context.poll(&mut copy).map_err(error)?;
        let event_status = context.query_event(event).map_err(error)?;
        match status {
            RuntimePollV1::Succeeded if event_status == RuntimeCompletionStatusV1::Succeeded => {
                break;
            }
            RuntimePollV1::Pending
                if event_status == RuntimeCompletionStatusV1::Pending
                    && Instant::now() < deadline =>
            {
                if context.backend().completed_compute_xgmi_copies_v1() != 0 {
                    return Err("intermediate segment reported logical success".into());
                }
                std::thread::sleep(Duration::from_micros(50));
            }
            _ => {
                return Err(format!(
                    "list progress: result={status:?} event={event_status:?}"
                ));
            }
        }
    }
    if context.backend().retained_compute_xgmi_copies_v1() != 0
        || context.backend().completed_compute_xgmi_copies_v1() != 1
    {
        return Err("list did not settle as one native operation".into());
    }
    context.release_event(event).map_err(error)?;
    context.release_submission(copy).map_err(error)?;
    verify(&mut context, source.allocation, &input)?;
    verify(&mut context, destination.allocation, &output)?;
    for (host, device) in allocations.into_iter().rev() {
        context.release_allocation(device).map_err(error)?;
        context.release_allocation(host).map_err(error)?;
    }
    for stream in streams.into_iter().rev() {
        context.destroy_stream(stream).map_err(error)?;
    }
    context
        .shutdown()
        .map_err(error)?
        .shutdown_native_v1()
        .map_err(error)?;
    let mut hash = Sha256::new();
    hash.update(b"fe2o3.unified-peer-segments.v1\0");
    for data in [&input, &output] {
        hash.update((data.len() as u64).to_le_bytes());
        hash.update(data);
    }
    let packets: usize = layout
        .segments
        .iter()
        .map(|segment| {
            Gfx942ComputeXgmiPacketPlanV1::new(segment.byte_len)
                .unwrap()
                .count()
        })
        .sum();
    let useful_bytes: u64 = layout.segments.iter().map(|segment| segment.byte_len).sum();
    let output_sha256: String = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    println!(
        "PASS schema=fe2o3.unified-peer-segments.v1 devices=2 unique_ids={:#x},{:#x} case={} segments={} packets={} useful_bytes={} source_bytes={} destination_bytes={} source_envelope={}:{} destination_envelope={}:{} backend=unified-compute-xgmi authority=qualification-r57-n3-v2 kernels=0 native_counter=0,1 publication_retained=1 initial_cancel=cancelled late_cancel=too-late descriptor_snapshot=pass event_scope=whole-list source=full-byte-pass destination=full-byte-pass digest=length-prefixed-source-destination output_sha256={} cleanup=explicit-owned physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
        options.ids[0],
        options.ids[1],
        options.case,
        layout.segments.len(),
        packets,
        useful_bytes,
        layout.source_bytes,
        layout.destination_bytes,
        SOURCE_BASE,
        layout.source_len,
        DESTINATION_BASE,
        layout.destination_len,
        output_sha256
    );
    Ok(())
}

fn main() -> ResultV1<()> {
    run(options(&std::env::args().skip(1).collect::<Vec<_>>())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_reject_invalid_controls_before_native_open() {
        for arguments in [
            vec![],
            vec!["0x1", "0x1", "4"],
            vec!["0x0", "0x2", "4"],
            vec!["1", "0x2", "4"],
            vec!["0x1", "0x2", "0"],
            vec!["0x1", "0x2", "4097"],
            vec!["0x1", "0x2", "4", "extra"],
        ] {
            assert!(
                options(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
        assert_eq!(
            options(&["0x1".into(), "0x2".into(), "packets".into()])
                .unwrap()
                .ids,
            [1, 2]
        );
    }

    #[test]
    fn witness_layouts_fit_both_logical_envelopes_and_have_expected_packet_counts() {
        for case in ["1", "4", "65", "4096", "packets"] {
            let layout = layout(case);
            assert!(SOURCE_BASE + layout.source_len < layout.source_bytes as u64);
            assert!(DESTINATION_BASE + layout.destination_len < layout.destination_bytes as u64);
            let bytes = fe2o3_runtime_model::validate_ordered_peer_copy_segments_v1(
                SOURCE_BASE,
                layout.source_len,
                DESTINATION_BASE,
                layout.destination_len,
                &layout.segments,
            )
            .unwrap();
            assert_eq!(
                bytes,
                layout
                    .segments
                    .iter()
                    .map(|segment| segment.byte_len)
                    .sum::<u64>()
            );
            let packets: usize = layout
                .segments
                .iter()
                .map(|segment| {
                    Gfx942ComputeXgmiPacketPlanV1::new(segment.byte_len)
                        .unwrap()
                        .count()
                })
                .sum();
            assert_eq!(
                packets,
                if case == "packets" {
                    5
                } else {
                    case.parse().unwrap()
                }
            );
        }
    }

    #[test]
    fn oracle_preserves_guards_and_applies_overwrites_in_order() {
        let layout = layout("4");
        let input: Vec<_> = (0..layout.source_bytes as u64).map(pattern).collect();
        let output = expected(&layout, &input);
        let base = DESTINATION_BASE as usize;
        assert!(output[..base].iter().all(|byte| *byte == 0xa5));
        assert_eq!(
            &output[base..base + 64],
            &input[SOURCE_BASE as usize..SOURCE_BASE as usize + 64]
        );
        assert_eq!(
            &output[base + 64..base + 96],
            &input[SOURCE_BASE as usize + 160..SOURCE_BASE as usize + 192]
        );
        assert!(
            output[base + layout.destination_len as usize..]
                .iter()
                .all(|byte| *byte == 0xa5)
        );
    }
}
