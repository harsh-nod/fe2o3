//! Structural restart coverage through the existing fixture Worker and durable store.
use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3, ProtectedWorkerV3CompactFinalizerReplayPartsV2,
    finalize_protected_worker_nominal_hsaco_v3, persist_prepared_nominal_worker_publication_v3,
    persist_prepared_nominal_worker_publication_v4,
    prepare_nominal_worker_compact_finalizer_replay_v3,
    prepare_nominal_worker_compact_finalizer_replay_v4, prepare_nominal_worker_publication_v3,
    prepare_nominal_worker_publication_v4, recover_nominal_worker_publication_v3,
    recover_nominal_worker_publication_v4,
};

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
fn v4_compact_restart_reconstructs_exact_contract_and_every_custody_axis() {
    let directory = TestDirectory::new();
    let (wire, _) = wires("restart", |_| {});
    let (attempt, finalized) = finalized(&directory, &wire, 0x61);
    let raw = finalized.raw();
    let source = raw.source_evidence_identity();
    let binding = raw.binding_identity();
    let transaction = raw.transaction_identity();
    let worker = raw.worker_measurement().clone();
    let module = ContentIdentityV1::calculate(raw.outer_handoff().module_handoff().module_bytes());
    let plan = raw.link_plan_identity();
    let derivation = raw.source_evidence().derivation_evidence().clone();
    let raw_identity = raw.linked_output_identity();
    let finalization = finalized.identity();
    let output = finalized.output_identity();
    let descriptor = finalized.finalized().descriptor_bytes().to_vec();
    let parts = prepare_nominal_worker_compact_finalizer_replay_v4(finalized)
        .unwrap()
        .into_parts();
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&parts.transcript).unwrap();
    assert_eq!(transcript.canonical_bytes(), parts.transcript);
    assert_eq!(transcript.transaction_identity(), transaction);
    let actual = replay(attempt, &parts).unwrap();
    assert_eq!(actual.descriptor_schema_version(), 4);
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
    let inspected =
        inspect_finalized_nominal_hsaco_v4(&parts.finalized_hsaco, SCRATCH, &mut free).unwrap();
    assert_eq!(inspected.descriptor_table().canonical_bytes(), descriptor);
    assert!(!actual.proves_llvm_to_machine_semantic_refinement());
    assert!(!actual.grants_compiler_authority() && !actual.grants_publication_authority());
    assert!(!actual.grants_load_authority() && !actual.grants_launch_authority());
}

#[test]
fn v4_intent_persistence_and_restart_stay_structural_and_cannot_downgrade() {
    let directory = TestDirectory::new();
    let (wire, _) = wires("persistence", |_| {});
    let (attempt, finalized) = finalized(&directory, &wire, 0x71);
    let identity = finalized.identity();
    let transaction = finalized.raw().transaction_identity();
    let exact = finalized.finalized().as_bytes().to_vec();
    let prepared = prepare_nominal_worker_publication_v4(&producer(), finalized).unwrap();
    assert_eq!(prepared.exact_finalized_hsaco(), exact);
    assert!(!prepared.grants_compiler_authority() && !prepared.grants_proof_authority());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority() && !prepared.grants_launch_authority());
    let intent = prepared.publication_intent();
    let persisted =
        persist_prepared_nominal_worker_publication_v4(&directory.0, &producer(), prepared)
            .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    let record = persisted.storage_record();
    assert_eq!(persisted.finalized_evidence().identity(), identity);
    drop(persisted);
    for after_finish in [false, true] {
        if after_finish {
            finish_build_attempt(&directory.0, &producer(), attempt).unwrap();
        }
        assert!(matches!(
            recover_protected_worker_v3_hsaco_publication_v1(&directory.0, &producer(), attempt),
            Err(WorkerV3HsacoPublicationErrorV1::DescriptorSchemaMismatch)
        ));
        assert!(matches!(
            recover_nominal_worker_publication_v3(&directory.0, &producer(), attempt),
            Err(WorkerV3HsacoPublicationErrorV1::DescriptorSchemaMismatch)
        ));
        let recovered =
            recover_nominal_worker_publication_v4(&directory.0, &producer(), attempt).unwrap();
        assert_eq!(
            recovered.outcome(),
            WorkerV3PublicationIntentOutcomeV1::Recovered
        );
        assert_eq!(recovered.storage_record(), record);
        assert_eq!(recovered.publication_intent(), intent);
        assert_eq!(recovered.exact_finalized_hsaco(), exact);
        assert_eq!(recovered.finalized_evidence().identity(), identity);
        assert_eq!(
            recovered.finalized_evidence().raw().transaction_identity(),
            transaction
        );
        assert!(recovered.is_structural_only());
        assert!(!recovered.grants_compiler_authority() && !recovered.grants_proof_authority());
        assert!(!recovered.grants_publication_authority());
        assert!(!recovered.grants_load_authority() && !recovered.grants_launch_authority());
    }
}

