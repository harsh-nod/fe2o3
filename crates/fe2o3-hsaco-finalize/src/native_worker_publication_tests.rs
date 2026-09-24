use super::*;
use fe2o3_artifact_transaction::{BuildInvocation, BuildSession, begin_build_attempt};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn attempt(generation: u64, session: u8, invocation: u8) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        format!("{session:02x}").repeat(16),
        format!("{invocation:02x}").repeat(32)
    ))
    .unwrap()
}

fn inputs() -> NativePlanInputs {
    NativePlanInputs {
        package: PackageIdentityV1::from_bytes([1; 32]),
        attempt: attempt(1, 2, 3),
        slot: 0,
        transaction: [4; 32],
        outer: ContentIdentityV1::from_parts([5; 32], 6),
        binding: [7; 32],
        source: [8; 32],
        worker: [22; 32],
        finalized: [9; 32],
        transcript: [10; 32],
        link_plan: [11; 32],
        manifest: ContentIdentityV1::from_parts([12; 32], 13),
        policy: [14; 32],
        raw: ContentIdentityV1::from_parts([15; 32], 16),
        output: ContentIdentityV1::from_parts([17; 32], 18),
        descriptor: ContentIdentityV1::from_parts([19; 32], 20),
        canonical_digest: [21; 32],
    }
}

#[test]
fn native_plan_is_deterministic_and_preserves_raw_output_and_attempt_coordinates() {
    let expected = inputs();
    let intent = derive_plan(inputs());
    assert_eq!(derive_plan(inputs()), intent);
    assert_eq!(intent.durable_plan().attempt(), expected.attempt);
    assert_eq!(intent.durable_plan().scope().package(), expected.package);
    assert_eq!(
        intent.durable_plan().linked_output().as_bytes(),
        expected.raw.sha256()
    );
    assert_eq!(
        intent.durable_plan().finalized_output().as_bytes(),
        expected.output.sha256()
    );
    assert!(!intent.grants_publication_authority());
    assert!(!intent.grants_load_authority());
    assert!(!intent.grants_launch_authority());
}

#[test]
fn every_native_source_worker_finalizer_producer_and_attempt_axis_binds_all_three_domains() {
    let original = derive_plan(inputs());
    let mutations: &[fn(&mut NativePlanInputs)] = &[
        |x| x.package = PackageIdentityV1::from_bytes([91; 32]),
        |x| x.attempt = attempt(2, 2, 3),
        |x| x.attempt = attempt(1, 4, 3),
        |x| x.attempt = attempt(1, 2, 4),
        |x| x.slot = 1,
        |x| x.transaction[0] ^= 1,
        |x| x.outer = ContentIdentityV1::from_parts([95; 32], x.outer.byte_len()),
        |x| x.outer = ContentIdentityV1::from_parts(*x.outer.sha256(), 99),
        |x| x.binding[0] ^= 1,
        |x| x.source[0] ^= 1,
        |x| x.worker[0] ^= 1,
        |x| x.finalized[0] ^= 1,
        |x| x.transcript[0] ^= 1,
        |x| x.link_plan[0] ^= 1,
        |x| x.manifest = ContentIdentityV1::from_parts([95; 32], x.manifest.byte_len()),
        |x| x.manifest = ContentIdentityV1::from_parts(*x.manifest.sha256(), 99),
        |x| x.policy[0] ^= 1,
        |x| x.raw = ContentIdentityV1::from_parts([95; 32], x.raw.byte_len()),
        |x| x.raw = ContentIdentityV1::from_parts(*x.raw.sha256(), 99),
        |x| x.output = ContentIdentityV1::from_parts([95; 32], x.output.byte_len()),
        |x| x.output = ContentIdentityV1::from_parts(*x.output.sha256(), 99),
        |x| x.descriptor = ContentIdentityV1::from_parts([95; 32], x.descriptor.byte_len()),
        |x| x.descriptor = ContentIdentityV1::from_parts(*x.descriptor.sha256(), 99),
        |x| x.canonical_digest[0] ^= 1,
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut changed = inputs();
        mutate(&mut changed);
        let changed = derive_plan(changed);
        assert_ne!(
            changed.durable_plan().request(),
            original.durable_plan().request(),
            "axis {index}"
        );
        assert_ne!(
            changed.plan_identity(),
            original.plan_identity(),
            "axis {index}"
        );
        assert_ne!(changed.identity(), original.identity(), "axis {index}");
    }
}

