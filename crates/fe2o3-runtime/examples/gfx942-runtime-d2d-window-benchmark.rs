//! Public-runtime-facade benchmark for same-device persistent SDMA copies.

use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

use fe2o3_runtime::{
    KfdRuntimeAuthorityRequestV1, KfdRuntimeBackendV1, KfdRuntimeLaunchAuthorityV1,
    RuntimeAccessV1, RuntimeContextV1, RuntimeCopyV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
    RuntimePollV1, RuntimeStreamIdV1, RuntimeSubmissionV1,
};

const COMPLETION_TIMEOUT: Duration = Duration::from_secs(60);
const COMPLETION_WAIT_SLICE: Duration = Duration::from_micros(50);
const MAX_COPY_BYTES: usize = 256 * 1024 * 1024;
const USAGE: &str =
    "usage: gfx942-runtime-d2d-window-benchmark <unique-id> <bytes> <warmups> <samples>";

type BenchmarkResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct CopyOnlyAuthorityV1;

// SAFETY: This benchmark never submits compute work. The authority therefore
// rejects every launch request and grants no machine-code execution authority.
unsafe impl KfdRuntimeLaunchAuthorityV1 for CopyOnlyAuthorityV1 {
    fn authorize_launch_v1(&self, _request: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

fn facade_error(error: impl Debug) -> Box<dyn Error> {
    format!("{error:?}").into()
}

fn parse_unique_id(value: &str) -> BenchmarkResult<u64> {
    let unique_id = if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        value.parse()?
    };
    if unique_id == 0 {
        return Err("unique ID must be nonzero".into());
    }
    Ok(unique_id)
}

fn percentile(values: &[u128], numerator: usize, denominator: usize) -> BenchmarkResult<u128> {
    if values.is_empty() || numerator == 0 || numerator > denominator || denominator == 0 {
        return Err("invalid percentile input".into());
    }
    let mut ordered = values.to_vec();
    ordered.sort_unstable();
    let rank = ordered
        .len()
        .checked_mul(numerator)
        .and_then(|value| value.checked_add(denominator - 1))
        .ok_or("percentile rank overflow")?
        / denominator;
    ordered
        .get(rank.checked_sub(1).ok_or("percentile rank underflow")?)
        .copied()
        .ok_or_else(|| "percentile rank is out of bounds".into())
}

fn pattern(round: usize) -> u8 {
    ((round.wrapping_mul(67).wrapping_add(1) % 251) + 1) as u8
}

fn region(
    allocation: fe2o3_runtime::RuntimeAllocationIdV1,
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

trait CopyProgressDriverV1 {
    fn wait(&mut self, timeout: Duration) -> BenchmarkResult<RuntimePollV1>;

    fn flush(&mut self) -> BenchmarkResult<()>;
}

struct FacadeCopyProgressDriverV1<'a> {
    context: &'a mut RuntimeContextV1<KfdRuntimeBackendV1>,
    submission: &'a mut RuntimeSubmissionV1<RuntimeCopyV1>,
    stream: RuntimeStreamIdV1,
}

impl CopyProgressDriverV1 for FacadeCopyProgressDriverV1<'_> {
    fn wait(&mut self, timeout: Duration) -> BenchmarkResult<RuntimePollV1> {
        self.context
            .wait(self.submission, timeout)
            .map_err(facade_error)
    }

    fn flush(&mut self) -> BenchmarkResult<()> {
        self.context.flush_stream(self.stream).map_err(facade_error)
    }
}

fn drive_copy_to_completion_v1(
    driver: &mut impl CopyProgressDriverV1,
    deadline: Instant,
) -> BenchmarkResult<()> {
    driver.flush()?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("same-device SDMA copy timed out".into());
        }
        match driver.wait(remaining.min(COMPLETION_WAIT_SLICE))? {
            RuntimePollV1::Succeeded => return Ok(()),
            RuntimePollV1::Pending => driver.flush()?,
            RuntimePollV1::Failed { code } => {
                return Err(format!("same-device SDMA copy failed with code {code}").into());
            }
        }
    }
}

fn run_copy(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
) -> BenchmarkResult<u128> {
    let start = Instant::now();
    let deadline = start
        .checked_add(COMPLETION_TIMEOUT)
        .ok_or("same-device SDMA deadline overflow")?;
    let mut submission: RuntimeSubmissionV1<RuntimeCopyV1> = context
        .copy_async(stream, source, destination, &[])
        .map_err(facade_error)?;
    {
        let mut driver = FacadeCopyProgressDriverV1 {
            context,
            submission: &mut submission,
            stream,
        };
        drive_copy_to_completion_v1(&mut driver, deadline)?;
    }
    let elapsed = start.elapsed().as_nanos();
    context
        .release_submission(submission)
        .map_err(facade_error)?;
    Ok(elapsed)
}

