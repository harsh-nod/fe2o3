//! Synthetic descriptor/ELF fixtures through real fixture-worker restart custody.
use super::*;
use fe2o3_hsaco_finalize::{
    PreparedFinalizedNominalWorkerHsacoV3, ProtectedWorkerV3CompactFinalizerReplayPartsV2,
    persist_prepared_nominal_worker_publication_v3,
    prepare_nominal_worker_compact_finalizer_replay_v3, prepare_nominal_worker_publication_v3,
    publish_recovered_nominal_worker_hsaco_v3, recover_nominal_worker_publication_v3,
};

fn finalized(
    directory: &TestDirectory,
    wire: &[u8],
    seed: u8,
) -> (
    fe2o3_artifact_transaction::BuildAttempt,
    PreparedFinalizedNominalWorkerHsacoV3,
) {
    let (attempt, source) = evidence_with_descriptor_source(
        directory,
        nominal_artifact(wire),
        EvidenceConfig {
            attempt_seed: seed,
            ..EvidenceConfig::BASE
        },
        &[("vecadd", "vecadd.kd")],
        Vec::new(),
        Some(wire),
    );
    (
        attempt,
        finalize_protected_worker_nominal_hsaco_v3(source, SCRATCH, &mut free).unwrap(),
    )
}

fn replay(
    attempt: fe2o3_artifact_transaction::BuildAttempt,
    parts: &ProtectedWorkerV3CompactFinalizerReplayPartsV2,
) -> Result<
    fe2o3_hsaco_finalize::RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    WorkerV3HsacoPublicationErrorV1,
> {
    revalidate_protected_worker_v3_finalizer_derivation_v1(
        attempt,
        &parts.outer_handoff,
        parts.external_provider_payloads.iter().map(Vec::as_slice),
        &parts.transcript,
        &parts.finalized_hsaco,
    )
}

#[test]
fn nominal_compact_restart_reconstructs_every_custody_axis_and_descriptor_version() {
    let directory = TestDirectory::new();
    let wire = nominal_slice_source("restart");
    let (attempt, finalized) = finalized(&directory, &wire, 0x61);
    let raw = finalized.raw();
    let source = raw.source_evidence().identity();
    let binding = raw.binding_identity();
    let worker = raw.worker_measurement().clone();
    let module = ContentIdentityV1::calculate(raw.outer_handoff().module_handoff().module_bytes());
    let plan = raw.link_plan_identity();
    let derivation = raw.source_evidence().derivation_evidence().clone();
    let raw_identity = raw.linked_output_identity();
    let finalization = finalized.identity();
    let output = finalized.output_identity();
    let descriptor = finalized.finalized().descriptor_bytes().to_vec();
    let parts = prepare_nominal_worker_compact_finalizer_replay_v3(finalized)
        .unwrap()
        .into_parts();
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&parts.transcript).unwrap();
    assert_eq!(transcript.canonical_bytes(), parts.transcript);
    let actual = replay(attempt, &parts).unwrap();
    assert_eq!(actual.descriptor_schema_version(), 3);
    assert_eq!(actual.transcript_identity(), transcript.identity());
    assert_eq!(actual.source_evidence_identity(), source);
    assert_eq!(actual.binding_identity(), binding);
    assert_eq!(actual.worker_measurement(), &worker);
    assert_eq!(actual.compiler_module_identity(), module);
    assert_eq!(actual.link_plan_identity(), plan);
    assert_eq!(actual.derivation_evidence(), &derivation);
    assert_eq!(actual.raw_hsaco_identity(), raw_identity);
    assert_eq!(actual.finalization_identity(), finalization);
    assert_eq!(actual.finalized_hsaco_identity(), output);
    let inspection = fe2o3_hsaco_finalize::inspect_finalized_nominal_hsaco_v3(
        &parts.finalized_hsaco,
        SCRATCH,
        &mut free,
    )
    .unwrap();
    let location = inspection.location();
    assert_eq!(
        &parts.finalized_hsaco[location.offset()..location.offset() + location.size()],
        descriptor
    );
    let reconstructed =
        derive_unfinalized_nominal_hsaco_v3(&parts.finalized_hsaco, SCRATCH, &mut free).unwrap();
    assert!(raw_identity.matches(&reconstructed));
    assert!(!actual.proves_llvm_to_machine_semantic_refinement());
    assert!(!actual.grants_compiler_authority() && !actual.grants_publication_authority());
    assert!(!actual.grants_load_authority() && !actual.grants_launch_authority());
}