#[test]
fn native_domains_are_distinct_and_not_the_legacy_semantic_domains() {
    let domains = [
        CONTEXT_DOMAIN,
        REQUEST_DOMAIN,
        PLAN_DOMAIN,
        INTENT_DOMAIN,
        KERNEL_DOMAIN,
        TARGET_DOMAIN,
        WORKER_DOMAIN,
        RESPONSE_DOMAIN,
        FINALIZATION_DOMAIN,
        PUBLICATION_DOMAIN,
    ];
    for (i, domain) in domains.iter().enumerate() {
        for other in &domains[..i] {
            assert_ne!(domain, other);
            assert_ne!(
                hash_parts(domain, &[&[7; 32]]),
                hash_parts(other, &[&[7; 32]])
            );
        }
    }
    let legacy = b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-WORKER-V3-PUBLICATION-REQUEST/V1\0";
    assert_ne!(
        hash_parts(REQUEST_DOMAIN, &[&[7; 32]]),
        hash_parts(legacy, &[&[7; 32]])
    );
}

#[test]
fn scope_and_worker_remain_stable_while_occurrence_bound_identities_change() {
    let original = derive_plan(inputs());
    let mut changed = inputs();
    changed.attempt = attempt(2, 2, 3);
    let changed = derive_plan(changed);
    assert_eq!(
        original.durable_plan().scope(),
        changed.durable_plan().scope()
    );
    assert_eq!(
        original.durable_plan().worker(),
        changed.durable_plan().worker()
    );
    assert_ne!(
        original.durable_plan().request(),
        changed.durable_plan().request()
    );
    assert_ne!(original.plan_identity(), changed.plan_identity());
    assert_ne!(original.identity(), changed.identity());
}

#[test]
fn shared_storage_caps_are_never_enlarged_to_native_codec_or_outer_caps() {
    assert!(
        check_shape(
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
            Some(MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1),
            MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1
        )
        .is_ok()
    );
    let bad_shapes = [
        (0, 0, 0, Some(1), 1),
        (MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 + 1, 0, 0, Some(1), 1),
        (
            1,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 + 1,
            128,
            Some(1),
            1,
        ),
        (
            1,
            1,
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 + 1,
            Some(1),
            1,
        ),
        (1, 0, 1, Some(1), 1),
        (1, 1, 0, Some(1), 1),
        (1, 0, 0, Some(0), 1),
        (
            1,
            0,
            0,
            Some(MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 + 1),
            1,
        ),
        (1, 0, 0, Some(1), 0),
        (
            1,
            0,
            0,
            Some(1),
            MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1 + 1,
        ),
        (usize::MAX, 0, 0, Some(1), 1),
    ];
    for (outer, count, bytes, transcript, output) in bad_shapes {
        assert!(check_shape(outer, count, bytes, transcript, output).is_err());
    }
    assert!(check_length(crate::native_worker_compact_replay::MAX_NATIVE_WORKER_COMPACT_FINALIZER_REPLAY_BYTES_V1,
        MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1, "transcript").is_err());
}

