//! Real public native-owner refusals and legacy replay regressions. Native
//! semantic recovery still requires descriptor V1, even in the V4 outer. These
//! tests do not synthesize a recovered V4 descriptor owner or conditional proof.
use super::*;
use fe2o3_artifact_transaction::ProducerIdentity;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as METADATA,
    InertSemanticCompilerModuleHandoffV4 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v4,
};
use fe2o3_hsaco_finalize::{
    NativeWorkerCompactFinalizerReplayV1, NativeWorkerEvidenceCustodyV1, NativeWorkerReplayErrorV1,
    finalize_native_worker_hsaco_v4, prepare_native_worker_compact_finalizer_replay_v1,
    revalidate_native_worker_finalizer_v1, revalidate_native_worker_finalizer_v4,
};
use fe2o3_verifier::recover_compiler_native_semantic_handoff_v4;
use sha2::{Digest, Sha256};

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_descriptor_v4_refuses_current_v1_only_native_sources() {
    let fixtures = Fixtures::open();
    for case in 0..CASES.len() {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let (source, _ready) = evidence(&fixtures, case, Mutation::None, &mut budget);
        assert_eq!(
            source.custody(),
            NativeWorkerEvidenceCustodyV1::ConsumedPublication
        );
        let abi = source
            .recovered_handoff()
            .handoff()
            .capsule()
            .base()
            .receipts()
            .abi()
            .canonical_preimage();
        assert_eq!(&abi[8..10], &1u16.to_le_bytes());
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let error = finalize_native_worker_hsaco_v4(source, &mut budget)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            NativeWorkerFinalizationErrorV1::Artifact {
                phase: "descriptor schema",
                ..
            }
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_descriptor_v4_entry_refuses_unpaid_floor_and_exhausted_original_budget() {
    let fixtures = Fixtures::open();
    for refusal in 0..3 {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let (source, _ready) = evidence(&fixtures, 3, Mutation::None, &mut budget);
        match refusal {
            0 => budget
                .release_storage(budget.storage() - (source.required_retained_storage() - 1))
                .unwrap(),
            1 => budget.charge_work(WORK_LIMIT - budget.work()).unwrap(),
            _ => budget
                .reserve_storage(STORAGE_LIMIT - budget.storage())
                .unwrap(),
        }
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let error = finalize_native_worker_hsaco_v4(source, &mut budget)
            .err()
            .unwrap();
        match refusal {
            0 => assert!(matches!(
                error,
                NativeWorkerFinalizationErrorV1::Resource(Resource::Accounting)
            )),
            1 => assert!(matches!(
                error,
                NativeWorkerFinalizationErrorV1::Resource(Resource::Work(_))
            )),
            _ => assert!(matches!(
                error,
                NativeWorkerFinalizationErrorV1::Resource(Resource::Storage(_))
            )),
        }
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_descriptor_replay_preserves_legacy_custody_and_rejects_v4_and_resealed_identity() {
    let fixtures = Fixtures::open();
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    let (source, _ready) = evidence(&fixtures, 0, Mutation::None, &mut budget);
    let original_source = source.identity();
    let original_binding = source.binding();
    let (finalized, storage) = finalize(source, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (transcript, storage) =
        prepare_native_worker_compact_finalizer_replay_v1(&finalized, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(
        finalized.source_evidence().custody(),
        NativeWorkerEvidenceCustodyV1::ConsumedPublication
    );
    let outer = finalized
        .source_evidence()
        .recovered_handoff()
        .handoff()
        .canonical_bytes();
    for variant in 0..4 {
        // A new caller models independent recovery; no production function resets a ledger.
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let wire = outer.to_vec();
        budget.reserve_storage(wire.capacity() + METADATA).unwrap();
        budget.charge_work(wire.len()).unwrap();
        budget
            .charge_work(inert_semantic_compiler_module_handoff_decode_work_v4(wire.len()).unwrap())
            .unwrap();
        let handoff = Handoff::decode_owned(wire).unwrap();
        let (recovered, storage) =
            recover_compiler_native_semantic_handoff_v4(handoff, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let mut wire = transcript.canonical_bytes().to_vec();
        if variant == 2 {
            // Re-seal a public inert transcript with a substituted expected
            // finalization identity; checksum validity cannot authenticate it.
            wire[10] ^= 1;
            let n = wire.len() - 32;
            let mut hash = Sha256::new();
            hash.update(b"FE2O3/NATIVE-WORKER-COMPACT-FINALIZER-REPLAY-CHECKSUM/V1\0");
            hash.update((n as u64).to_le_bytes());
            hash.update(&wire[..n]);
            wire[n..].copy_from_slice(&hash.finalize());
        }
        budget.reserve_storage(wire.len()).unwrap();
        let (replay, storage) =
            NativeWorkerCompactFinalizerReplayV1::decode_canonical(&wire, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let producer = if variant == 3 {
            ProducerIdentity::from_codegen("foreign_native_worker", None).unwrap()
        } else {
            ProducerIdentity::from_codegen(
                "native_worker_structural",
                Some(Path::new("/synthetic-native-worker.rs")),
            )
            .unwrap()
        };
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        if variant == 1 {
            let error = revalidate_native_worker_finalizer_v4(
                &producer,
                replay.attempt(),
                recovered,
                &replay,
                Vec::new(),
                finalized.exact_finalized_bytes(),
                &mut budget,
            )
            .err()
            .unwrap();
            assert!(matches!(
                error,
                NativeWorkerReplayErrorV1::Stage {
                    phase: "raw artifact reconstruction",
                    ..
                }
            ));
        } else {
            let result = revalidate_native_worker_finalizer_v1(
                &producer,
                replay.attempt(),
                recovered,
                &replay,
                Vec::new(),
                finalized.exact_finalized_bytes(),
                &mut budget,
            );
            if variant == 0 {
                let (replayed, storage) = result.unwrap();
                assert_eq!(replayed.identity(), finalized.identity());
                assert_eq!(replayed.source_evidence().identity(), original_source);
                assert_eq!(replayed.source_evidence().binding(), original_binding);
                assert_eq!(
                    replayed.source_evidence().custody(),
                    NativeWorkerEvidenceCustodyV1::RecoveredTranscript
                );
                assert_eq!(
                    replayed.exact_finalized_bytes(),
                    finalized.exact_finalized_bytes()
                );
                assert!(storage.retained_storage() > 0);
                assert!(!replayed.grants_publication_authority());
                assert!(!replayed.grants_load_authority() && !replayed.grants_launch_authority());
            } else {
                let error = result.err().unwrap();
                if variant == 2 {
                    assert!(
                        matches!(
                            error,
                            NativeWorkerReplayErrorV1::Mismatch("finalization identity")
                        ),
                        "{error:?}"
                    );
                } else {
                    assert!(
                        matches!(
                            error,
                            NativeWorkerReplayErrorV1::Stage {
                                phase: "occurrence",
                                ..
                            }
                        ),
                        "{error:?}"
                    );
                }
            }
        }
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}
