//! Codec fixtures are inert coordinates, never fabricated finalizers or receipts.
use super::*;
use crate::{
    WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerMeasurementV1,
    first_build_worker_v3::OwnedWorkerV3ProviderReplayPartV1,
    worker_protocol_v2::WorkerResponseReplayMetadataV1,
    worker_v3_compact_finalizer_replay::ProtectedWorkerV3CompactFinalizerReplayV2,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::time::Duration;

const WORKER: &str = "worker-v1";
const LLVM: &str = "llvm-v1";
const OUTER: usize = 10 + 3 * 32;
const GENERATION: usize = OUTER + 40;
const SESSION: usize = GENERATION + 8;
const INVOCATION: usize = SESSION + 16;
const SLOT: usize = INVOCATION + 32;
const TRANSACTION: usize = SLOT + 1;
const LIMITS: usize = HEADER_BYTES + 40 + 1 + WORKER.len() + 1 + LLVM.len();
const PROVIDERS: usize = LIMITS + 28 + 8;

fn fixture(providers: usize, options: usize, derivation_grammar: bool) -> Vec<u8> {
    let worker =
        WorkerMeasurementV1::new(ContentIdentityV1::from_parts([4; 32], 1), WORKER, LLVM).unwrap();
    let limits = WorkerExecutionLimitsV1::new(Duration::from_secs(2), 4096, 1024).unwrap();
    let diagnostics = 0_u32.to_le_bytes();
    let metadata = WorkerResponseReplayMetadataV1::from_bodies(&diagnostics, None, None);
    let mut refs: Vec<_> = (0..providers)
        .map(|i| {
            let bytes = vec![(i + 1) as u8];
            OwnedWorkerV3ProviderReplayPartV1 {
                kind: WorkerInputKindV1::LlvmBitcode,
                identity: ContentIdentityV1::calculate(&bytes),
                bytes,
            }
        })
        .collect();
    refs.sort_by_key(|p| p.identity);
    let options: Vec<_> = (0..options)
        .map(|i| LinkOptionV1::new(format!("o{i:02}"), "v").unwrap())
        .collect();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    for tag in [1, 2, 3, 4] {
        bytes.extend_from_slice(&[tag; 32]);
    }
    bytes.extend_from_slice(&123_u64.to_le_bytes());
    bytes.extend_from_slice(&7_u64.to_le_bytes());
    bytes.extend_from_slice(&[5; 16]);
    bytes.extend_from_slice(&[6; 32]);
    bytes.push(0);
    bytes.extend_from_slice(&[7; 32]);
    assert_eq!(bytes.len(), HEADER_BYTES);
    encode_compact_replay_tail(
        &mut bytes,
        derivation_grammar,
        &worker,
        limits,
        4096,
        &refs,
        &options,
        metadata,
        metadata,
    )
    .unwrap();
    let checksum = hash(CHECKSUM_DOMAIN, &bytes);
    bytes.extend_from_slice(&checksum);
    bytes
}

fn reseal(bytes: &mut [u8]) {
    let n = bytes.len() - 32;
    let checksum = hash(CHECKSUM_DOMAIN, &bytes[..n]);
    bytes[n..].copy_from_slice(&checksum);
}

fn decode(bytes: &[u8]) -> Result<NativeWorkerCompactFinalizerReplayV1> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(bytes.len()).unwrap();
    NativeWorkerCompactFinalizerReplayV1::decode_canonical(bytes, &mut budget).map(|v| v.0)
}

#[test]
fn canonical_round_trip_preserves_every_coordinate_and_common_tail() {
    let bytes = fixture(1, 1, true);
    let value = decode(&bytes).unwrap();
    assert_eq!(value.canonical_bytes(), bytes);
    assert_eq!(value.expected_finalization_identity(), &[1; 32]);
    assert_eq!(value.source_evidence_identity(), &[2; 32]);
    assert_eq!(value.binding_identity(), &[3; 32]);
    assert_eq!(
        value.coordinates().outer_identity_coordinates(),
        ([4; 32], 123)
    );
    assert_eq!(value.attempt().generation(), 7);
    assert_eq!(value.attempt().session().as_bytes(), &[5; 16]);
    assert_eq!(value.attempt().invocation().as_bytes(), &[6; 32]);
    assert_eq!(
        value.handoff_slot(),
        CompilerModuleHandoffSlotV4::Production
    );
    assert_eq!(value.transaction_identity().as_bytes(), &[7; 32]);
    let tail: ProtectedWorkerV3CompactFinalizerReplayViewV2<'_> = value.replay_view();
    assert_eq!(tail.worker.worker_build_identity(), WORKER);
    assert_eq!(tail.worker.llvm_build_identity(), LLVM);
    assert_eq!(tail.bootstrap_output_bound, 4096);
    assert_eq!(tail.execution_limits.timeout(), Duration::from_secs(2));
    assert_eq!(tail.external_providers.len(), 1);
    assert_eq!(tail.link_options.len(), 1);
    assert_eq!(tail.bootstrap_metadata.diagnostics_body(), [0; 4]);
    assert_eq!(tail.replay_metadata.diagnostics_body(), [0; 4]);
    assert!(!value.authenticates_compiler_origin());
    assert!(!value.grants_publication_authority());
    assert!(!value.grants_load_authority());
    assert!(!value.grants_launch_authority());
    assert_eq!(decode(&bytes).unwrap().identity(), value.identity());
    let pointer = value.canonical_bytes().as_ptr();
    let moved = value.into_canonical_bytes();
    assert_eq!(moved.as_ptr(), pointer);
    assert_eq!(moved, bytes);
}

