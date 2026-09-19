//! Public-runtime-facade gfx942 XGMI peer-copy benchmark.

use std::error::Error;
use std::fmt::Debug;
use std::time::{Duration, Instant};

use fe2o3_runtime::{
    KfdNativeXgmiRuntimeBackendV1, MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1, RuntimeAccessV1,
    RuntimeAllocationIdV1, RuntimeContextV1, RuntimeDeviceIdV1, RuntimeMemoryKindV1,
    RuntimeMemoryRegionV1, RuntimePeerCopyBatchPollV1, RuntimePeerCopyV1, RuntimePollV1,
    RuntimeStreamIdV1, RuntimeSubmissionV1,
};

const CANARY_BYTES: usize = 32;
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_DEPTH: usize = 32;
const USAGE: &str = "usage: gfx942-runtime-xgmi-peer-benchmark <unique-id-0> <unique-id-1> <bytes> <depth> <warmups> <samples> [--diagnose-xgmi|--aggregate-peer-batch|--aggregate-peer-batch-hot-only|--aggregate-peer-batch-hot-diagnose]";

type BenchmarkResult<T> = Result<T, Box<dyn Error>>;
type XgmiContextV1 = RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>;

struct DirectionResourcesV1 {
    sources: Vec<RuntimeAllocationIdV1>,
    destinations: Vec<RuntimeAllocationIdV1>,
    streams: Vec<RuntimeStreamIdV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProgressModeV1 {
    Ordinary,
    Diagnostic,
    AggregatePeerBatch,
    AggregatePeerBatchHotOnly,
    AggregatePeerBatchHotDiagnostic,
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
        return Err("unique IDs must be nonzero".into());
    }
    Ok(unique_id)
}

fn pattern(round: usize, slot: usize, direction: usize) -> u8 {
    let value = ((round as u128 * 67 + slot as u128 * 29 + direction as u128 * 101 + 1) % 251) + 1;
    u8::try_from(value).expect("pattern is reduced to u8 range")
}

fn percentile(values: &[u128], numerator: usize, denominator: usize) -> Option<u128> {
    if values.is_empty() || numerator == 0 || denominator == 0 || numerator > denominator {
        return None;
    }
    let scaled = values.len() as u128 * numerator as u128 + denominator as u128 - 1;
    let rank = usize::try_from(scaled / denominator as u128).ok()?;
    values.get(rank.checked_sub(1)?).copied()
}

fn progress_mode(args: &[String]) -> BenchmarkResult<ProgressModeV1> {
    if args.len() == 6 {
        return Ok(ProgressModeV1::Ordinary);
    }
    if args.len() != 7 {
        return Err(USAGE.into());
    }
    match args[6].as_str() {
        "--diagnose-xgmi" if cfg!(feature = "hardware-diagnostic") => {
            Ok(ProgressModeV1::Diagnostic)
        }
        "--diagnose-xgmi" => Err("--diagnose-xgmi requires the hardware-diagnostic feature".into()),
        "--aggregate-peer-batch" => Ok(ProgressModeV1::AggregatePeerBatch),
        "--aggregate-peer-batch-hot-only" => Ok(ProgressModeV1::AggregatePeerBatchHotOnly),
        "--aggregate-peer-batch-hot-diagnose" if cfg!(feature = "hardware-diagnostic") => {
            Ok(ProgressModeV1::AggregatePeerBatchHotDiagnostic)
        }
        "--aggregate-peer-batch-hot-diagnose" => Err(
            "--aggregate-peer-batch-hot-diagnose requires the hardware-diagnostic feature".into(),
        ),
        _ => Err(USAGE.into()),
    }
}

fn valid_depth(mode: ProgressModeV1, depth: usize) -> bool {
    let maximum = match mode {
        ProgressModeV1::Ordinary | ProgressModeV1::Diagnostic => MAX_DEPTH,
        ProgressModeV1::AggregatePeerBatch => MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1,
        ProgressModeV1::AggregatePeerBatchHotOnly
        | ProgressModeV1::AggregatePeerBatchHotDiagnostic => 1,
    };
    depth != 0 && depth <= maximum
}

fn is_aggregate_mode(mode: ProgressModeV1) -> bool {
    matches!(
        mode,
        ProgressModeV1::AggregatePeerBatch
            | ProgressModeV1::AggregatePeerBatchHotOnly
            | ProgressModeV1::AggregatePeerBatchHotDiagnostic
    )
}

