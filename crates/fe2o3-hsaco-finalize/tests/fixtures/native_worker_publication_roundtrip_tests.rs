//! Register as a child of native_worker_finalization_tests.rs to reuse its real
//! structural source/F + measured CPU fixture Worker + ELF finalization path.
//! No protected compiler, native LLVM, machine proof or GPU claim is made.

use super::{
    Budget, CASES, Fixtures, Mutation, STORAGE_LIMIT, Scratch, WORK_LIMIT, Work, evidence, finalize,
};
use fe2o3_artifact_transaction::{
    DurableLinkPublicationPlanV1, FinalizedOutputIdentityV1, ProducerIdentity,
    WorkerV3FinalizerReplayAttachmentsV1, WorkerV3PublicationIntentOutcomeV1, begin_build_attempt,
    persist_worker_v3_publication_intent_v1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, NativeWorkerCompactFinalizerReplayV1, NativeWorkerEvidenceCustodyV1,
    NativeWorkerHsacoPublicationErrorV1 as Error, PreparedNativeWorkerHsacoPublicationV1,
    RecoveredNativeWorkerHsacoPublicationV1,
    persist_prepared_native_worker_hsaco_publication_v1 as persist,
    prepare_native_worker_hsaco_publication_v1 as prepare,
    recover_native_worker_hsaco_publication_v1 as recover,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use sha2::{Digest, Sha256};
use std::{fs, mem::size_of, path::Path};

fn prepared(
    fixtures: &Fixtures,
    case: usize,
    budget: &mut Budget<'_>,
) -> (
    PreparedNativeWorkerHsacoPublicationV1,
    Scratch,
    ProducerIdentity,
) {
    let (source, _ready) = evidence(fixtures, case, Mutation::None, budget);
    let attempt = source.binding().receipt().attempt();
    let source_pointer = source
        .recovered_handoff()
        .handoff()
        .canonical_bytes()
        .as_ptr();
    // Same explicit synthetic producer spelling as the shared Worker fixture.
    // A separate live journal is opened through the real build-attempt API;
    // transaction rederivation is producer/attempt/content-specific, not a path lease.
    let producer = ProducerIdentity::from_codegen(
        "native_worker_structural",
        Some(Path::new("/synthetic-native-worker.rs")),
    )
    .unwrap();
    let directory = Scratch::new();
    let actual = begin_build_attempt(
        &directory.0,
        &producer,
        attempt.invocation(),
        attempt.session(),
    )
    .unwrap();
    assert_eq!(actual, attempt);
    let (finalized, storage) = finalize(source, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let original_floor = finalized.required_retained_storage();
    let floor = budget.storage();
    let (prepared, storage) = prepare(&producer, finalized, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        prepared.required_retained_storage(),
        original_floor + storage.retained_storage()
    );
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(
        prepared
            .finalized()
            .source_evidence()
            .recovered_handoff()
            .handoff()
            .canonical_bytes()
            .as_ptr(),
        source_pointer
    );
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());
    (prepared, directory, producer)
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_durable_fresh_and_restart_retain_all_four_source_f_owners() {
    let fixtures = Fixtures::open();
    for case in 0..CASES.len() {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let (prepared, directory, producer) = prepared(&fixtures, case, &mut budget);
        let expected_intent = prepared.intent();
        let expected_transcript = prepared.transcript().canonical_bytes().to_vec();
        let expected_output = prepared.finalized().exact_finalized_bytes().to_vec();
        let source_identity = prepared.finalized().source_evidence().identity();
        let finalization_identity = prepared.finalized().identity();
        let expected_receipt = prepared.finalized().source_evidence().binding().receipt();
        let outer_identity = prepared
            .finalized()
            .source_evidence()
            .recovered_handoff()
            .handoff()
            .identity();
        let actual_f = prepared
            .finalized()
            .source_evidence()
            .binding()
            .actual_f_identity();
        let attempt = expected_intent.durable_plan().attempt();
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let (fresh, storage) = persist(&directory.0, &producer, prepared, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(
            storage.retained_storage(),
            fresh.required_retained_storage()
        );
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(
            fresh.outcome(),
            WorkerV3PublicationIntentOutcomeV1::Persisted
        );
        assert_eq!(fresh.intent(), expected_intent);
        assert_eq!(fresh.record().plan(), expected_intent.durable_plan());
        assert_eq!(fresh.transcript().canonical_bytes(), expected_transcript);
        assert_eq!(fresh.finalized().exact_finalized_bytes(), expected_output);
        assert_eq!(
            fresh.finalized().source_evidence().identity(),
            source_identity
        );
        assert_eq!(fresh.finalized().identity(), finalization_identity);
        assert_eq!(
            fresh.finalized().source_evidence().binding().receipt(),
            expected_receipt
        );
        assert_eq!(
            fresh.finalized().source_evidence().custody(),
            NativeWorkerEvidenceCustodyV1::RecoveredTranscript
        );
        assert_eq!(
            fresh
                .finalized()
                .source_evidence()
                .binding()
                .actual_f_identity(),
            actual_f
        );

        // A new caller models a later process. No library operation replaces an
        // exhausted ledger, and the active first caller is not reused/reset.
        let mut restart_work = Work::new(WORK_LIMIT);
        let mut restart_budget = Budget::new(&mut restart_work, STORAGE_LIMIT);
        let (restarted, storage) =
            recover(&directory.0, &producer, attempt, &mut restart_budget).unwrap();
        assert_eq!(restart_budget.storage(), 0);
        restart_budget
            .reserve_storage(storage.retained_storage())
            .unwrap();
        assert_eq!(
            storage.retained_storage(),
            restarted.required_retained_storage()
        );
        assert_eq!(
            restarted.outcome(),
            WorkerV3PublicationIntentOutcomeV1::Recovered
        );
        assert_eq!(restarted.record(), fresh.record());
        assert_eq!(restarted.intent(), expected_intent);
        assert_eq!(
            restarted.transcript().canonical_bytes(),
            expected_transcript
        );
        assert_eq!(
            restarted.finalized().exact_finalized_bytes(),
            expected_output
        );
        assert_eq!(
            restarted.finalized().source_evidence().identity(),
            source_identity
        );
        assert_eq!(restarted.finalized().identity(), finalization_identity);
        assert_eq!(
            restarted.finalized().source_evidence().binding().receipt(),
            expected_receipt
        );
        assert_eq!(
            restarted.finalized().source_evidence().custody(),
            NativeWorkerEvidenceCustodyV1::RecoveredTranscript
        );
        assert_eq!(
            restarted
                .finalized()
                .source_evidence()
                .recovered_handoff()
                .handoff()
                .identity(),
            outer_identity
        );
        assert_eq!(
            restarted
                .finalized()
                .source_evidence()
                .binding()
                .actual_f_identity(),
            actual_f
        );
        assert!(!restarted.authenticates_compiler_origin());
        assert!(!restarted.grants_publication_authority());
        assert!(!restarted.grants_load_authority());
        assert!(!restarted.grants_launch_authority());
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_durable_foreign_producer_refuses_before_journal_writes() {
    let fixtures = Fixtures::open();
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    let (prepared, _directory, _producer) = prepared(&fixtures, 0, &mut budget);
    let foreign = ProducerIdentity::from_codegen("foreign_native_publication", None).unwrap();
    let untouched = Scratch::new();
    let floor = budget.storage();
    let error = persist(&untouched.0, &foreign, prepared, &mut budget)
        .err()
        .unwrap();
    assert!(matches!(error, Error::Mismatch("producer")));
    assert_eq!(budget.storage(), floor);
    assert_eq!(fs::read_dir(&untouched.0).unwrap().count(), 0);
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_restart_rejects_committed_transcript_and_artifact_substitution() {
    let fixtures = Fixtures::open();
    for corrupt_transcript in [true, false] {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let (prepared, directory, producer) = prepared(&fixtures, 0, &mut budget);
        let finalized = prepared.finalized();
        assert_eq!(finalized.source_evidence().plan().inputs().len(), 1);
        let outer = finalized
            .source_evidence()
            .recovered_handoff()
            .handoff()
            .canonical_bytes()
            .to_vec();
        let mut transcript = prepared.transcript().canonical_bytes().to_vec();
        let mut output = finalized.exact_finalized_bytes().to_vec();
        let mut plan = prepared.intent().durable_plan();
        if corrupt_transcript {
            transcript[10] ^= 1;
        } else {
            output[0] ^= 1;
            // A public, inert plan can declare the substituted payload hash.
            // This lets the storage layer commit it, not authenticate it.
            plan = DurableLinkPublicationPlanV1::new(
                plan.attempt(),
                plan.scope(),
                plan.request(),
                plan.worker(),
                plan.response(),
                plan.linked_output(),
                plan.finalization(),
                FinalizedOutputIdentityV1::from_bytes(
                    *ContentIdentityV1::calculate(&output).sha256(),
                ),
                plan.publication(),
            );
        }
        let attachments =
            WorkerV3FinalizerReplayAttachmentsV1::new(outer, Vec::new(), transcript).unwrap();
        let stored = persist_worker_v3_publication_intent_v1(
            &directory.0,
            &producer,
            plan.attempt(),
            plan,
            attachments,
            output,
        )
        .unwrap();
        assert!(!stored.grants_publication_authority());
        let floor = budget.storage();
        let error = recover(&directory.0, &producer, plan.attempt(), &mut budget)
            .err()
            .unwrap();
        let expected_phase = if corrupt_transcript {
            "native compact replay"
        } else {
            "native finalizer replay"
        };
        assert!(
            matches!(error, Error::Stage { phase, .. } if phase == expected_phase),
            "{error:?}"
        );
        assert_eq!(budget.storage(), floor);
    }
}

// Store public, inert attachments without using the native prepare validator.
// Every case gets its own live journal; Scratch removes only that directory.
fn store_attachments(
    prepared: &PreparedNativeWorkerHsacoPublicationV1,
    producer: &ProducerIdentity,
    transcript: Vec<u8>,
    providers: Vec<Vec<u8>>,
) -> Scratch {
    let directory = Scratch::new();
    let plan = prepared.intent().durable_plan();
    let attempt = plan.attempt();
    assert_eq!(
        begin_build_attempt(
            &directory.0,
            producer,
            attempt.invocation(),
            attempt.session(),
        )
        .unwrap(),
        attempt
    );
    let finalized = prepared.finalized();
    let outer = finalized
        .source_evidence()
        .recovered_handoff()
        .handoff()
        .canonical_bytes()
        .to_vec();
    let attachments =
        WorkerV3FinalizerReplayAttachmentsV1::new(outer, providers, transcript).unwrap();
    let stored = persist_worker_v3_publication_intent_v1(
        &directory.0,
        producer,
        attempt,
        plan,
        attachments,
        finalized.exact_finalized_bytes().to_vec(),
    )
    .unwrap();
    assert!(!stored.grants_publication_authority());
    directory
}

fn assert_returned_charge(recovered: &RecoveredNativeWorkerHsacoPublicationV1, retained: usize) {
    // Source backing/recovery is already included in the finalizer's floor.
    // Do not add it again, or treat artifact-domain payloads as native charges.
    assert_eq!(
        retained,
        recovered.finalized().required_retained_storage()
            + recovered.transcript().storage().retained_storage()
            + size_of::<RecoveredNativeWorkerHsacoPublicationV1>()
    );
    assert_eq!(retained, recovered.required_retained_storage());
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_durable_replay_exact_and_one_short_resources_preserve_original_ledger() {
    let fixtures = Fixtures::open();
    let mut fixture_work = Work::new(WORK_LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, STORAGE_LIMIT);
    let (prepared, _directory, producer) = prepared(&fixtures, 0, &mut fixture_budget);
    let directory = store_attachments(
        &prepared,
        &producer,
        prepared.transcript().canonical_bytes().to_vec(),
        Vec::new(),
    );
    let attempt = prepared.intent().durable_plan().attempt();
    const INHERITED_WORK: usize = 7;
    const INHERITED_STORAGE: usize = 13;

    // Measure the real decode/source/codec/Worker/finalizer path, not a dummy
    // prepaid callback. Each subsequent ledger models an independent restart.
    let (exact_work, exact_storage, returned) = {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(INHERITED_WORK).unwrap();
        budget.reserve_storage(INHERITED_STORAGE).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let (recovered, storage) = recover(&directory.0, &producer, attempt, &mut budget).unwrap();
        assert_eq!(budget.storage(), INHERITED_STORAGE);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_returned_charge(&recovered, storage.retained_storage());
        budget.reserve_storage(storage.retained_storage()).unwrap();
        (
            budget.work(),
            budget.peak_storage(),
            storage.retained_storage(),
        )
    };

    for (work_limit, storage_limit, refusal) in [
        (exact_work, exact_storage, None),
        (exact_work - 1, exact_storage, Some("work")),
        (exact_work, exact_storage - 1, Some("storage")),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(INHERITED_WORK).unwrap();
        budget.reserve_storage(INHERITED_STORAGE).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = recover(&directory.0, &producer, attempt, &mut budget);
        assert_eq!(budget.storage(), INHERITED_STORAGE);
        assert!(budget.work_ledger_identity_v1() == ledger);
        match refusal {
            None => {
                let (recovered, storage) = result.unwrap();
                assert_eq!(budget.work(), exact_work);
                assert_eq!(storage.retained_storage(), returned);
                assert_returned_charge(&recovered, returned);
                assert_eq!(recovered.intent(), prepared.intent());
                assert_eq!(
                    recovered.finalized().source_evidence().binding().receipt(),
                    prepared.finalized().source_evidence().binding().receipt()
                );
                budget.reserve_storage(returned).unwrap();
                assert_eq!(budget.peak_storage(), exact_storage);
                assert_eq!(budget.failed_storage(), None);
                drop(recovered);
                budget.release_storage(returned).unwrap();
                assert_eq!(budget.storage(), INHERITED_STORAGE);
                assert_eq!(work.failed_work(), None);
            }
            Some("work") => {
                let error = result
                    .err()
                    .expect("one-short work must refuse actual replay");
                assert!(
                    matches!(error, Error::Resource(Resource::Work(_))),
                    "{error:?}"
                );
                assert!(budget.work() >= INHERITED_WORK);
                assert_eq!(work.failed_work(), Some(exact_work));
            }
            Some("storage") => {
                let error = result
                    .err()
                    .expect("one-short storage must refuse actual replay");
                assert!(
                    matches!(error, Error::Resource(Resource::Storage(_))),
                    "{error:?}"
                );
                assert_eq!(budget.failed_storage(), Some(exact_storage));
                assert!(budget.work() > INHERITED_WORK);
                assert_eq!(work.failed_work(), None);
            }
            _ => unreachable!(),
        }
    }
}

fn reseal_native_transcript(bytes: &mut [u8]) {
    let (body, checksum) = bytes.split_at_mut(bytes.len() - 32);
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/NATIVE-WORKER-COMPACT-FINALIZER-REPLAY-CHECKSUM/V1\0");
    hash.update((body.len() as u64).to_le_bytes());
    hash.update(body);
    checksum.copy_from_slice(&hash.finalize());
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_restart_rejects_checksum_valid_occurrence_and_evidence_substitution() {
    let fixtures = Fixtures::open();
    let mut fixture_work = Work::new(WORK_LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, STORAGE_LIMIT);
    let (prepared, _directory, producer) = prepared(&fixtures, 0, &mut fixture_budget);
    let attempt = prepared.intent().durable_plan().attempt();
    // Native V1 header offsets, independent of the shared variable-length tail.
    for (offset, expected) in [
        (10, "finalization identity"),
        (42, "source evidence"),
        (74, "Worker binding"),
        (106, "outer"),
        (138, "outer"),
        (146, "attempt"),
        (154, "attempt"),
        (170, "attempt"),
        (203, "occurrence"),
    ] {
        let mut bytes = prepared.transcript().canonical_bytes().to_vec();
        bytes[offset] ^= 0x80;
        reseal_native_transcript(&mut bytes);
        // Prove each mutation survives the strict native codec, so refusal is
        // the source/occurrence/evidence join, not a damaged checksum or grammar.
        {
            let mut work = Work::new(WORK_LIMIT);
            let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
            budget.reserve_storage(bytes.len()).unwrap();
            let (decoded, storage) =
                NativeWorkerCompactFinalizerReplayV1::decode_canonical(&bytes, &mut budget)
                    .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_ne!(decoded.identity(), prepared.transcript().identity());
            assert_eq!(decoded.canonical_bytes(), bytes);
        }
        let directory = store_attachments(&prepared, &producer, bytes, Vec::new());
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.reserve_storage(13).unwrap();
        budget.charge_work(7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error = recover(&directory.0, &producer, attempt, &mut budget)
            .err()
            .expect("checksum-valid foreign coordinate must refuse");
        match expected {
            "attempt" => assert!(
                matches!(error, Error::Mismatch("transcript attempt")),
                "{error:?}"
            ),
            "outer" => assert!(
                matches!(
                    &error,
                    Error::Stage {
                        phase: "native compact replay",
                        ..
                    }
                ),
                "{error:?}"
            ),
            join => assert!(
                matches!(&error, Error::Stage { phase: "native finalizer replay", diagnostic }
                    if diagnostic.to_string().contains(join)),
                "offset {offset}: {error:?}"
            ),
        }
        assert_eq!(budget.storage(), 13);
        assert!(budget.work() > 7);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
#[ignore = "requires exported FE2O3_NATIVE_WORKER_FIXTURE_DIR and measured CPU fixture Worker"]
fn native_restart_rejects_nonempty_provider_attachment_against_empty_replay() {
    let fixtures = Fixtures::open();
    let mut fixture_work = Work::new(WORK_LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, STORAGE_LIMIT);
    let (prepared, _directory, producer) = prepared(&fixtures, 0, &mut fixture_budget);
    assert_eq!(
        prepared.finalized().source_evidence().plan().inputs().len(),
        1
    );
    // This is only a count-mismatch refusal. It asserts no positive external
    // provider support under the strict native raw-artifact policy.
    let directory = store_attachments(
        &prepared,
        &producer,
        prepared.transcript().canonical_bytes().to_vec(),
        vec![b"unexpected-provider".to_vec()],
    );
    let mut work = Work::new(WORK_LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    budget.reserve_storage(13).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let error = recover(
        &directory.0,
        &producer,
        prepared.intent().durable_plan().attempt(),
        &mut budget,
    )
    .err()
    .expect("undeclared external provider must refuse");
    assert!(
        matches!(&error, Error::Stage { phase: "native finalizer replay", diagnostic }
            if diagnostic.to_string().contains("provider count")),
        "{error:?}"
    );
    assert_eq!(budget.storage(), 13);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
}
