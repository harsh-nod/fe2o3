//! Optional host timing diagnostic; the legacy comparator stays uninstrumented.

use std::io::Write;

use super::*;

const SCHEMA: &str = "fe2o3.kfd-directional-progress-diagnostic.v1";
const MAX_ROUNDS: usize = 10_000;
const DIAGNOSTIC_USAGE: &str = "diagnostic usage: gfx942-runtime-directional-window-benchmark <unique-id> <bytes> <warmups> <samples> <diagnostic-slice50us|diagnostic-window-deadline>";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitPolicyV1 {
    Slice50us,
    WindowDeadline,
}

impl WaitPolicyV1 {
    fn name(self) -> &'static str {
        match self {
            Self::Slice50us => "slice50us",
            Self::WindowDeadline => "window-deadline",
        }
    }

    fn budget(self, remaining: Duration) -> Duration {
        match self {
            Self::Slice50us => remaining.min(COMPLETION_WAIT_SLICE),
            Self::WindowDeadline => remaining,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DiagnosticConfigV1 {
    unique_id: u64,
    bytes: usize,
    warmups: usize,
    samples: usize,
    rounds: usize,
    policy: WaitPolicyV1,
}

pub(super) fn parse_config_v1(args: &[String]) -> BenchmarkResult<Option<DiagnosticConfigV1>> {
    if args.len() == 4 {
        return Ok(None);
    }
    if args.len() != 5 {
        return Err(USAGE.into());
    }
    let policy = match args[4].as_str() {
        "diagnostic-slice50us" => WaitPolicyV1::Slice50us,
        "diagnostic-window-deadline" => WaitPolicyV1::WindowDeadline,
        _ => return Err(DIAGNOSTIC_USAGE.into()),
    };
    let unique_id = parse_unique_id(&args[0])?;
    let bytes = args[1].parse()?;
    let warmups: usize = args[2].parse()?;
    let samples: usize = args[3].parse()?;
    let rounds = warmups.checked_add(samples).ok_or("round count overflow")?;
    if bytes == 0 || bytes > MAX_COPY_BYTES || samples == 0 || rounds > MAX_ROUNDS {
        return Err("diagnostic copy size or statistical controls are out of range".into());
    }
    Ok(Some(DiagnosticConfigV1 {
        unique_id,
        bytes,
        warmups,
        samples,
        rounds,
        policy,
    }))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ProgressCountsV1 {
    waits: u64,
    flushes: u64,
}

fn increment(count: &mut u64) -> BenchmarkResult<()> {
    *count = count
        .checked_add(1)
        .ok_or("diagnostic call count overflow")?;
    Ok(())
}

fn drive_v1(
    driver: &mut impl CopyProgressDriverV1,
    policy: WaitPolicyV1,
    mut remaining: impl FnMut() -> Duration,
) -> BenchmarkResult<ProgressCountsV1> {
    let mut counts = ProgressCountsV1::default();
    increment(&mut counts.flushes)?;
    driver.flush()?;
    loop {
        let remaining = remaining();
        if remaining.is_zero() {
            return Err("directional SDMA copy timed out".into());
        }
        increment(&mut counts.waits)?;
        match driver.wait(policy.budget(remaining))? {
            RuntimePollV1::Succeeded => return Ok(counts),
            RuntimePollV1::Pending => {
                increment(&mut counts.flushes)?;
                driver.flush()?;
            }
            RuntimePollV1::Failed { code } => {
                return Err(format!("directional SDMA copy failed with code {code}").into());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CopyTimingV1 {
    submit_ns: u128,
    progress_ns: u128,
    total_ns: u128,
    counts: ProgressCountsV1,
}

impl CopyTimingV1 {
    fn from_instants(
        start: Instant,
        submitted: Instant,
        finished: Instant,
        counts: ProgressCountsV1,
    ) -> BenchmarkResult<Self> {
        let submit_ns = submitted
            .checked_duration_since(start)
            .ok_or("nonmonotonic submission timing")?
            .as_nanos();
        let progress_ns = finished
            .checked_duration_since(submitted)
            .ok_or("nonmonotonic progress timing")?
            .as_nanos();
        let total_ns = finished
            .checked_duration_since(start)
            .ok_or("nonmonotonic total timing")?
            .as_nanos();
        if total_ns == 0
            || submit_ns.checked_add(progress_ns) != Some(total_ns)
            || counts.waits == 0
            || counts.flushes != counts.waits
        {
            return Err("invalid diagnostic timing or call counts".into());
        }
        Ok(Self {
            submit_ns,
            progress_ns,
            total_ns,
            counts,
        })
    }
}

fn run_copy_v1(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
    policy: WaitPolicyV1,
) -> BenchmarkResult<CopyTimingV1> {
    let start = Instant::now();
    let deadline = start
        .checked_add(COMPLETION_TIMEOUT)
        .ok_or("directional SDMA deadline overflow")?;
    let mut submission: RuntimeSubmissionV1<RuntimeCopyV1> = context
        .copy_async(stream, source, destination, &[])
        .map_err(facade_error)?;
    let submitted = Instant::now();
    let counts = {
        let mut driver = FacadeCopyProgressDriverV1 {
            context,
            submission: &mut submission,
            stream,
        };
        drive_v1(&mut driver, policy, || {
            deadline.saturating_duration_since(Instant::now())
        })?
    };
    let finished = Instant::now();
    context
        .release_submission(submission)
        .map_err(facade_error)?;
    CopyTimingV1::from_instants(start, submitted, finished, counts)
}

struct RoundV1 {
    h2d: CopyTimingV1,
    d2h: CopyTimingV1,
}

struct CompletedRunV1 {
    config: DiagnosticConfigV1,
    rounds: Vec<RoundV1>,
}

fn execute_v1(config: DiagnosticConfigV1) -> BenchmarkResult<CompletedRunV1> {
    let mut rounds = Vec::new();
    rounds.try_reserve_exact(config.rounds)?;
    let mut host_image = Vec::new();
    host_image.try_reserve_exact(config.bytes)?;
    host_image.resize(config.bytes, 0_u8);
    let mut observed = Vec::new();
    observed.try_reserve_exact(config.bytes)?;
    observed.resize(config.bytes, 0_u8);

    let backend = KfdRuntimeBackendV1::open_default(config.unique_id, CopyOnlyAuthorityV1)?;
    let mut context = RuntimeContextV1::open(backend).map_err(facade_error)?;
    if context.devices().len() != 1 || context.devices()[0].target() != "gfx942:xnack-" {
        return Err("direct KFD diagnostic did not enumerate one gfx942:xnack- device".into());
    }
    let device = context.devices()[0].id();
    let capabilities = context.execution_capabilities(device)?;
    if !capabilities.native_async_copy || !capabilities.memory_pool {
        return Err("direct KFD diagnostic requires native persistent SDMA".into());
    }
    let byte_len = u64::try_from(config.bytes)?;
    let stream = context.create_stream(device).map_err(facade_error)?;
    let upload = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, byte_len, 4096)
        .map_err(facade_error)?;
    let device_buffer = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, byte_len, 4096)
        .map_err(facade_error)?;
    let download = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, byte_len, 4096)
        .map_err(facade_error)?;
    for index in 0..config.rounds {
        let expected = pattern(index);
        host_image.fill(expected);
        context
            .write_allocation(upload, 0, &host_image)
            .map_err(facade_error)?;
        host_image.fill(expected ^ 0xff);
        context
            .write_allocation(download, 0, &host_image)
            .map_err(facade_error)?;
        let h2d = run_copy_v1(
            &mut context,
            stream,
            region(upload, RuntimeAccessV1::Read, byte_len),
            region(device_buffer, RuntimeAccessV1::Write, byte_len),
            config.policy,
        )?;
        let d2h = run_copy_v1(
            &mut context,
            stream,
            region(device_buffer, RuntimeAccessV1::Read, byte_len),
            region(download, RuntimeAccessV1::Write, byte_len),
            config.policy,
        )?;
        context
            .read_allocation(download, 0, &mut observed)
            .map_err(facade_error)?;
        if observed.iter().any(|byte| *byte != expected) {
            return Err(format!("directional SDMA copy mismatch at round {index}").into());
        }
        rounds.push(RoundV1 { h2d, d2h });
    }
    context.release_allocation(download).map_err(facade_error)?;
    context
        .release_allocation(device_buffer)
        .map_err(facade_error)?;
    context.release_allocation(upload).map_err(facade_error)?;
    context.destroy_stream(stream).map_err(facade_error)?;
    let mut backend = context.shutdown().map_err(facade_error)?;
    backend.shutdown_native_v1().map_err(facade_error)?;
    Ok(CompletedRunV1 { config, rounds })
}

impl CompletedRunV1 {
    fn write(&self, output: &mut impl Write) -> BenchmarkResult<()> {
        let config = self.config;
        if self.rounds.len() != config.rounds {
            return Err("incomplete diagnostic round roster".into());
        }
        let packet_bytes = usize::try_from(fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)?;
        let packets = config.bytes.div_ceil(packet_bytes);
        let max_window_packets =
            fe2o3_kfd::GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1;
        let windows = packets.div_ceil(max_window_packets);
        writeln!(
            output,
            "schema={SCHEMA} record=config backend=kfd unique_id={:016x} target=gfx942:xnack- bytes={} depth=1 warmups={} samples={} wait_policy={} wait_slice_ns={} outer_timeout_ns={} packets_per_transfer={packets} windows_per_transfer={windows} max_packets_per_window={max_window_packets} h2d_engine_index=1 d2h_engine_index=0 optional_context_journal=disabled timing=host-submit-and-progress",
            config.unique_id,
            config.bytes,
            config.warmups,
            config.samples,
            config.policy.name(),
            if config.policy == WaitPolicyV1::Slice50us {
                COMPLETION_WAIT_SLICE.as_nanos()
            } else {
                0
            },
            COMPLETION_TIMEOUT.as_nanos(),
        )?;
        for (index, round) in self.rounds.iter().enumerate() {
            let phase = if index < config.warmups {
                "warmup"
            } else {
                "sample"
            };
            writeln!(
                output,
                "schema={SCHEMA} record=round index={index} phase={phase} pattern={} h2d_submit_ns={} h2d_progress_ns={} h2d_total_ns={} h2d_wait_calls={} h2d_flush_calls={} d2h_submit_ns={} d2h_progress_ns={} d2h_total_ns={} d2h_wait_calls={} d2h_flush_calls={} checked_bytes={}",
                pattern(index),
                round.h2d.submit_ns,
                round.h2d.progress_ns,
                round.h2d.total_ns,
                round.h2d.counts.waits,
                round.h2d.counts.flushes,
                round.d2h.submit_ns,
                round.d2h.progress_ns,
                round.d2h.total_ns,
                round.d2h.counts.waits,
                round.d2h.counts.flushes,
                config.bytes,
            )?;
        }
        writeln!(
            output,
            "schema={SCHEMA} record=complete validated_rounds={} measured_rounds={} checked_bytes_per_round={} validation=full-returned-buffer-every-round teardown=explicit-complete",
            self.rounds.len(),
            config.samples,
            config.bytes,
        )?;
        output.flush()?;
        Ok(())
    }
}

pub(super) fn run_v1(config: DiagnosticConfigV1, output: &mut impl Write) -> BenchmarkResult<()> {
    execute_v1(config)?.write(output)
}

#[cfg(test)]
#[path = "directional_copy_diagnostic_tests.rs"]
mod tests;
