//! Matched ordered-list host latency, not device/link bandwidth or parity evidence.

use fe2o3_runtime::{
    KfdNativeXgmiRuntimeBackendV1, RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeContextV1,
    RuntimeDeviceIdV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimePeerCopySegmentV1,
    RuntimePollV1, RuntimeStreamIdV1,
};
use std::{
    error::Error,
    fmt::Debug,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Context = RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>;
const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct Plan {
    useful: usize,
    warmups: usize,
    samples: usize,
    band_bytes: usize,
    destination_bytes: usize,
    bands: usize,
    segments: Vec<RuntimePeerCopySegmentV1>,
}

impl Plan {
    fn new(bytes: usize, count: usize, warmups: usize, samples: usize) -> Result<Self> {
        let rounds = warmups.checked_add(samples).ok_or("round overflow")?;
        if bytes == 0
            || bytes > 2 * 1024 * 1024
            || count == 0
            || count > 4096
            || bytes < count
            || samples == 0
            || rounds > 64
        {
            return Err("invalid bounded segment workload".into());
        }
        let quotient = bytes / count;
        let remainder = bytes % count;
        let slot_bytes = (quotient + 2 + 16).div_ceil(64) * 64;
        let band_bytes = (64 + count * slot_bytes).div_ceil(4096) * 4096;
        let bands = rounds + 1;
        let destination_bytes = bands.checked_mul(band_bytes).ok_or("band overflow")?;
        if destination_bytes > 256 * 1024 * 1024 {
            return Err("destination budget exceeded".into());
        }
        let segments: Vec<_> = (0..count)
            .map(|i| {
                let mut length = quotient + usize::from(i < remainder);
                if quotient >= 2 && count > 1 {
                    if i % 2 == 0 && i + 1 < count {
                        length += 1;
                    } else if i % 2 == 1 {
                        length -= 1;
                    }
                }
                RuntimePeerCopySegmentV1 {
                    source_offset: (32 + i * slot_bytes) as u64,
                    destination_offset: (32 + (count - 1 - i) * slot_bytes) as u64,
                    byte_len: length as u64,
                }
            })
            .collect();
        let total = fe2o3_runtime_model::validate_ordered_peer_copy_segments_v1(
            0,
            band_bytes as u64,
            0,
            band_bytes as u64,
            &segments,
        )
        .map_err(error)?;
        if total != bytes as u64 {
            return Err("segment payload does not match declared size".into());
        }
        Ok(Self {
            useful: bytes,
            warmups,
            samples,
            band_bytes,
            destination_bytes,
            bands,
            segments,
        })
    }

    fn source_byte(position: usize, direction: usize) -> u8 {
        (((position % 251) * 131 + ((position / 256) % 251) * 17 + direction * 73 + 1) % 251 + 1)
            as u8
    }

    fn source(&self, direction: usize) -> Vec<u8> {
        (0..self.band_bytes)
            .map(|i| Self::source_byte(i, direction))
            .collect()
    }

    fn destination(&self, direction: usize, completed: bool) -> Vec<u8> {
        let mut result = vec![if direction == 0 { 0xa5 } else { 0x5a }; self.destination_bytes];
        for band in 0..self.bands {
            for segment in &self.segments {
                for byte in 0..segment.byte_len as usize {
                    let expected =
                        Self::source_byte(segment.source_offset as usize + byte, direction);
                    result[band * self.band_bytes + segment.destination_offset as usize + byte] =
                        if completed {
                            expected
                        } else {
                            expected ^ (band + 1) as u8
                        };
                }
            }
        }
        result
    }

    fn describe(&self) -> serde_json::Value {
        fn checksum(bytes: &[u8]) -> String {
            let value = bytes.iter().fold(14695981039346656037_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(1099511628211)
            });
            format!("{value:016x}")
        }
        serde_json::json!({
            "useful_bytes": self.useful, "warmups": self.warmups, "samples": self.samples,
            "band_bytes": self.band_bytes, "destination_bytes": self.destination_bytes,
            "bands": self.bands,
            "segments": self.segments.iter().map(|s| [s.source_offset, s.destination_offset, s.byte_len]).collect::<Vec<_>>(),
            "compatibility_checksums_fnv1a64": (0..2).map(|direction| [
                checksum(&self.source(direction)), checksum(&self.destination(direction, false)),
                checksum(&self.destination(direction, true))
            ]).collect::<Vec<_>>()
        })
    }
}

fn decimal(text: &str) -> Result<usize> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("decimal control required".into());
    }
    Ok(text.parse()?)
}