fn includes_remap_phase(mode: ProgressModeV1) -> bool {
    !matches!(
        mode,
        ProgressModeV1::AggregatePeerBatchHotOnly | ProgressModeV1::AggregatePeerBatchHotDiagnostic
    )
}

fn report_schema(mode: ProgressModeV1) -> &'static str {
    match mode {
        ProgressModeV1::AggregatePeerBatch => "fe2o3.xgmi-peer-aggregate-benchmark.v1",
        ProgressModeV1::AggregatePeerBatchHotOnly
        | ProgressModeV1::AggregatePeerBatchHotDiagnostic => {
            "fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1"
        }
        ProgressModeV1::Ordinary | ProgressModeV1::Diagnostic => "fe2o3.xgmi-peer-benchmark.v1",
    }
}

#[cfg(any(feature = "hardware-diagnostic", test))]
fn diagnostic_submission_count(rounds: usize, depth: usize) -> BenchmarkResult<usize> {
    if depth != 1 {
        return Err("XGMI diagnostics require depth 1".into());
    }
    rounds
        .checked_mul(4)
        .and_then(|n| n.checked_add(2))
        .filter(|n| *n <= 20_000)
        .ok_or_else(|| "XGMI diagnostic submission count exceeds the bounded roster".into())
}

#[cfg(any(feature = "hardware-diagnostic", test))]
fn aggregate_diagnostic_call_count(rounds: usize, depth: usize) -> BenchmarkResult<usize> {
    if depth != 1 {
        return Err("XGMI aggregate diagnostics require depth 1".into());
    }
    rounds
        .checked_add(1)
        .and_then(|n| n.checked_mul(2))
        .filter(|n| *n <= 40_000)
        .ok_or_else(|| "XGMI aggregate diagnostic call count exceeds the bounded roster".into())
}

fn diagnostic_label(enabled: bool) -> &'static str {
    if enabled {
        " diagnostic=xgmi-host-stages-v1"
    } else {
        ""
    }
}

fn aggregate_diagnostic_label(mode: ProgressModeV1) -> &'static str {
    if mode == ProgressModeV1::AggregatePeerBatchHotDiagnostic {
        " diagnostic=aggregate-host-attribution"
    } else {
        ""
    }
}

#[allow(clippy::too_many_arguments)]
fn report_measurement(
    unique_ids: [u64; 2],
    copy_bytes: usize,
    depth: usize,
    warmups: usize,
    samples: usize,
    measurement: &str,
    mapping_lifetime: &str,
    prime_batches: usize,
    mode: ProgressModeV1,
    mut forward_ns: Vec<u128>,
    mut reverse_ns: Vec<u128>,
) -> BenchmarkResult<()> {
    forward_ns.sort_unstable();
    reverse_ns.sort_unstable();
    let forward_p50 = percentile(&forward_ns, 1, 2).ok_or("missing forward p50")?;
    let forward_p95 = percentile(&forward_ns, 19, 20).ok_or("missing forward p95")?;
    let reverse_p50 = percentile(&reverse_ns, 1, 2).ok_or("missing reverse p50")?;
    let reverse_p95 = percentile(&reverse_ns, 19, 20).ok_or("missing reverse p95")?;
    if forward_p50 == 0 || reverse_p50 == 0 {
        return Err("zero XGMI benchmark duration".into());
    }
    let bytes_per_round = copy_bytes
        .checked_mul(depth)
        .ok_or("XGMI bytes per round overflow")?;
    if is_aggregate_mode(mode) {
        println!(
            "backend=kfd schema={} surface=runtime-facade unique_ids={:016x},{:016x} target=gfx942:xnack- bytes={} depth={} queue_depth={} batch_size={} direction=forward-then-reverse outstanding_depth={} engine_parallelism=ordered-single-sdma warmups={} samples={} measurement={} peer_access=topology-xgmi mapping_lifetime={} prime_batches={} doorbells_per_batch=1 progress=explicit-exact-roster-aggregate-wait aggregate_roster=exact-round-submissions background_progress=false forward_engine=topology-selected reverse_engine=topology-selected forward_p50_ns={} forward_p95_ns={} forward_p50_GBps={:.3} reverse_p50_ns={} reverse_p95_ns={} reverse_p50_GBps={:.3} canaries=pass teardown=explicit timing=facade-enqueue-through-aggregate-close{}",
            report_schema(mode),
            unique_ids[0],
            unique_ids[1],
            copy_bytes,
            depth,
            depth,
            depth,
            depth,
            warmups,
            samples,
            measurement,
            mapping_lifetime,
            prime_batches,
            forward_p50,
            forward_p95,
            bytes_per_round as f64 / forward_p50 as f64,
            reverse_p50,
            reverse_p95,
            bytes_per_round as f64 / reverse_p50 as f64,
            aggregate_diagnostic_label(mode),
        );
    } else {
        println!(
            "backend=kfd schema={} surface=runtime-facade unique_ids={:016x},{:016x} target=gfx942:xnack- bytes={} depth={} queue_depth={} batch_size={} direction=forward-then-reverse outstanding_depth={} engine_parallelism=ordered-single-sdma warmups={} samples={} measurement={} peer_access=topology-xgmi mapping_lifetime={} prime_batches={} doorbells_per_batch=1 progress=explicit-flush-then-wait background_progress=false forward_engine=topology-selected reverse_engine=topology-selected forward_p50_ns={} forward_p95_ns={} forward_p50_GBps={:.3} reverse_p50_ns={} reverse_p95_ns={} reverse_p50_GBps={:.3} canaries=pass teardown=explicit timing=facade-enqueue-flush-through-observed-completion{}",
            report_schema(mode),
            unique_ids[0],
            unique_ids[1],
            copy_bytes,
            depth,
            depth,
            depth,
            depth,
            warmups,
            samples,
            measurement,
            mapping_lifetime,
            prime_batches,
            forward_p50,
            forward_p95,
            bytes_per_round as f64 / forward_p50 as f64,
            reverse_p50,
            reverse_p95,
            bytes_per_round as f64 / reverse_p50 as f64,
            diagnostic_label(mode == ProgressModeV1::Diagnostic),
        );
    }
    Ok(())
}