#[test]
fn same_artifact_foreign_occurrence_cannot_reuse_nominal_finalization() {
    let a = TestDirectory::new();
    let b = TestDirectory::new();
    let wire = nominal_slice_source("same");
    let (attempt_a, first) = finalized(&a, &wire, 0x61);
    let (attempt_b, second) = finalized(&b, &wire, 0x62);
    assert_eq!(first.finalized().as_bytes(), second.finalized().as_bytes());
    assert_eq!(first.descriptor_identity(), second.descriptor_identity());
    assert_ne!(
        first.raw().transaction_identity(),
        second.raw().transaction_identity()
    );
    assert_ne!(
        first.raw().source_evidence_identity(),
        second.raw().source_evidence_identity()
    );
    assert_ne!(first.identity(), second.identity());
    let first = prepare_nominal_worker_compact_finalizer_replay_v3(first)
        .unwrap()
        .into_parts();
    let second = prepare_nominal_worker_compact_finalizer_replay_v3(second)
        .unwrap()
        .into_parts();
    replay(attempt_a, &first).unwrap();
    replay(attempt_b, &second).unwrap();
    assert!(replay(attempt_b, &first).is_err());
    assert!(
        revalidate_protected_worker_v3_finalizer_derivation_v1(
            attempt_a,
            &first.outer_handoff,
            first.external_provider_payloads.iter().map(Vec::as_slice),
            &second.transcript,
            &first.finalized_hsaco,
        )
        .is_err()
    );
}

#[test]
fn nominal_publication_restarts_before_and_after_commit_and_retains_lease() {
    let directory = TestDirectory::new();
    let wire = nominal_slice_source("publication");
    let (attempt, finalized) = finalized(&directory, &wire, 0x71);
    let identity = finalized.identity();
    let exact = finalized.finalized().as_bytes().to_vec();
    let closure = finalized.raw().compiler_closure();
    let prepared = prepare_nominal_worker_publication_v3(&producer(), finalized).unwrap();
    assert!(!prepared.grants_publication_authority() && !prepared.grants_launch_authority());
    let intent = prepared.publication_intent();
    let persisted =
        persist_prepared_nominal_worker_publication_v3(&directory.0, &producer(), prepared)
            .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    assert_eq!(persisted.finalized_evidence().identity(), identity);
    let binding = persisted.publication_binding(closure).unwrap();
    let subject = persisted.compiler_execution_subject_v1().unwrap();
    assert!(!persisted.grants_publication_authority());
    drop(persisted);
    assert!(matches!(
        recover_protected_worker_v3_hsaco_publication_v1(&directory.0, &producer(), attempt),
        Err(WorkerV3HsacoPublicationErrorV1::DescriptorSchemaMismatch)
    ));
    let recovered =
        recover_nominal_worker_publication_v3(&directory.0, &producer(), attempt).unwrap();
    assert_eq!(recovered.publication_intent(), intent);
    assert_eq!(recovered.compiler_execution_subject_v1().unwrap(), subject);
    assert_eq!(recovered.exact_finalized_hsaco(), exact);
    let foreign =
        CompilerClosureV2::new([21; 32], [22; 32], [23; 32], [24; 32], [25; 32], [26; 32]).unwrap();
    assert!(matches!(
        recovered.publication_binding(foreign),
        Err(WorkerV3HsacoPublicationErrorV1::CompilerClosureMismatch)
    ));
    let published =
        publish_recovered_nominal_worker_hsaco_v3(&directory.0, &producer(), closure, recovered)
            .unwrap();
    assert_eq!(
        published.publication_result().publication_binding(),
        binding
    );
    assert_eq!(published.compiler_execution_subject_v1().unwrap(), subject);
    assert!(!published.grants_compiler_authority() && !published.grants_proof_authority());
    assert!(!published.grants_load_authority() && !published.grants_launch_authority());
    drop(published);
    finish_build_attempt(&directory.0, &producer(), attempt).unwrap();
    let recovered =
        recover_nominal_worker_publication_v3(&directory.0, &producer(), attempt).unwrap();
    assert_eq!(
        recovered.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Recovered
    );
    assert_eq!(recovered.finalized_evidence().identity(), identity);
    assert_eq!(recovered.publication_intent(), intent);
    let published =
        publish_recovered_nominal_worker_hsaco_v3(&directory.0, &producer(), closure, recovered)
            .unwrap();
    assert_eq!(
        published.publication_result().outcome(),
        AttemptScopedHsacoPublicationOutcomeV3::RecoveredCommittedPublication
    );
    let (parts, record, claim, lease) = published
        .into_load_envelope_parts_v1()
        .unwrap()
        .into_parts();
    assert_eq!(record.attempt(), attempt);
    assert_eq!(lease.exact_artifact_bytes(), exact);
    assert_eq!(parts.finalized_hsaco, exact);
    assert_eq!(
        replay(attempt, &parts).unwrap().finalization_identity(),
        identity
    );
    let reacquired = reacquire_current_hsaco_publication_lease_v3(&directory.0, &claim).unwrap();
    assert_eq!(
        reacquired.exact_artifact_bytes(),
        lease.exact_artifact_bytes()
    );
}