fn uid(text: &str) -> Result<u64> {
    let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
            u64::from_str_radix(hex, 16)?
        }
        None if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) => text.parse()?,
        _ => return Err("invalid unique ID".into()),
    };
    if value == 0 {
        return Err("zero unique ID".into());
    }
    Ok(value)
}

fn error(value: impl Debug) -> Box<dyn Error> {
    format!("{value:?}").into()
}

struct Direction {
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    stream: RuntimeStreamIdV1,
}

impl Direction {
    fn prepare(
        context: &mut Context,
        devices: [RuntimeDeviceIdV1; 2],
        plan: &Plan,
        direction: usize,
    ) -> Result<Self> {
        let source = context
            .allocate(
                devices[direction],
                RuntimeMemoryKindV1::DeviceLocal,
                plan.band_bytes as u64,
                4096,
            )
            .map_err(error)?;
        let destination = context
            .allocate(
                devices[1 - direction],
                RuntimeMemoryKindV1::DeviceLocal,
                plan.destination_bytes as u64,
                4096,
            )
            .map_err(error)?;
        let stream = context
            .create_stream(devices[1 - direction])
            .map_err(error)?;
        context
            .write_allocation(source, 0, &plan.source(direction))
            .map_err(error)?;
        context
            .write_allocation(destination, 0, &plan.destination(direction, false))
            .map_err(error)?;
        Ok(Self {
            source,
            destination,
            stream,
        })
    }

    fn copy(&self, context: &mut Context, plan: &Plan, band: usize) -> Result<u128> {
        let source = RuntimeMemoryRegionV1 {
            allocation: self.source,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: plan.band_bytes as u64,
        };
        let destination = RuntimeMemoryRegionV1 {
            allocation: self.destination,
            access: RuntimeAccessV1::Write,
            byte_offset: (band * plan.band_bytes) as u64,
            byte_len: plan.band_bytes as u64,
        };
        let start = Instant::now();
        let deadline = start + TIMEOUT;
        let mut submission = context
            .peer_copy_segments(self.stream, source, destination, &plan.segments, &[])
            .map_err(error)?;
        let remaining = deadline.saturating_duration_since(Instant::now());
        let result = context.wait(&mut submission, remaining).map_err(error)?;
        let end = Instant::now();
        if result != RuntimePollV1::Succeeded || end >= deadline {
            return Err("whole-list completion failed or exceeded deadline".into());
        }
        let elapsed = end.duration_since(start).as_nanos();
        if elapsed == 0 {
            return Err("zero duration".into());
        }
        context.release_submission(submission).map_err(error)?;
        Ok(elapsed)
    }

    fn validate(&self, context: &mut Context, plan: &Plan, direction: usize) -> Result<()> {
        let mut observed = vec![0; plan.band_bytes];
        context
            .read_allocation(self.source, 0, &mut observed)
            .map_err(error)?;
        if observed != plan.source(direction) {
            return Err("source changed".into());
        }
        observed.resize(plan.destination_bytes, 0);
        context
            .read_allocation(self.destination, 0, &mut observed)
            .map_err(error)?;
        if observed != plan.destination(direction, true) {
            return Err("destination or canary mismatch".into());
        }
        Ok(())
    }

    fn release(self, context: &mut Context) -> Result<()> {
        context
            .release_allocation(self.destination)
            .map_err(error)?;
        context.release_allocation(self.source).map_err(error)?;
        context.destroy_stream(self.stream).map_err(error)?;
        Ok(())
    }
}