fn main() -> BenchmarkResult<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err(USAGE.into());
    }
    let unique_id = parse_unique_id(&args[0])?;
    let copy_bytes: usize = args[1].parse()?;
    let warmups: usize = args[2].parse()?;
    let samples: usize = args[3].parse()?;
    if copy_bytes == 0 || copy_bytes > MAX_COPY_BYTES || samples == 0 {
        return Err("copy size or statistical controls are out of range".into());
    }
    let byte_len = u64::try_from(copy_bytes)?;
    let rounds = warmups.checked_add(samples).ok_or("round count overflow")?;

    let backend = KfdRuntimeBackendV1::open_default(unique_id, CopyOnlyAuthorityV1)?;
    let mut context = RuntimeContextV1::open(backend).map_err(facade_error)?;
    if context.devices().len() != 1 || context.devices()[0].target() != "gfx942:xnack-" {
        return Err("direct KFD facade did not enumerate one gfx942:xnack- device".into());
    }
    let device = context.devices()[0].id();
    let target = context.devices()[0].target().to_owned();
    let capabilities = context.execution_capabilities(device)?;
    if !capabilities.native_async_copy || !capabilities.memory_pool {
        return Err("direct KFD facade did not advertise native persistent SDMA".into());
    }
    let stream = context.create_stream(device).map_err(facade_error)?;
    let source = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, byte_len, 4096)
        .map_err(facade_error)?;
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, byte_len, 4096)
        .map_err(facade_error)?;
    let validation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, byte_len, 4096)
        .map_err(facade_error)?;

    let mut host_image = vec![0_u8; copy_bytes];
    let mut observed = vec![0_u8; copy_bytes];
    let mut d2d = Vec::with_capacity(samples);
    for round in 0..rounds {
        let expected = pattern(round);
        host_image.fill(expected);
        context
            .write_allocation(source, 0, &host_image)
            .map_err(facade_error)?;
        host_image.fill(expected ^ 0xff);
        context
            .write_allocation(destination, 0, &host_image)
            .map_err(facade_error)?;

        let d2d_ns = run_copy(
            &mut context,
            stream,
            region(source, RuntimeAccessV1::Read, byte_len),
            region(destination, RuntimeAccessV1::Write, byte_len),
        )?;
        host_image.fill(expected ^ 0xa5);
        context
            .write_allocation(validation, 0, &host_image)
            .map_err(facade_error)?;
        let _ = run_copy(
            &mut context,
            stream,
            region(destination, RuntimeAccessV1::Read, byte_len),
            region(validation, RuntimeAccessV1::Write, byte_len),
        )?;
        context
            .read_allocation(validation, 0, &mut observed)
            .map_err(facade_error)?;
        if observed.iter().any(|byte| *byte != expected) {
            return Err(format!("same-device destination mismatch at round {round}").into());
        }
        host_image.fill(expected ^ 0x5a);
        context
            .write_allocation(validation, 0, &host_image)
            .map_err(facade_error)?;
        let _ = run_copy(
            &mut context,
            stream,
            region(source, RuntimeAccessV1::Read, byte_len),
            region(validation, RuntimeAccessV1::Write, byte_len),
        )?;
        context
            .read_allocation(validation, 0, &mut observed)
            .map_err(facade_error)?;
        if observed.iter().any(|byte| *byte != expected) {
            return Err(format!("same-device source changed at round {round}").into());
        }
        if round >= warmups {
            d2d.push(d2d_ns);
        }
    }

    let d2d_p50 = percentile(&d2d, 1, 2)?;
    let d2d_p95 = percentile(&d2d, 19, 20)?;
    if d2d_p50 == 0 {
        return Err("zero benchmark duration".into());
    }
    let packet_bytes = fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
    let packet_count = copy_bytes.div_ceil(packet_bytes);
    let max_window_packets = fe2o3_kfd::GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1;
    let window_count = packet_count.div_ceil(max_window_packets);

    context
        .release_allocation(validation)
        .map_err(facade_error)?;
    context
        .release_allocation(destination)
        .map_err(facade_error)?;
    context.release_allocation(source).map_err(facade_error)?;
    context.destroy_stream(stream).map_err(facade_error)?;
    let mut backend = context.shutdown().map_err(facade_error)?;
    backend.shutdown_native_v1().map_err(facade_error)?;

    println!(
        "backend=kfd schema=fe2o3.d2d-copy-benchmark.v1 device_index=0 unique_id={unique_id:016x} target={target} xnack=disabled bytes={copy_bytes} depth=1 warmups={warmups} samples={samples} d2d_p50_ns={d2d_p50} d2d_p95_ns={d2d_p95} d2d_p50_GBps={:.3} profile=same-device-d2d packet_count={packet_count} window_count={window_count} doorbells_per_copy={window_count} max_packets_per_window={max_window_packets} validation=full-source-and-destination-every-round teardown=explicit progress=explicit-flush-then-wait timing=facade-enqueue-flush-through-observed-completion",
        copy_bytes as f64 / d2d_p50 as f64,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    struct ScriptedProgressDriverV1 {
        observations: VecDeque<RuntimePollV1>,
        wait_slices: Vec<Duration>,
        flushes: usize,
    }

    impl CopyProgressDriverV1 for ScriptedProgressDriverV1 {
        fn wait(&mut self, timeout: Duration) -> BenchmarkResult<RuntimePollV1> {
            self.wait_slices.push(timeout);
            self.observations
                .pop_front()
                .ok_or_else(|| "missing scripted progress observation".into())
        }

        fn flush(&mut self) -> BenchmarkResult<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    #[test]
    fn sixty_three_plus_two_window_copy_flushes_without_full_timeout_wait() {
        let packet_bytes = usize::try_from(fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
            .expect("packet length fits usize");
        let packet_count = MAX_COPY_BYTES.div_ceil(packet_bytes);
        assert_eq!(packet_count, 65);
        assert_eq!(
            packet_count
                .div_ceil(fe2o3_kfd::GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1),
            2
        );

        let mut driver = ScriptedProgressDriverV1 {
            observations: [RuntimePollV1::Pending, RuntimePollV1::Succeeded].into(),
            wait_slices: Vec::new(),
            flushes: 0,
        };
        drive_copy_to_completion_v1(
            &mut driver,
            Instant::now()
                .checked_add(COMPLETION_TIMEOUT)
                .expect("fixed timeout is representable"),
        )
        .unwrap();

        assert!(driver.observations.is_empty());
        assert_eq!(driver.flushes, 2);
        assert_eq!(driver.wait_slices.len(), 2);
        assert!(
            driver
                .wait_slices
                .iter()
                .all(|slice| *slice <= COMPLETION_WAIT_SLICE)
        );
    }
}