#[test]
fn candidate_and_replay_metadata_remain_distinct_exact_bodies() {
    let mut bytes = fixture(0, 0, true);
    let mut metadata = Vec::new();
    for diagnostic in [b"candidate".as_slice(), b"replay".as_slice()] {
        let body_length = 4 + 4 + diagnostic.len();
        metadata.extend_from_slice(&(body_length as u16).to_le_bytes());
        metadata.extend_from_slice(&1_u32.to_le_bytes());
        metadata.extend_from_slice(&(diagnostic.len() as u32).to_le_bytes());
        metadata.extend_from_slice(diagnostic);
        metadata.extend_from_slice(&0_u32.to_le_bytes());
        metadata.extend_from_slice(&0_u16.to_le_bytes());
    }
    drop(bytes.splice(PROVIDERS + 2..bytes.len() - 32, metadata));
    reseal(&mut bytes);
    let value = decode(&bytes).unwrap();
    let view = value.replay_view();
    assert_eq!(
        &view.bootstrap_metadata.diagnostics_body()[8..],
        b"candidate"
    );
    assert_eq!(&view.replay_metadata.diagnostics_body()[8..], b"replay");
    assert_eq!(value.canonical_bytes(), bytes);
}

#[test]
fn every_bound_coordinate_changes_transcript_identity_without_minting_authority() {
    let original = fixture(0, 0, true);
    let identity = decode(&original).unwrap().identity();
    for offset in [
        10,
        42,
        74,
        OUTER,
        OUTER + 32,
        GENERATION,
        SESSION,
        INVOCATION,
        TRANSACTION,
        HEADER_BYTES,
        LIMITS,
        LIMITS + 12,
        LIMITS + 20,
        LIMITS + 28,
    ] {
        let mut changed = original.clone();
        changed[offset] ^= 1;
        reseal(&mut changed);
        let changed = decode(&changed).unwrap();
        assert_ne!(changed.identity(), identity, "offset {offset}");
        assert!(!changed.grants_launch_authority());
    }
}

#[test]
fn rejects_wrong_magic_version_checksum_every_truncation_and_trailing_bytes() {
    let original = fixture(0, 0, true);
    for n in 0..original.len() {
        assert!(decode(&original[..n]).is_err(), "prefix {n}");
    }
    let mut bad = original.clone();
    bad[0] ^= 1;
    reseal(&mut bad);
    assert!(matches!(
        decode(&bad),
        Err(NativeWorkerCompactReplayErrorV1::Magic)
    ));
    bad = original.clone();
    bad[8] = 2;
    reseal(&mut bad);
    assert!(matches!(
        decode(&bad),
        Err(NativeWorkerCompactReplayErrorV1::Version)
    ));
    bad = original.clone();
    bad[10] ^= 1;
    assert!(matches!(
        decode(&bad),
        Err(NativeWorkerCompactReplayErrorV1::Checksum)
    ));
    bad = original.clone();
    bad.insert(bad.len() - 32, 0);
    reseal(&mut bad);
    assert!(matches!(
        decode(&bad),
        Err(NativeWorkerCompactReplayErrorV1::Tail(_))
    ));
}