fn run(ids: [u64; 2], plan: &Plan) -> Result<()> {
    let backend = KfdNativeXgmiRuntimeBackendV1::open_default(ids[0], ids[1])?;
    let mut context =
        RuntimeContextV1::open_with_version_journal_v1(backend, 16, 8).map_err(error)?;
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let directions = [
        Direction::prepare(&mut context, devices, plan, 0)?,
        Direction::prepare(&mut context, devices, plan, 1)?,
    ];
    let mut times = Vec::with_capacity(plan.bands * 2);
    for band in 0..plan.bands {
        for direction in &directions {
            times.push(direction.copy(&mut context, plan, band)?);
        }
    }
    for (direction, resources) in directions.iter().enumerate() {
        resources.validate(&mut context, plan, direction)?;
    }
    for resources in directions.into_iter().rev() {
        resources.release(&mut context)?;
    }
    let mut backend = context.shutdown().map_err(error)?;
    backend.shutdown_native_v1().map_err(error)?;
    for band in 0..plan.bands {
        let population = if band == 0 {
            "prime"
        } else if band <= plan.warmups {
            "warmup"
        } else {
            "sample"
        };
        for direction in 0..2 {
            println!(
                "schema=fe2o3.xgmi-ordered-segments.v1 record=list backend=kfd band={band} direction={direction} population={population} elapsed_ns={}",
                times[band * 2 + direction]
            );
        }
    }
    println!(
        "schema=fe2o3.xgmi-ordered-segments.v1 record=complete backend=kfd unique_ids={:016x},{:016x} useful_bytes={} descriptor_count={} logical_depth=1 warmups={} samples={} prime_lists=1 band_bytes={} source_bytes={} destination_bytes={} layout=reversed-ragged-slots-v1 progress=single-ticket-sequence-full-currentness deadline_ns=60000000000 timing=list-admission-through-observed-completion mapping_lifetime=retained-pair-no-allocation-host-readwrite-between-lists completion_cleanup=outside-timing correctness=passed teardown=explicit",
        ids[0],
        ids[1],
        plan.useful,
        plan.segments.len(),
        plan.warmups,
        plan.samples,
        plan.band_bytes,
        plan.band_bytes,
        plan.destination_bytes
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 5 && args[0] == "--describe-plan" {
        let plan = Plan::new(
            decimal(&args[1])?,
            decimal(&args[2])?,
            decimal(&args[3])?,
            decimal(&args[4])?,
        )?;
        println!("{}", plan.describe());
        return Ok(());
    }
    if args.len() != 6 {
        return Err("usage: gfx942-runtime-xgmi-segments-benchmark <uid-0> <uid-1> <useful-bytes> <segments> <warmups> <samples>".into());
    }
    let ids = [uid(&args[0])?, uid(&args[1])?];
    if ids[0] == ids[1] {
        return Err("distinct devices required".into());
    }
    let plan = Plan::new(
        decimal(&args[2])?,
        decimal(&args[3])?,
        decimal(&args[4])?,
        decimal(&args[5])?,
    )?;
    if let Err(failure) = run(ids, &plan) {
        // Context/native owners retain ambiguous resources; emit no timing rows.
        eprintln!("ordered peer benchmark failed: {failure}");
        std::process::exit(3);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_bounded_ragged_disjoint_and_has_exact_useful_size() {
        for bytes in [65_536, 2_097_152] {
            for count in [1, 65, 256, 4096] {
                let plan = Plan::new(bytes, count, 10, 30).unwrap();
                assert_eq!(plan.segments.len(), count);
                assert_eq!(
                    plan.segments.iter().map(|s| s.byte_len).sum::<u64>(),
                    bytes as u64
                );
                assert_eq!(plan.band_bytes % 4096, 0);
                assert_eq!(plan.bands, 41);
                let mut ranges: Vec<_> = plan
                    .segments
                    .iter()
                    .map(|s| (s.destination_offset, s.destination_offset + s.byte_len))
                    .collect();
                ranges.sort_unstable();
                assert!(ranges.windows(2).all(|r| r[0].1 < r[1].0));
                assert!(plan.segments.iter().all(|s| s.byte_len > 0
                    && s.source_offset + s.byte_len < plan.band_bytes as u64
                    && s.destination_offset + s.byte_len < plan.band_bytes as u64));
            }
        }
    }

    #[test]
    fn each_list_has_independent_poison_and_canaries() {
        let plan = Plan::new(257, 65, 1, 3).unwrap();
        for direction in 0..2 {
            let source = plan.source(direction);
            let expected = plan.destination(direction, true);
            for omitted in 0..plan.bands {
                let mut observed = plan.destination(direction, false);
                for band in 0..plan.bands {
                    if band == omitted {
                        continue;
                    }
                    for s in &plan.segments {
                        let to = band * plan.band_bytes + s.destination_offset as usize;
                        observed[to..to + s.byte_len as usize].copy_from_slice(
                            &source
                                [s.source_offset as usize..(s.source_offset + s.byte_len) as usize],
                        );
                    }
                }
                assert_ne!(observed, expected);
            }
        }
    }

    #[test]
    fn malformed_and_unbounded_controls_fail_before_native_open() {
        for (b, n, w, s) in [
            (0, 1, 0, 1),
            (1, 0, 0, 1),
            (1, 2, 0, 1),
            (4097, 4097, 0, 1),
            (2_097_153, 1, 0, 1),
            (1, 1, 0, 0),
            (1, 1, 64, 1),
            (1, 1, usize::MAX, 1),
        ] {
            assert!(Plan::new(b, n, w, s).is_err());
        }
        for text in ["", "+1", "-1", " 1", "1 ", "1e2"] {
            assert!(decimal(text).is_err());
        }
        for text in ["0", "0x", "-1", "+1", "0x-1"] {
            assert!(uid(text).is_err());
        }
    }
}
