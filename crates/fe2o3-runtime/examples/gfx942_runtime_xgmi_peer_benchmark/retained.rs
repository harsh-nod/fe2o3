//! One retained Context scope per batch, including entry and finish in timing.

use super::*;
use fe2o3_kfd::{
    GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1, GFX942_XGMI_RETAINED_PAIR_PROFILE_V1,
    Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
};
use fe2o3_runtime::RuntimeNativeRetainedPeerCopyPollV1;

pub(super) const SCHEMA: &str = "fe2o3.xgmi-peer-retained-facade-benchmark.v1";
const MAX_ROUNDS: usize = 10_000;

pub(super) fn scope_count(warmups: usize, samples: usize) -> BenchmarkResult<usize> {
    warmups
        .checked_add(samples)
        .filter(|rounds| samples != 0 && *rounds <= MAX_ROUNDS)
        .and_then(|rounds| rounds.checked_add(1))
        .and_then(|rounds| rounds.checked_mul(2))
        .ok_or_else(|| {
            "retained facade requires 1..=10000 warmup/sample rounds and nonzero samples".into()
        })
}

pub(super) fn complete(
    context: &mut XgmiContextV1,
    submissions: &mut [&mut RuntimeSubmissionV1<RuntimePeerCopyV1>],
) -> BenchmarkResult<()> {
    let mut batch = context
        .begin_retained_peer_copy_batch_v1(
            submissions,
            COMPLETION_TIMEOUT,
            Gfx942XgmiRetainedPairEnvironmentAssumptionV1::ReviewedMi300xAmdgpu61613OrdinaryLifetime,
        )
        .map_err(facade_error)?;
    if batch.profile() != GFX942_XGMI_RETAINED_PAIR_PROFILE_V1 {
        return Err("unexpected retained XGMI profile".into());
    }
    if batch.wait().map_err(facade_error)? != RuntimeNativeRetainedPeerCopyPollV1::ReadyToFinish {
        return Err("retained XGMI batch exceeded its fixed deadline".into());
    }
    batch.finish().map_err(facade_error)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn measurement_row(
    unique_ids: [u64; 2],
    copy_bytes: usize,
    depth: usize,
    warmups: usize,
    samples: usize,
    measurement: &str,
    mapping_lifetime: &str,
    prime_batches: usize,
    mut forward_ns: Vec<u128>,
    mut reverse_ns: Vec<u128>,
) -> BenchmarkResult<String> {
    if unique_ids.contains(&0)
        || unique_ids[0] == unique_ids[1]
        || !valid_depth(ProgressModeV1::RetainedPeerBatch, depth)
        || copy_bytes == 0
        || copy_bytes > fe2o3_kfd::GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize
        || samples == 0
        || forward_ns.len() != samples
        || reverse_ns.len() != samples
        || forward_ns.contains(&0)
        || reverse_ns.contains(&0)
        || measurement != "persistent-hot"
        || mapping_lifetime != "persistent-no-host-access-between-timed-rounds"
        || prime_batches != 1
    {
        return Err("retained facade measurement population mismatch".into());
    }
    let scopes = scope_count(warmups, samples)?;
    let bytes_per_batch = copy_bytes
        .checked_mul(depth)
        .ok_or("batch bytes overflow")?;
    forward_ns.sort_unstable();
    reverse_ns.sort_unstable();
    let forward_p50 = percentile(&forward_ns, 1, 2).ok_or("missing forward p50")?;
    let forward_p95 = percentile(&forward_ns, 19, 20).ok_or("missing forward p95")?;
    let reverse_p50 = percentile(&reverse_ns, 1, 2).ok_or("missing reverse p50")?;
    let reverse_p95 = percentile(&reverse_ns, 19, 20).ok_or("missing reverse p95")?;
    let population = |values: &[u128]| {
        values
            .iter()
            .map(u128::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    Ok(format!(
        "backend=kfd schema={SCHEMA} surface=runtime-facade qualification_profile={} qualification_policy_sha256={} environment_assumption=reviewed-mi300x-amdgpu61613-ordinary-lifetime unique_ids={:016x},{:016x} target=gfx942:xnack- bytes={copy_bytes} depth={depth} queue_depth={depth} batch_size={depth} outstanding_depth={depth} direction=forward-then-reverse engine_parallelism=ordered-single-sdma warmups={warmups} samples={samples} measurement={measurement} peer_access=topology-xgmi mapping_lifetime={mapping_lifetime} prime_batches={prime_batches} scopes={scopes} batches_per_scope=1 doorbells_per_batch=1 progress=explicit-retained-wait-then-finish background_progress=false forward_engine=topology-selected reverse_engine=topology-selected timing=facade-enqueue-through-retained-finish lifetime_setup=inside-samples lifetime_finish=inside-samples logical_settlement=inside-samples submission_release=outside-samples operational_fences=inside-samples validation_scope=final-payload-and-canaries-only sample_order=sorted forward_samples_ns={} reverse_samples_ns={} forward_p50_ns={forward_p50} forward_p95_ns={forward_p95} forward_p50_GBps={:.3} reverse_p50_ns={reverse_p50} reverse_p95_ns={reverse_p95} reverse_p50_GBps={:.3} canaries=pass teardown=explicit",
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1,
        GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1,
        unique_ids[0],
        unique_ids[1],
        population(&forward_ns),
        population(&reverse_ns),
        bytes_per_batch as f64 / forward_p50 as f64,
        bytes_per_batch as f64 / reverse_p50 as f64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(forward: Vec<u128>, reverse: Vec<u128>) -> BenchmarkResult<String> {
        measurement_row(
            [1, 2],
            1024,
            16,
            2,
            2,
            "persistent-hot",
            "persistent-no-host-access-between-timed-rounds",
            1,
            forward,
            reverse,
        )
    }

    #[test]
    fn flag_requires_explicit_exclusive_opt_in_and_keeps_a_distinct_schema() {
        let mut args = vec![String::new(); 6];
        assert_eq!(progress_mode(&args).unwrap(), ProgressModeV1::Ordinary);
        args.push("--retained-peer-batch-reviewed-mi300x".into());
        let mode = progress_mode(&args).unwrap();
        assert_eq!(mode, ProgressModeV1::RetainedPeerBatch);
        assert!(!is_aggregate_mode(mode));
        assert!(!includes_remap_phase(mode));
        assert_eq!(report_schema(mode), SCHEMA);
        for depth in [1, 16, 32] {
            assert!(valid_depth(mode, depth));
        }
        for depth in [0, 33, usize::MAX] {
            assert!(!valid_depth(mode, depth));
        }
        for other in [
            "--aggregate-peer-batch",
            "--diagnose-xgmi",
            "--retained-peer-batch-reviewed-mi300x",
        ] {
            args.push(other.into());
            assert!(progress_mode(&args).is_err());
            args.swap(6, 7);
            assert!(progress_mode(&args).is_err());
            args.swap(6, 7);
            args.pop();
        }
    }

    #[test]
    fn scope_budget_rejects_zero_excessive_and_overflowing_rounds_before_native_open() {
        assert_eq!(scope_count(0, 1).unwrap(), 4);
        assert_eq!(scope_count(2, 2).unwrap(), 10);
        assert_eq!(
            scope_count(MAX_ROUNDS - 1, 1).unwrap(),
            2 * (MAX_ROUNDS + 1)
        );
        for (warmups, samples) in [
            (0, 0),
            (1, 0),
            (MAX_ROUNDS, 1),
            (0, MAX_ROUNDS + 1),
            (usize::MAX, 1),
            (usize::MAX - 1, 1),
            (usize::MAX / 2, 1),
        ] {
            assert!(scope_count(warmups, samples).is_err());
        }
        let parent = include_str!("../gfx942-runtime-xgmi-peer-benchmark.rs");
        let main = parent
            .split("fn main()")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let open = main
            .find("KfdNativeXgmiRuntimeBackendV1::open_default")
            .unwrap();
        assert!(main.find("let hot_pattern_round =").unwrap() < open);
        assert!(
            main.find("retained::scope_count(warmups, samples)?")
                .unwrap()
                < open
        );
    }

    #[test]
    fn rows_bind_complete_population_and_end_to_end_timing() {
        let row = row(vec![20, 10], vec![40, 30]).unwrap();
        for field in [
            format!("schema={SCHEMA}"),
            format!("qualification_profile={GFX942_XGMI_RETAINED_PAIR_PROFILE_V1}"),
            format!("qualification_policy_sha256={GFX942_XGMI_RETAINED_PAIR_POLICY_SHA256_V1}"),
        ] {
            assert!(row.split_whitespace().any(|value| value == field));
        }
        for field in [
            "scopes=10",
            "batches_per_scope=1",
            "surface=runtime-facade",
            "timing=facade-enqueue-through-retained-finish",
            "lifetime_setup=inside-samples",
            "lifetime_finish=inside-samples",
            "logical_settlement=inside-samples",
            "submission_release=outside-samples",
            "validation_scope=final-payload-and-canaries-only",
            "sample_order=sorted",
            "forward_samples_ns=10,20",
            "reverse_samples_ns=30,40",
            "forward_p50_ns=10",
            "forward_p95_ns=20",
            "reverse_p50_ns=30",
            "reverse_p95_ns=40",
        ] {
            assert!(
                row.split_whitespace().any(|value| value == field),
                "{field}"
            );
        }
    }

    #[test]
    fn rows_reject_incomplete_zero_or_unmatched_samples() {
        for (forward, reverse) in [
            (vec![], vec![1, 2]),
            (vec![1, 2], vec![1]),
            (vec![1, 2, 3], vec![1, 2]),
            (vec![0, 2], vec![1, 2]),
            (vec![1, 2], vec![1, 0]),
        ] {
            assert!(row(forward, reverse).is_err());
        }
        for (measurement, lifetime, primes) in [
            (
                "persistent-series",
                "persistent-no-host-access-between-timed-rounds",
                1,
            ),
            ("persistent-hot", "host-access-between-rounds", 1),
            (
                "persistent-hot",
                "persistent-no-host-access-between-timed-rounds",
                0,
            ),
        ] {
            assert!(
                measurement_row(
                    [1, 2],
                    1024,
                    1,
                    0,
                    1,
                    measurement,
                    lifetime,
                    primes,
                    vec![1],
                    vec![1]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn source_wiring_times_entry_publication_finish_before_handle_release() {
        let parent = include_str!("../gfx942-runtime-xgmi-peer-benchmark.rs");
        let body = parent
            .split("fn run_direction_batch(")
            .nth(1)
            .unwrap()
            .split("fn validate_direction(")
            .next()
            .unwrap();
        let clock = body.find("let start = Instant::now()").unwrap();
        let enqueue = body.find(".peer_copy(").unwrap();
        let complete = body
            .find("retained::complete(context, &mut aggregate_submissions)?")
            .unwrap();
        let elapsed = body.find("let elapsed = start.elapsed()").unwrap();
        let release = body.find(".release_submission(submission)").unwrap();
        assert!(clock < enqueue && enqueue < complete && complete < elapsed && elapsed < release);
        let source = include_str!("retained.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let begin = source.find(".begin_retained_peer_copy_batch_v1(").unwrap();
        let wait = source.find("batch.wait()").unwrap();
        let finish = source.find("batch.finish()").unwrap();
        assert!(begin < wait && wait < finish);
    }
}