#[test]
fn refuses_invalid_attempt_occurrence_and_retired_slots() {
    let original = fixture(0, 0, true);
    for range in [
        10..42,
        42..74,
        74..106,
        OUTER + 32..OUTER + 40,
        GENERATION..SESSION,
        SESSION..INVOCATION,
        INVOCATION..SLOT,
        TRANSACTION..HEADER_BYTES,
    ] {
        let mut bad = original.clone();
        bad[range].fill(0);
        reseal(&mut bad);
        assert!(matches!(
            decode(&bad),
            Err(NativeWorkerCompactReplayErrorV1::Coordinates)
        ));
    }
    for slot in [1, 2, 255] {
        let mut bad = original.clone();
        bad[SLOT] = slot;
        reseal(&mut bad);
        assert!(matches!(
            decode(&bad),
            Err(NativeWorkerCompactReplayErrorV1::Coordinates)
        ));
    }
    let mut direct = original;
    direct[SESSION..SLOT].fill(0);
    reseal(&mut direct);
    assert!(decode(&direct).is_ok());
}

#[test]
fn shared_tail_enforces_counts_canonical_order_and_metadata_lengths() {
    let full = fixture(MAX_PROVIDERS, MAX_LINK_OPTIONS, true);
    let decoded = decode(&full).unwrap();
    assert_eq!(
        decoded.replay_view().external_providers.len(),
        MAX_PROVIDERS
    );
    assert_eq!(decoded.replay_view().link_options.len(), MAX_LINK_OPTIONS);
    let mut bad = full.clone();
    bad[PROVIDERS] = MAX_LINK_INPUTS as u8;
    reseal(&mut bad);
    assert!(decode(&bad).is_err());
    bad = full.clone();
    bad[PROVIDERS + 1 + 41 * MAX_PROVIDERS] = (MAX_LINK_OPTIONS + 1) as u8;
    reseal(&mut bad);
    assert!(decode(&bad).is_err());
    bad = full;
    let first = bad[PROVIDERS + 1..PROVIDERS + 42].to_vec();
    bad[PROVIDERS + 42..PROVIDERS + 83].copy_from_slice(&first);
    reseal(&mut bad);
    assert!(decode(&bad).is_err());
    bad = fixture(0, 2, true);
    // Each option is u8 name length, three name bytes, u16 value length, one value.
    let first_name = bad[PROVIDERS + 3..PROVIDERS + 6].to_vec();
    bad[PROVIDERS + 10..PROVIDERS + 13].copy_from_slice(&first_name);
    reseal(&mut bad);
    assert!(decode(&bad).is_err());
    bad = fixture(0, 0, true);
    bad[PROVIDERS + 2..PROVIDERS + 4].copy_from_slice(&u16::MAX.to_le_bytes());
    reseal(&mut bad);
    assert!(decode(&bad).is_err());
}

#[test]
fn shared_tail_enforces_output_timeout_provider_aggregate_and_outer_lengths() {
    let original = fixture(2, 0, true);
    for (offset, value) in [
        (LIMITS + 28, 0),
        (LIMITS + 28, crate::MAX_WORKER_OUTPUT_BYTES as u64 + 1),
        (OUTER + 32, MAX_COMPILER_MODULE_HANDOFF_BYTES_V4 as u64 + 1),
        (
            PROVIDERS + 1 + 1 + 32,
            crate::MAX_WORKER_TOTAL_INPUT_BYTES as u64,
        ),
    ] {
        let mut bad = original.clone();
        bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        reseal(&mut bad);
        assert!(decode(&bad).is_err(), "offset {offset}");
    }
    let mut bad = original;
    bad[LIMITS + 8..LIMITS + 12].copy_from_slice(&1_000_000_000_u32.to_le_bytes());
    reseal(&mut bad);
    assert!(decode(&bad).is_err());
}

#[test]
fn semantic_and_native_domains_cannot_be_cross_used_or_downgraded() {
    let native = fixture(0, 0, true);
    let mut semantic = Vec::new();
    semantic.extend_from_slice(b"F2V3CFR3");
    semantic.extend_from_slice(&3_u16.to_le_bytes());
    semantic.extend_from_slice(&native[10..74]);
    semantic.push(0);
    semantic.extend_from_slice(&native[TRANSACTION..HEADER_BYTES]);
    semantic.extend_from_slice(&native[HEADER_BYTES..native.len() - 32]);
    let checksum_domain = b"FE2O3/WORKER-V3-COMPACT-FINALIZER-REPLAY-CHECKSUM/V3\0";
    let checksum = hash(checksum_domain, &semantic);
    semantic.extend_from_slice(&checksum);
    let old = ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&semantic).unwrap();
    let new = decode(&native).unwrap();
    assert_ne!(old.identity().as_bytes(), new.identity().as_bytes());
    assert!(decode(&semantic).is_err());
    assert!(ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&native).is_err());
    let mut wrong_domain = native.clone();
    let n = wrong_domain.len() - 32;
    let checksum = hash(checksum_domain, &wrong_domain[..n]);
    wrong_domain[n..].copy_from_slice(&checksum);
    assert!(matches!(
        decode(&wrong_domain),
        Err(NativeWorkerCompactReplayErrorV1::Checksum)
    ));
    assert!(decode(&fixture(0, 0, false)).is_err());
    for magic in [b"F2V3CFR1", b"F2V3CFR2", b"F2V3CFR3"] {
        let mut bad = native.clone();
        bad[..8].copy_from_slice(magic);
        reseal(&mut bad);
        assert!(matches!(
            decode(&bad),
            Err(NativeWorkerCompactReplayErrorV1::Magic)
        ));
    }
}