fn guarded_bytes(copy_bytes: usize, outer: u8, inner: u8) -> BenchmarkResult<Vec<u8>> {
    let total = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .ok_or("XGMI allocation size overflow")?;
    let mut bytes = vec![outer; total];
    bytes[CANARY_BYTES..CANARY_BYTES + copy_bytes].fill(inner);
    Ok(bytes)
}

fn validate_guarded_bytes(
    observed: &[u8],
    copy_bytes: usize,
    outer: u8,
    inner: u8,
) -> BenchmarkResult<()> {
    let end = CANARY_BYTES
        .checked_add(copy_bytes)
        .ok_or("XGMI validation range overflow")?;
    let total = end
        .checked_add(CANARY_BYTES)
        .ok_or("XGMI validation size overflow")?;
    if observed.len() != total
        || !observed[..CANARY_BYTES].iter().all(|byte| *byte == outer)
        || !observed[CANARY_BYTES..end]
            .iter()
            .all(|byte| *byte == inner)
        || !observed[end..].iter().all(|byte| *byte == outer)
    {
        return Err("XGMI payload or canary mismatch".into());
    }
    Ok(())
}

fn allocate_direction(
    context: &mut XgmiContextV1,
    source_device: RuntimeDeviceIdV1,
    destination_device: RuntimeDeviceIdV1,
    total_bytes: u64,
    depth: usize,
) -> BenchmarkResult<DirectionResourcesV1> {
    let mut sources = Vec::with_capacity(depth);
    let mut destinations = Vec::with_capacity(depth);
    let mut streams = Vec::with_capacity(depth);
    for _ in 0..depth {
        sources.push(
            context
                .allocate(
                    source_device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    total_bytes,
                    4096,
                )
                .map_err(facade_error)?,
        );
        destinations.push(
            context
                .allocate(
                    destination_device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    total_bytes,
                    4096,
                )
                .map_err(facade_error)?,
        );
        streams.push(
            context
                .create_stream(destination_device)
                .map_err(facade_error)?,
        );
    }
    Ok(DirectionResourcesV1 {
        sources,
        destinations,
        streams,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_direction(
    context: &mut XgmiContextV1,
    resources: &DirectionResourcesV1,
    copy_bytes: usize,
    round: usize,
    direction: usize,
    source_canary: u8,
    destination_canary: u8,
) -> BenchmarkResult<()> {
    for slot in 0..resources.sources.len() {
        let value = pattern(round, slot, direction);
        let source = guarded_bytes(copy_bytes, source_canary, value)?;
        let destination = guarded_bytes(copy_bytes, destination_canary, value ^ 0xff)?;
        context
            .write_allocation(resources.sources[slot], 0, &source)
            .map_err(facade_error)?;
        context
            .write_allocation(resources.destinations[slot], 0, &destination)
            .map_err(facade_error)?;
    }
    Ok(())
}

fn run_direction(
    context: &mut XgmiContextV1,
    resources: &DirectionResourcesV1,
    copy_bytes: u64,
    aggregate: bool,
) -> BenchmarkResult<u128> {
    if aggregate {
        return run_direction_aggregate(context, resources, copy_bytes);
    }
    let mut submissions: Vec<RuntimeSubmissionV1<RuntimePeerCopyV1>> =
        Vec::with_capacity(resources.sources.len());
    let start = Instant::now();
    for slot in 0..resources.sources.len() {
        submissions.push(
            context
                .peer_copy(
                    resources.streams[slot],
                    RuntimeMemoryRegionV1 {
                        allocation: resources.sources[slot],
                        access: RuntimeAccessV1::Read,
                        byte_offset: CANARY_BYTES as u64,
                        byte_len: copy_bytes,
                    },
                    RuntimeMemoryRegionV1 {
                        allocation: resources.destinations[slot],
                        access: RuntimeAccessV1::Write,
                        byte_offset: CANARY_BYTES as u64,
                        byte_len: copy_bytes,
                    },
                    &[],
                )
                .map_err(facade_error)?,
        );
    }
    context
        .flush_stream(resources.streams[0])
        .map_err(facade_error)?;
    for submission in &mut submissions {
        let status = context
            .wait(submission, COMPLETION_TIMEOUT)
            .map_err(facade_error)?;
        if status != RuntimePollV1::Succeeded {
            return Err(format!("XGMI peer copy did not succeed: {status:?}").into());
        }
    }
    let elapsed = start.elapsed().as_nanos();
    for submission in submissions {
        context
            .release_submission(submission)
            .map_err(facade_error)?;
    }
    Ok(elapsed)
}

fn run_direction_aggregate(
    context: &mut XgmiContextV1,
    resources: &DirectionResourcesV1,
    copy_bytes: u64,
) -> BenchmarkResult<u128> {
    let mut submissions: Vec<RuntimeSubmissionV1<RuntimePeerCopyV1>> =
        Vec::with_capacity(resources.sources.len());
    let mut aggregate_submissions: Vec<&mut RuntimeSubmissionV1<RuntimePeerCopyV1>> =
        Vec::with_capacity(resources.sources.len());
    let start = Instant::now();
    for slot in 0..resources.sources.len() {
        submissions.push(
            context
                .peer_copy(
                    resources.streams[slot],
                    RuntimeMemoryRegionV1 {
                        allocation: resources.sources[slot],
                        access: RuntimeAccessV1::Read,
                        byte_offset: CANARY_BYTES as u64,
                        byte_len: copy_bytes,
                    },
                    RuntimeMemoryRegionV1 {
                        allocation: resources.destinations[slot],
                        access: RuntimeAccessV1::Write,
                        byte_offset: CANARY_BYTES as u64,
                        byte_len: copy_bytes,
                    },
                    &[],
                )
                .map_err(facade_error)?,
        );
    }
    aggregate_submissions.extend(submissions.iter_mut());
    let status = context
        .wait_peer_copy_batch(&mut aggregate_submissions, COMPLETION_TIMEOUT)
        .map_err(facade_error)?;
    if status != RuntimePeerCopyBatchPollV1::Succeeded {
        return Err(format!("XGMI peer-copy aggregate did not succeed: {status:?}").into());
    }
    let elapsed = start.elapsed().as_nanos();
    drop(aggregate_submissions);
    for submission in submissions {
        context
            .release_submission(submission)
            .map_err(facade_error)?;
    }
    Ok(elapsed)
}

#[allow(clippy::too_many_arguments)]
fn validate_direction(
    context: &mut XgmiContextV1,
    resources: &DirectionResourcesV1,
    copy_bytes: usize,
    round: usize,
    direction: usize,
    source_canary: u8,
    destination_canary: u8,
) -> BenchmarkResult<()> {
    let total = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .ok_or("XGMI validation size overflow")?;
    let mut observed = vec![0_u8; total];
    for slot in 0..resources.sources.len() {
        let expected = pattern(round, slot, direction);
        context
            .read_allocation(resources.sources[slot], 0, &mut observed)
            .map_err(facade_error)?;
        validate_guarded_bytes(&observed, copy_bytes, source_canary, expected)?;
        observed.fill(0);
        context
            .read_allocation(resources.destinations[slot], 0, &mut observed)
            .map_err(facade_error)?;
        validate_guarded_bytes(&observed, copy_bytes, destination_canary, expected)?;
    }
    Ok(())
}

fn release_direction(
    context: &mut XgmiContextV1,
    resources: DirectionResourcesV1,
) -> BenchmarkResult<()> {
    for destination in resources.destinations.into_iter().rev() {
        context
            .release_allocation(destination)
            .map_err(facade_error)?;
    }
    for source in resources.sources.into_iter().rev() {
        context.release_allocation(source).map_err(facade_error)?;
    }
    for stream in resources.streams.into_iter().rev() {
        context.destroy_stream(stream).map_err(facade_error)?;
    }
    Ok(())
}

fn main() -> BenchmarkResult<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = progress_mode(&args)?;
    #[cfg(feature = "hardware-diagnostic")]
    let diagnostic = mode == ProgressModeV1::Diagnostic;
    #[cfg(feature = "hardware-diagnostic")]
    let aggregate_diagnostic = mode == ProgressModeV1::AggregatePeerBatchHotDiagnostic;
    let aggregate = is_aggregate_mode(mode);
    let unique_ids = [parse_unique_id(&args[0])?, parse_unique_id(&args[1])?];
    let copy_bytes: usize = args[2].parse()?;
    let depth: usize = args[3].parse()?;
    let warmups: usize = args[4].parse()?;
    let samples: usize = args[5].parse()?;
    if unique_ids[0] == unique_ids[1]
        || copy_bytes == 0
        || copy_bytes > fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize
        || !valid_depth(mode, depth)
        || samples == 0
    {
        return Err("XGMI benchmark controls are out of range".into());
    }
    let rounds = warmups.checked_add(samples).ok_or("round count overflow")?;
    #[cfg(feature = "hardware-diagnostic")]
    let diagnostic_submissions = if diagnostic {
        Some(diagnostic_submission_count(rounds, depth)?)
    } else {
        None
    };
    #[cfg(feature = "hardware-diagnostic")]
    let aggregate_diagnostic_calls = if aggregate_diagnostic {
        Some(aggregate_diagnostic_call_count(rounds, depth)?)
    } else {
        None
    };
    let total_bytes = copy_bytes
        .checked_add(2 * CANARY_BYTES)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or("XGMI allocation size overflow")?;

    let backend = KfdNativeXgmiRuntimeBackendV1::open_default(unique_ids[0], unique_ids[1])?;
    #[cfg(feature = "hardware-diagnostic")]
    let backend = {
        let mut backend = backend;
        if let Some(expected) = diagnostic_submissions {
            backend
                .enable_xgmi_copy_diagnostics_v1(expected, 40_000)
                .map_err(facade_error)?;
        }
        if let Some(expected) = aggregate_diagnostic_calls {
            backend
                .enable_xgmi_aggregate_diagnostics_v1(expected)
                .map_err(facade_error)?;
        }
        backend
    };
    let mut context = RuntimeContextV1::open(backend).map_err(facade_error)?;
    if context.devices().len() != 2
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err("native XGMI facade did not enumerate two gfx942:xnack- devices".into());
    }
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for device in devices {
        let capabilities = context.execution_capabilities(device)?;
        if !capabilities.native_peer_copy {
            return Err("native XGMI facade did not report native peer copy".into());
        }
    }

    let forward = allocate_direction(&mut context, devices[0], devices[1], total_bytes, depth)?;
    let reverse = allocate_direction(&mut context, devices[1], devices[0], total_bytes, depth)?;
    let remap_measurements = if includes_remap_phase(mode) {
        let mut forward_ns = Vec::with_capacity(samples);
        let mut reverse_ns = Vec::with_capacity(samples);
        for round in 0..rounds {
            prepare_direction(&mut context, &forward, copy_bytes, round, 0, 0x17, 0xa5)?;
            let elapsed = run_direction(&mut context, &forward, copy_bytes as u64, aggregate)?;
            validate_direction(&mut context, &forward, copy_bytes, round, 0, 0x17, 0xa5)?;
            if round >= warmups {
                forward_ns.push(elapsed);
            }

            prepare_direction(&mut context, &reverse, copy_bytes, round, 1, 0x71, 0x5a)?;
            let elapsed = run_direction(&mut context, &reverse, copy_bytes as u64, aggregate)?;
            validate_direction(&mut context, &reverse, copy_bytes, round, 1, 0x71, 0x5a)?;
            if round >= warmups {
                reverse_ns.push(elapsed);
            }
        }
        Some((forward_ns, reverse_ns))
    } else {
        None
    };

    // Establish one mapped, completed batch in each direction, then time only
    // repetitions with no intervening host access. Final readback validates the
    // exact payload and canaries after the entire persistent-hot sequence.
    let hot_pattern_round = rounds.checked_add(1).ok_or("hot pattern round overflow")?;
    prepare_direction(
        &mut context,
        &forward,
        copy_bytes,
        hot_pattern_round,
        0,
        0x17,
        0xa5,
    )?;
    prepare_direction(
        &mut context,
        &reverse,
        copy_bytes,
        hot_pattern_round,
        1,
        0x71,
        0x5a,
    )?;
    let _ = run_direction(&mut context, &forward, copy_bytes as u64, aggregate)?;
    let _ = run_direction(&mut context, &reverse, copy_bytes as u64, aggregate)?;
    let mut hot_forward_ns = Vec::with_capacity(samples);
    let mut hot_reverse_ns = Vec::with_capacity(samples);
    for round in 0..rounds {
        let forward_elapsed = run_direction(&mut context, &forward, copy_bytes as u64, aggregate)?;
        let reverse_elapsed = run_direction(&mut context, &reverse, copy_bytes as u64, aggregate)?;
        if round >= warmups {
            hot_forward_ns.push(forward_elapsed);
            hot_reverse_ns.push(reverse_elapsed);
        }
    }
    validate_direction(
        &mut context,
        &forward,
        copy_bytes,
        hot_pattern_round,
        0,
        0x17,
        0xa5,
    )?;
    validate_direction(
        &mut context,
        &reverse,
        copy_bytes,
        hot_pattern_round,
        1,
        0x71,
        0x5a,
    )?;

    release_direction(&mut context, reverse)?;
    release_direction(&mut context, forward)?;
    let mut backend = context.shutdown().map_err(facade_error)?;
    backend.shutdown_native_v1().map_err(facade_error)?;

    #[cfg(feature = "hardware-diagnostic")]
    if diagnostic {
        let records = backend
            .finish_xgmi_copy_diagnostics_v1()
            .map_err(facade_error)?;
        for (index, record) in records.into_iter().enumerate() {
            let call = match record.call {
                fe2o3_runtime::KfdRuntimeXgmiDiagnosticCallV1::Submit => "submit",
                fe2o3_runtime::KfdRuntimeXgmiDiagnosticCallV1::Pending => "pending",
                fe2o3_runtime::KfdRuntimeXgmiDiagnosticCallV1::Completed => "completed",
            };
            let ns = |value: Option<u64>| {
                value.map_or_else(|| "unavailable".to_owned(), |n| n.to_string())
            };
            let preparation = record
                .native
                .preparation_ns
                .map_or_else(|| "not-applicable".to_owned(), |n| n.to_string());
            println!(
                "schema=fe2o3.xgmi-host-attribution.v1 backend=kfd ordinal={} backend_submission={} source_uid={:016x} destination_uid={:016x} call={} opening_currentness_ns={} preparation_ns={} native_call_ns={} closing_currentness_ns={} total_ns={} authority=none teardown=explicit",
                index,
                record.backend_submission,
                record.source_device,
                record.destination_device,
                call,
                ns(record.native.opening_currentness_ns),
                preparation,
                ns(record.native.native_call_ns),
                ns(record.native.closing_currentness_ns),
                ns(record.native.total_ns),
            );
        }
    }

    #[cfg(feature = "hardware-diagnostic")]
    if aggregate_diagnostic {
        let records = backend
            .finish_xgmi_aggregate_diagnostics_v1()
            .map_err(facade_error)?;
        for (index, record) in records.into_iter().enumerate() {
            let ns = |value: Option<u64>| {
                value.map_or_else(|| "unavailable".to_owned(), |n| n.to_string())
            };
            println!(
                "schema=fe2o3.xgmi-aggregate-host-attribution.v1 backend=kfd ordinal={} backend_submission={} source_uid={:016x} destination_uid={:016x} admission_validation_ns={} preparation_ns={} opening_currentness_ns={} submission_ns={} wait_ns={} closing_currentness_ns={} settlement_ns={} total_ns={} authority=none teardown=explicit timing=backend-aggregate-progress-host-only",
                index,
                record.backend_submission,
                record.source_device,
                record.destination_device,
                ns(record.host.admission_validation_ns),
                ns(record.host.preparation_ns),
                ns(record.host.opening_currentness_ns),
                ns(record.host.submission_ns),
                ns(record.host.wait_ns),
                ns(record.host.closing_currentness_ns),
                ns(record.host.settlement_ns),
                ns(record.host.total_ns),
            );
        }
    }

    if let Some((remap_forward_ns, remap_reverse_ns)) = remap_measurements {
        report_measurement(
            unique_ids,
            copy_bytes,
            depth,
            warmups,
            samples,
            "remap-per-round",
            "host-access-between-rounds",
            0,
            mode,
            remap_forward_ns,
            remap_reverse_ns,
        )?;
    }
    report_measurement(
        unique_ids,
        copy_bytes,
        depth,
        warmups,
        samples,
        "persistent-hot",
        "persistent-no-host-access-between-timed-rounds",
        1,
        mode,
        hot_forward_ns,
        hot_reverse_ns,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canaries_bind_the_exact_inner_copy_region() {
        let bytes = guarded_bytes(17, 0xa5, 0x3c).unwrap();
        validate_guarded_bytes(&bytes, 17, 0xa5, 0x3c).unwrap();
        let mut prefix = bytes.clone();
        prefix[0] ^= 1;
        assert!(validate_guarded_bytes(&prefix, 17, 0xa5, 0x3c).is_err());
        let mut payload = bytes.clone();
        payload[CANARY_BYTES + 8] ^= 1;
        assert!(validate_guarded_bytes(&payload, 17, 0xa5, 0x3c).is_err());
        let mut suffix = bytes;
        *suffix.last_mut().unwrap() ^= 1;
        assert!(validate_guarded_bytes(&suffix, 17, 0xa5, 0x3c).is_err());
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        let values = [10, 20, 30, 40, 50];
        assert_eq!(percentile(&values, 1, 2), Some(30));
        assert_eq!(percentile(&values, 19, 20), Some(50));
        assert_eq!(percentile(&[], 1, 2), None);
        assert_eq!(percentile(&values, 0, 2), None);
    }

    #[test]
    fn patterns_distinguish_round_slot_and_direction() {
        assert_ne!(pattern(0, 0, 0), pattern(1, 0, 0));
        assert_ne!(pattern(0, 0, 0), pattern(0, 1, 0));
        assert_ne!(pattern(0, 0, 0), pattern(0, 0, 1));
    }

    #[test]
    fn progress_flags_are_explicit_and_mutually_exclusive_before_native_open() {
        let mut args = vec![String::new(); 6];
        assert_eq!(progress_mode(&args).unwrap(), ProgressModeV1::Ordinary);
        assert_eq!(diagnostic_label(false), "");
        args.push("--diagnose-xgmi".into());
        assert_eq!(
            progress_mode(&args).is_ok(),
            cfg!(feature = "hardware-diagnostic")
        );
        assert_eq!(diagnostic_label(true), " diagnostic=xgmi-host-stages-v1");
        args[6] = "--unknown".into();
        assert!(progress_mode(&args).is_err());
        args[6] = "--aggregate-peer-batch".into();
        assert_eq!(
            progress_mode(&args).unwrap(),
            ProgressModeV1::AggregatePeerBatch
        );
        args[6] = "--aggregate-peer-batch-hot-only".into();
        assert_eq!(
            progress_mode(&args).unwrap(),
            ProgressModeV1::AggregatePeerBatchHotOnly
        );
        args[6] = "--aggregate-peer-batch-hot-diagnose".into();
        let aggregate_diagnostic = progress_mode(&args);
        if cfg!(feature = "hardware-diagnostic") {
            assert_eq!(
                aggregate_diagnostic.unwrap(),
                ProgressModeV1::AggregatePeerBatchHotDiagnostic
            );
        } else {
            assert_eq!(
                aggregate_diagnostic.unwrap_err().to_string(),
                "--aggregate-peer-batch-hot-diagnose requires the hardware-diagnostic feature"
            );
        }
        args.push("--diagnose-xgmi".into());
        assert!(progress_mode(&args).is_err());
        args.swap(6, 7);
        assert!(progress_mode(&args).is_err());
        args.truncate(5);
        assert!(progress_mode(&args).is_err());
    }

    #[test]
    fn aggregate_and_ordinary_depth_bounds_are_distinct() {
        assert!(valid_depth(ProgressModeV1::Ordinary, 1));
        assert!(valid_depth(ProgressModeV1::Ordinary, MAX_DEPTH));
        assert!(!valid_depth(ProgressModeV1::Ordinary, 0));
        assert!(!valid_depth(ProgressModeV1::Ordinary, MAX_DEPTH + 1));
        assert!(valid_depth(ProgressModeV1::Diagnostic, MAX_DEPTH));
        assert!(valid_depth(ProgressModeV1::AggregatePeerBatch, 1));
        assert!(valid_depth(
            ProgressModeV1::AggregatePeerBatch,
            MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1
        ));
        assert_eq!(MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1, 63);
        assert!(!valid_depth(ProgressModeV1::AggregatePeerBatch, 0));
        assert!(!valid_depth(
            ProgressModeV1::AggregatePeerBatch,
            MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1 + 1
        ));
    }

    #[test]
    fn aggregate_hot_only_requires_depth_one() {
        for mode in [
            ProgressModeV1::AggregatePeerBatchHotOnly,
            ProgressModeV1::AggregatePeerBatchHotDiagnostic,
        ] {
            assert!(valid_depth(mode, 1));
            assert!(!valid_depth(mode, 0));
            assert!(!valid_depth(mode, 2));
        }
    }

    #[test]
    fn aggregate_classification_includes_all_aggregate_modes() {
        assert!(!is_aggregate_mode(ProgressModeV1::Ordinary));
        assert!(!is_aggregate_mode(ProgressModeV1::Diagnostic));
        assert!(is_aggregate_mode(ProgressModeV1::AggregatePeerBatch));
        assert!(is_aggregate_mode(ProgressModeV1::AggregatePeerBatchHotOnly));
        assert!(is_aggregate_mode(
            ProgressModeV1::AggregatePeerBatchHotDiagnostic
        ));
    }

    #[test]
    fn existing_modes_preserve_both_phases_and_report_schemas() {
        for mode in [
            ProgressModeV1::Ordinary,
            ProgressModeV1::Diagnostic,
            ProgressModeV1::AggregatePeerBatch,
        ] {
            assert!(includes_remap_phase(mode));
        }
        assert_eq!(
            report_schema(ProgressModeV1::Ordinary),
            "fe2o3.xgmi-peer-benchmark.v1"
        );
        assert_eq!(
            report_schema(ProgressModeV1::Diagnostic),
            "fe2o3.xgmi-peer-benchmark.v1"
        );
        assert_eq!(
            report_schema(ProgressModeV1::AggregatePeerBatch),
            "fe2o3.xgmi-peer-aggregate-benchmark.v1"
        );
        assert!(!includes_remap_phase(
            ProgressModeV1::AggregatePeerBatchHotOnly
        ));
        assert_eq!(
            report_schema(ProgressModeV1::AggregatePeerBatchHotOnly),
            "fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1"
        );
        assert!(!includes_remap_phase(
            ProgressModeV1::AggregatePeerBatchHotDiagnostic
        ));
        assert_eq!(
            report_schema(ProgressModeV1::AggregatePeerBatchHotDiagnostic),
            "fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1"
        );
        for mode in [
            ProgressModeV1::Ordinary,
            ProgressModeV1::Diagnostic,
            ProgressModeV1::AggregatePeerBatch,
            ProgressModeV1::AggregatePeerBatchHotOnly,
        ] {
            assert_eq!(aggregate_diagnostic_label(mode), "");
        }
        assert_eq!(
            aggregate_diagnostic_label(ProgressModeV1::AggregatePeerBatchHotDiagnostic),
            " diagnostic=aggregate-host-attribution"
        );
    }

    #[test]
    fn diagnostic_submission_roster_is_bounded_before_native_open() {
        assert_eq!(diagnostic_submission_count(40, 1).unwrap(), 162);
        assert_eq!(diagnostic_submission_count(1, 1).unwrap(), 6);
        assert_eq!(diagnostic_submission_count(4999, 1).unwrap(), 19998);
        for (rounds, depth) in [(40, 0), (40, 2), (5000, 1), (usize::MAX, 1)] {
            assert!(diagnostic_submission_count(rounds, depth).is_err());
        }
    }

    #[test]
    fn aggregate_diagnostic_call_roster_is_bounded_before_native_open() {
        assert_eq!(aggregate_diagnostic_call_count(40, 1).unwrap(), 82);
        assert_eq!(aggregate_diagnostic_call_count(1, 1).unwrap(), 4);
        assert_eq!(aggregate_diagnostic_call_count(19_999, 1).unwrap(), 40_000);
        for (rounds, depth) in [(40, 0), (40, 2), (20_000, 1), (usize::MAX, 1)] {
            assert!(aggregate_diagnostic_call_count(rounds, depth).is_err());
        }
    }
}