#[test]
fn v4_same_artifact_foreign_transaction_cannot_reuse_finalization_or_transcript() {
    let a = TestDirectory::new();
    let b = TestDirectory::new();
    let (wire, _) = wires("occurrence", |_| {});
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
    let first = prepare_nominal_worker_compact_finalizer_replay_v4(first)
        .unwrap()
        .into_parts();
    let second = prepare_nominal_worker_compact_finalizer_replay_v4(second)
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
fn v4_restart_rejects_valid_contract_source_and_cross_schema_substitutions() {
    let (v4, v3) = wires("substitution", |_| {});
    let (foreign_source, _) = wires("foreign", |_| {});
    let (foreign_contract, _) = wires("substitution", |c| c.arguments[0].adjusted_argument = 8);
    let v1 = slice_descriptor_table_with_workgroup(256);
    let mut cases = Vec::new();
    for (schema, wire) in [
        (4, &v4),
        (4, &foreign_source),
        (4, &foreign_contract),
        (3, &v3),
        (1, &v1),
    ] {
        let directory = TestDirectory::new();
        let (attempt, evidence) = source(
            &directory,
            artifact(wire, schema),
            wire,
            EvidenceConfig::BASE,
        );
        let parts = match schema {
            4 => prepare_nominal_worker_compact_finalizer_replay_v4(
                finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut free).unwrap(),
            )
            .unwrap()
            .into_parts(),
            3 => prepare_nominal_worker_compact_finalizer_replay_v3(
                finalize_protected_worker_nominal_hsaco_v3(
                    evidence,
                    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                    &mut free,
                )
                .unwrap(),
            )
            .unwrap()
            .into_parts(),
            _ => prepare_protected_worker_v3_compact_finalizer_replay_v2(
                finalize_protected_worker_v3_hsaco_v1(
                    inspect_protected_worker_v3_hsaco_v1(evidence).unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
            .into_parts(),
        };
        assert_eq!(
            replay(attempt, &parts).unwrap().descriptor_schema_version(),
            u16::from(schema)
        );
        cases.push((directory, attempt, parts));
    }
    let (_, first_attempt, first) = &cases[0];
    for (_, other_attempt, other) in &cases[1..] {
        for (attempt, good, foreign) in [
            (*first_attempt, first, other),
            (*other_attempt, other, first),
        ] {
            assert!(
                revalidate_protected_worker_v3_finalizer_derivation_v1(
                    attempt,
                    &good.outer_handoff,
                    good.external_provider_payloads.iter().map(Vec::as_slice),
                    &good.transcript,
                    &foreign.finalized_hsaco
                )
                .is_err()
            );
            assert!(
                revalidate_protected_worker_v3_finalizer_derivation_v1(
                    attempt,
                    &foreign.outer_handoff,
                    good.external_provider_payloads.iter().map(Vec::as_slice),
                    &good.transcript,
                    &good.finalized_hsaco
                )
                .is_err()
            );
        }
    }
}

#[test]
fn v1_v3_v4_receipt_artifact_and_typed_recovery_matrix_has_no_fallback() {
    let (v4, v3) = wires("schema-matrix", |_| {});
    let v1 = slice_descriptor_table_with_workgroup(256);
    for (schema, receipt) in [(1, &v1), (3, &v3), (4, &v4)] {
        for (object_schema, wire) in [(1, &v1), (3, &v3), (4, &v4)] {
            let directory = TestDirectory::new();
            let (attempt, evidence) = source(
                &directory,
                artifact(wire, object_schema),
                receipt,
                EvidenceConfig::BASE,
            );
            let persisted = match schema {
                1 => inspect_protected_worker_v3_hsaco_v1(evidence)
                    .map_err(|e| e.to_string())
                    .and_then(|raw| {
                        finalize_protected_worker_v3_hsaco_v1(raw).map_err(|e| e.to_string())
                    })
                    .map(|finalized| {
                        let prepared = prepare_protected_worker_v3_hsaco_publication_v1(
                            &producer(),
                            finalized,
                        )
                        .unwrap();
                        persist_prepared_protected_worker_v3_hsaco_publication_v1(
                            &directory.0,
                            &producer(),
                            prepared,
                        )
                        .unwrap();
                    }),
                3 => finalize_protected_worker_nominal_hsaco_v3(
                    evidence,
                    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                    &mut free,
                )
                .map_err(|e| e.to_string())
                .map(|finalized| {
                    let prepared =
                        prepare_nominal_worker_publication_v3(&producer(), finalized).unwrap();
                    persist_prepared_nominal_worker_publication_v3(
                        &directory.0,
                        &producer(),
                        prepared,
                    )
                    .unwrap();
                }),
                _ => finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut free)
                    .map_err(|e| e.to_string())
                    .map(|finalized| {
                        let prepared =
                            prepare_nominal_worker_publication_v4(&producer(), finalized).unwrap();
                        persist_prepared_nominal_worker_publication_v4(
                            &directory.0,
                            &producer(),
                            prepared,
                        )
                        .unwrap();
                    }),
            };
            assert_eq!(
                persisted.is_ok(),
                schema == object_schema,
                "receipt={schema}, object={object_schema}"
            );
            if persisted.is_err() {
                continue;
            }
            let v1 = recover_protected_worker_v3_hsaco_publication_v1(
                &directory.0,
                &producer(),
                attempt,
            )
            .map(|_| ());
            let v3 = recover_nominal_worker_publication_v3(&directory.0, &producer(), attempt)
                .map(|_| ());
            let v4 = recover_nominal_worker_publication_v4(&directory.0, &producer(), attempt)
                .map(|_| ());
            for (recovery_schema, result) in [(1, v1), (3, v3), (4, v4)] {
                if recovery_schema == schema {
                    result.unwrap();
                } else {
                    assert!(matches!(
                        result,
                        Err(WorkerV3HsacoPublicationErrorV1::DescriptorSchemaMismatch)
                    ));
                }
            }
        }
    }
}

#[test]
fn v4_wire_and_section_relabeling_cannot_erase_mandatory_contracts() {
    let (v4, v3) = wires("relabel", |_| {});
    for wire in [&v4, &v3] {
        for wire_schema in [1u16, 3, 4, 5] {
            for section_schema in [1, 3, 4, 5] {
                if wire == &v4 && wire_schema == 4 && section_schema == 4 {
                    continue;
                }
                let mut changed = wire.clone();
                changed[8..10].copy_from_slice(&wire_schema.to_le_bytes());
                let directory = TestDirectory::new();
                let (_, evidence) = source(
                    &directory,
                    artifact(&changed, section_schema),
                    &v4,
                    EvidenceConfig::BASE,
                );
                assert!(
                    finalize_protected_worker_nominal_hsaco_v4(evidence, SCRATCH, &mut free)
                        .is_err(),
                    "wire={wire_schema}, section={section_schema}"
                );
            }
        }
    }
}