#[test]
fn copy_refuses_logical_oversize_and_returns_exact_bounded_attachment() {
    assert!(matches!(
        copy_attachment(&[1, 2], 1, "test"),
        Err(Error::Limit("test"))
    ));
    assert!(copy_attachment(&[], 1, "test").is_err());
    let copied = copy_attachment(&[1, 2], 2, "test").unwrap();
    assert_eq!(copied, [1, 2]);
    assert_eq!(copied.capacity(), 2);
    assert!(matches!(
        add(usize::MAX, 1),
        Err(Error::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn shared_attachment_owner_still_rejects_excess_spare_capacity() {
    let mut transcript = Vec::with_capacity(MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1 + 1);
    transcript.push(1);
    assert!(WorkerV3FinalizerReplayAttachmentsV1::new(vec![1], Vec::new(), transcript).is_err());
    let providers = Vec::with_capacity(MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 + 1);
    assert!(WorkerV3FinalizerReplayAttachmentsV1::new(vec![1], providers, vec![1]).is_err());
}

#[test]
fn native_scope_restores_entry_storage_on_success_error_and_unwind() {
    for mode in 0..3 {
        let mut work = Work::new(ENTRY_WORK);
        let mut budget = Budget::new(&mut work, 7 + FRAME + 19);
        budget.reserve_storage(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped(&mut budget, 7, |budget| {
                budget.reserve_storage(19)?;
                match mode {
                    0 => Ok(23),
                    1 => Err(Error::Mismatch("test")),
                    _ => panic!("test unwind"),
                }
            })
        }));
        match mode {
            0 => assert_eq!(result.unwrap().unwrap(), 23),
            1 => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), 7);
        assert_eq!(budget.work(), ENTRY_WORK);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn native_scope_rejects_unpaid_floor_one_short_limits_and_oversized_native_cap() {
    for (work_limit, storage_limit, floor, expected_floor) in [
        (ENTRY_WORK, 7 + FRAME, 6, 7),
        (ENTRY_WORK - 1, 7 + FRAME, 7, 7),
        (ENTRY_WORK, 7 + FRAME - 1, 7, 7),
        (
            ENTRY_WORK,
            MAX_INERT_REFINED_FORWARDING_STORAGE_V1 + 1,
            7,
            7,
        ),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        assert!(
            scoped::<()>(&mut budget, expected_floor, |_| panic!(
                "must refuse before callback"
            ))
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-native-publication-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn inert_store() -> (Scratch, ProducerIdentity, BuildAttempt) {
    let scratch = Scratch::new();
    let producer = ProducerIdentity::from_codegen("native_restart_negative", None).unwrap();
    let attempt = begin_build_attempt(
        &scratch.0,
        &producer,
        BuildInvocation::from_bytes([3; 32]),
        BuildSession::from_bytes([2; 16]),
    )
    .unwrap();
    let output = b"inert artifact".to_vec();
    let mut input = inputs();
    input.package = producer_package_identity_v1(&producer);
    input.attempt = attempt;
    input.output = ContentIdentityV1::calculate(&output);
    let plan = derive_plan(input).durable_plan();
    let mut legacy_shaped_outer = vec![0; 256];
    legacy_shaped_outer[..8].copy_from_slice(b"F2O3IHV3");
    let attachments = WorkerV3FinalizerReplayAttachmentsV1::new(
        legacy_shaped_outer,
        Vec::new(),
        b"F2V3CFR3".to_vec(),
    )
    .unwrap();
    persist_worker_v3_publication_intent_v1(
        &scratch.0,
        &producer,
        attempt,
        plan,
        attachments,
        output,
    )
    .unwrap();
    (scratch, producer, attempt)
}

#[test]
fn opaque_journal_success_cannot_promote_legacy_shaped_bytes_to_native_recovery() {
    let (scratch, producer, attempt) = inert_store();
    let stored = recover_worker_v3_publication_intent_v1(&scratch.0, &producer, attempt).unwrap();
    check_record_inputs(&producer, attempt, &stored).unwrap();
    assert!(!stored.grants_publication_authority());
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, MAX_INERT_REFINED_FORWARDING_STORAGE_V1);
    budget.reserve_storage(19).unwrap();
    let error =
        recover_native_worker_hsaco_publication_v1(&scratch.0, &producer, attempt, &mut budget)
            .err()
            .unwrap();
    assert!(matches!(
        error,
        Error::Stage {
            phase: "V4 decode" | "V4 decode quote",
            ..
        }
    ));
    assert_eq!(budget.storage(), 19);
    assert!(budget.work() >= 2 * ENTRY_WORK);
}

#[test]
fn record_check_refuses_foreign_producer_and_attempt_before_native_replay() {
    let (scratch, producer, actual_attempt) = inert_store();
    let stored =
        recover_worker_v3_publication_intent_v1(&scratch.0, &producer, actual_attempt).unwrap();
    let foreign = ProducerIdentity::from_codegen("foreign_native_restart", None).unwrap();
    assert!(matches!(
        check_record_inputs(&foreign, actual_attempt, &stored),
        Err(Error::Mismatch("record producer/attempt"))
    ));
    assert!(matches!(
        check_record_inputs(&producer, attempt(2, 2, 3), &stored),
        Err(Error::Mismatch("record producer/attempt"))
    ));
}

#[test]
fn unavailable_journal_refusal_preserves_native_entry_and_work_history() {
    let scratch = Scratch::new();
    let producer = ProducerIdentity::from_codegen("missing_native_restart", None).unwrap();
    let mut work = Work::new(ENTRY_WORK);
    let mut budget = Budget::new(&mut work, 3 + FRAME);
    budget.reserve_storage(3).unwrap();
    let result = recover_native_worker_hsaco_publication_v1(
        &scratch.0,
        &producer,
        attempt(1, 2, 3),
        &mut budget,
    );
    assert!(result.is_err());
    assert_eq!(budget.storage(), 3);
    assert_eq!(budget.work(), ENTRY_WORK);
    assert!(size_of::<Error>() <= 128);
}