#[test]
fn precise_quote_retained_delta_and_caller_ledger_are_preserved() {
    let bytes = fixture(MAX_PROVIDERS, MAX_LINK_OPTIONS, true);
    let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(bytes.len()).unwrap();
    let floor = bytes.len() + 19;
    let mut work = Work::new(quote.work());
    let mut budget = Budget::new(&mut work, floor + quote.scratch_storage());
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (value, returned) =
        NativeWorkerCompactFinalizerReplayV1::decode_canonical(&bytes, &mut budget).unwrap();
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.work(), quote.work());
    assert_eq!(budget.storage(), floor);
    assert_eq!(value.storage(), returned);
    let expected = size_of::<NativeWorkerCompactFinalizerReplayV1>()
        + bytes.len()
        + MAX_PROVIDERS * size_of::<WorkerV3ProviderReplayReferenceV1>()
        + MAX_LINK_OPTIONS * size_of::<LinkOptionV1>()
        + WORKER.len()
        + LLVM.len()
        + MAX_LINK_OPTIONS * 4;
    assert_eq!(returned.retained_storage(), expected);
    assert!(returned.retained_storage() <= quote.scratch_storage());
    budget.reserve_storage(returned.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor + returned.retained_storage());
}

#[test]
fn one_short_work_storage_and_unpaid_input_fail_without_leaking_storage() {
    let bytes = fixture(0, 0, true);
    let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(bytes.len()).unwrap();
    for (work_limit, storage_limit, floor) in [
        (quote.work() - 1, usize::MAX, bytes.len()),
        (
            quote.work(),
            bytes.len() + quote.scratch_storage() - 1,
            bytes.len(),
        ),
        (quote.work(), usize::MAX, bytes.len() - 1),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(matches!(
            NativeWorkerCompactFinalizerReplayV1::decode_canonical(&bytes, &mut budget),
            Err(NativeWorkerCompactReplayErrorV1::Resource(_))
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn malformed_tail_releases_full_scratch_and_retains_work_history() {
    let mut bytes = fixture(0, 0, true);
    bytes[PROVIDERS] = 255;
    reseal(&mut bytes);
    let quote = NativeWorkerCompactReplayResourcesV1::for_wire_length(bytes.len()).unwrap();
    let mut work = Work::new(quote.work());
    let mut budget = Budget::new(&mut work, bytes.len() + quote.scratch_storage());
    budget.reserve_storage(bytes.len()).unwrap();
    assert!(NativeWorkerCompactFinalizerReplayV1::decode_canonical(&bytes, &mut budget).is_err());
    assert_eq!(budget.storage(), bytes.len());
    assert_eq!(budget.work(), quote.work());
}

#[test]
fn schedule_rejects_length_overflow_and_tracks_only_native_header_growth() {
    assert_eq!(HEADER_BYTES, 235);
    assert_eq!(
        MAX_NATIVE_WORKER_COMPACT_FINALIZER_REPLAY_BYTES_V1,
        MAX_PROTECTED_WORKER_V3_COMPACT_FINALIZER_REPLAY_BYTES_V1 + 128
    );
    for n in [
        0,
        MIN_BYTES - 1,
        MAX_NATIVE_WORKER_COMPACT_FINALIZER_REPLAY_BYTES_V1 + 1,
        usize::MAX,
    ] {
        assert!(NativeWorkerCompactReplayResourcesV1::for_wire_length(n).is_err());
    }
    let small = NativeWorkerCompactReplayResourcesV1::for_wire_length(MIN_BYTES).unwrap();
    let large = NativeWorkerCompactReplayResourcesV1::for_wire_length(
        MAX_NATIVE_WORKER_COMPACT_FINALIZER_REPLAY_BYTES_V1,
    )
    .unwrap();
    assert!(large.work() > small.work());
    assert!(large.scratch_storage() > small.scratch_storage());
    assert!(size_of::<NativeWorkerCompactReplayErrorV1>() <= 128);
}