#[test]
fn nominal_same_width_type_substitution_and_v1_cross_schema_replay_fail() {
    let a = TestDirectory::new();
    let b = TestDirectory::new();
    let c = TestDirectory::new();
    let f32_wire = nominal_slice_source_type("types", ScalarTypeV1::F32);
    let u32_wire = nominal_slice_source_type("types", ScalarTypeV1::U32);
    let (attempt_a, a_final) = finalized(&a, &f32_wire, 0x61);
    let (attempt_b, b_final) = finalized(&b, &u32_wire, 0x61);
    let first = prepare_nominal_worker_compact_finalizer_replay_v3(a_final)
        .unwrap()
        .into_parts();
    let second = prepare_nominal_worker_compact_finalizer_replay_v3(b_final)
        .unwrap()
        .into_parts();
    replay(attempt_a, &first).unwrap();
    replay(attempt_b, &second).unwrap();
    let v1 = slice_descriptor_table_with_workgroup(256);
    let (attempt_c, source) = evidence_with_descriptor_source(
        &c,
        slice_fixture_with_descriptor_table_and_workgroup(&v1, 256).bytes,
        EvidenceConfig {
            attempt_seed: 0x61,
            ..EvidenceConfig::BASE
        },
        &[("vecadd", "vecadd.kd")],
        Vec::new(),
        Some(&v1),
    );
    let final_c = finalize_protected_worker_v3_hsaco_v1(
        inspect_protected_worker_v3_hsaco_v1(source).unwrap(),
    )
    .unwrap();
    let legacy = prepare_protected_worker_v3_compact_finalizer_replay_v2(final_c)
        .unwrap()
        .into_parts();
    assert_eq!(
        replay(attempt_c, &legacy)
            .unwrap()
            .descriptor_schema_version(),
        1
    );
    for (attempt, good, other) in [
        (attempt_a, &first, &second),
        (attempt_a, &first, &legacy),
        (attempt_c, &legacy, &first),
    ] {
        assert!(
            revalidate_protected_worker_v3_finalizer_derivation_v1(
                attempt,
                &good.outer_handoff,
                good.external_provider_payloads.iter().map(Vec::as_slice),
                &good.transcript,
                &other.finalized_hsaco,
            )
            .is_err()
        );
        assert!(
            revalidate_protected_worker_v3_finalizer_derivation_v1(
                attempt,
                &other.outer_handoff,
                good.external_provider_payloads.iter().map(Vec::as_slice),
                &good.transcript,
                &good.finalized_hsaco,
            )
            .is_err()
        );
    }
}

#[test]
fn receipt_and_artifact_schema_matrix_has_no_implicit_conversion() {
    let nominal = nominal_slice_source("schema-matrix");
    let legacy = slice_descriptor_table_with_workgroup(256);
    for nominal_receipt in [false, true] {
        for nominal_object in [false, true] {
            let directory = TestDirectory::new();
            let artifact = if nominal_object {
                nominal_artifact(&nominal)
            } else {
                slice_fixture_with_descriptor_table_and_workgroup(&legacy, 256).bytes
            };
            let receipt = if nominal_receipt { &nominal } else { &legacy };
            let (attempt, source) = evidence_with_descriptor_source(
                &directory,
                artifact,
                EvidenceConfig::BASE,
                &[("vecadd", "vecadd.kd")],
                Vec::new(),
                Some(receipt),
            );
            if nominal_receipt {
                let result = finalize_protected_worker_nominal_hsaco_v3(source, SCRATCH, &mut free);
                assert_eq!(
                    result.is_ok(),
                    nominal_object,
                    "nominal receipt, nominal object={nominal_object}"
                );
            } else {
                let result = inspect_protected_worker_v3_hsaco_v1(source)
                    .map_err(|error| error.to_string())
                    .and_then(|raw| {
                        finalize_protected_worker_v3_hsaco_v1(raw)
                            .map_err(|error| error.to_string())
                    });
                assert_eq!(
                    result.is_ok(),
                    !nominal_object,
                    "V1 receipt, nominal object={nominal_object}"
                );
                if let Ok(finalized) = result {
                    let prepared =
                        prepare_protected_worker_v3_hsaco_publication_v1(&producer(), finalized)
                            .unwrap();
                    let recovered = persist_prepared_protected_worker_v3_hsaco_publication_v1(
                        &directory.0,
                        &producer(),
                        prepared,
                    )
                    .unwrap();
                    let identity = recovered.finalized_evidence().identity();
                    drop(recovered);
                    assert!(matches!(
                        recover_nominal_worker_publication_v3(&directory.0, &producer(), attempt),
                        Err(WorkerV3HsacoPublicationErrorV1::DescriptorSchemaMismatch)
                    ));
                    let recovered = recover_protected_worker_v3_hsaco_publication_v1(
                        &directory.0,
                        &producer(),
                        attempt,
                    )
                    .unwrap();
                    assert_eq!(recovered.finalized_evidence().identity(), identity);
                }
            }
        }
    }
}
