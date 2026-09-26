//! Inert restart/intent controls. These tests never publish a code object or envelope.
use super::*;
use fe2o3_artifact_transaction::{
    WorkerV3PublicationIntentErrorV1 as StoreError, clear_worker_v3_publication_intent_v1,
};
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V4,
    ProtectedWorkerV3CompactFinalizerReplayPartsV2, finalize_protected_worker_nominal_hsaco_v3,
    finalize_protected_worker_nominal_hsaco_v4, persist_prepared_nominal_worker_publication_v3,
    persist_prepared_nominal_worker_publication_v4, persist_prepared_nominal_worker_publication_v5,
    prepare_nominal_worker_compact_finalizer_replay_v5, prepare_nominal_worker_publication_v3,
    prepare_nominal_worker_publication_v4, prepare_nominal_worker_publication_v5,
    recover_nominal_worker_publication_v3, recover_nominal_worker_publication_v4,
    recover_nominal_worker_publication_v5,
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
fn compact_restart_binds_every_original_lineage_axis_and_v5_contract() {
    let directory = TestDirectory::new();
    let (wire, _) = wires(TARGET, "restart");
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
    let parts = prepare_nominal_worker_compact_finalizer_replay_v5(finalized)
        .unwrap()
        .into_parts();
    let transcript =
        ProtectedWorkerV3CompactFinalizerReplayV2::decode_canonical(&parts.transcript).unwrap();
    assert_eq!(transcript.transaction_identity(), transaction);
    let actual = replay(attempt, &parts).unwrap();
    assert_eq!(actual.descriptor_schema_version(), 5);
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
        inspect_finalized_nominal_hsaco_v5(&parts.finalized_hsaco, SCRATCH, &mut free).unwrap();
    assert_eq!(inspected.descriptor_table().canonical_bytes(), descriptor);
    assert!(!actual.proves_llvm_to_machine_semantic_refinement());
    assert!(!actual.grants_compiler_authority() && !actual.grants_publication_authority());
    assert!(!actual.grants_load_authority() && !actual.grants_launch_authority());
}

#[test]
fn same_bytes_other_transaction_and_coherent_cpu_substitution_cannot_reuse_replay() {
    let (wire, _) = wires(TARGET, "replay");
    let changed = substitute_cpu(&wire, true);
    let (foreign, _) = wires(TARGET, "foreign");
    let mut cases = Vec::new();
    for (index, wire) in [&wire, &wire, &changed, &foreign].into_iter().enumerate() {
        let directory = TestDirectory::new();
        let (attempt, finalized) = finalized(&directory, wire, 0x61 + index as u8);
        let identity = finalized.identity();
        let parts = prepare_nominal_worker_compact_finalizer_replay_v5(finalized)
            .unwrap()
            .into_parts();
        replay(attempt, &parts).unwrap();
        cases.push((directory, attempt, identity, parts));
    }
    assert_eq!(cases[0].3.finalized_hsaco, cases[1].3.finalized_hsaco);
    let (_, attempt, identity, first) = &cases[0];
    for (_, other_attempt, other_identity, other) in &cases[1..] {
        assert_ne!(identity, other_identity);
        assert!(replay(*other_attempt, first).is_err());
        for (outer, transcript, hsaco) in [
            (
                &other.outer_handoff,
                &first.transcript,
                &first.finalized_hsaco,
            ),
            (
                &first.outer_handoff,
                &other.transcript,
                &first.finalized_hsaco,
            ),
        ] {
            assert!(
                revalidate_protected_worker_v3_finalizer_derivation_v1(
                    *attempt,
                    outer,
                    first.external_provider_payloads.iter().map(Vec::as_slice),
                    transcript,
                    hsaco
                )
                .is_err()
            );
        }
        // Same bytes are deliberately valid under their own lineage; changed
        // source/CPU bytes must additionally fail exact output reconstruction.
        if other.finalized_hsaco != first.finalized_hsaco {
            assert!(
                revalidate_protected_worker_v3_finalizer_derivation_v1(
                    *attempt,
                    &first.outer_handoff,
                    first.external_provider_payloads.iter().map(Vec::as_slice),
                    &first.transcript,
                    &other.finalized_hsaco
                )
                .is_err()
            );
        }
    }
}

fn record_path(directory: &TestDirectory) -> PathBuf {
    let files = fs::read_dir(&directory.0)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_str().unwrap();
            n.starts_with(".fe2o3-worker-v3-publication-intent-v1-") && n.ends_with(".record")
        })
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 1);
    files[0].clone()
}

#[test]
fn prepared_intent_refuses_a_different_producer_before_persisting() {
    let directory = TestDirectory::new();
    let (wire, _) = wires(TARGET, "producer");
    let (_, finalized) = finalized(&directory, &wire, 0x71);
    let prepared = prepare_nominal_worker_publication_v5(&producer(), finalized).unwrap();
    let other = ProducerIdentity::from_codegen("v5-other", Some(Path::new("v5-other.rs"))).unwrap();
    assert!(matches!(
        persist_prepared_nominal_worker_publication_v5(&directory.0, &other, prepared),
        Err(WorkerV3HsacoPublicationErrorV1::ProducerIdentityMismatch)
    ));
    assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_str()
            .unwrap()
            .starts_with(".fe2o3-worker-v3-publication-intent-v1-")
    }));
}

#[test]
fn wrong_typed_recovery_repairs_redo_before_refusal_and_correct_recovery_stays_inert() {
    let directory = TestDirectory::new();
    let (wire, _) = wires(TARGET, "redo");
    let (attempt, finalized) = finalized(&directory, &wire, 0x71);
    let identity = finalized.identity();
    let exact = finalized.finalized().as_bytes().to_vec();
    let prepared = prepare_nominal_worker_publication_v5(&producer(), finalized).unwrap();
    assert!(!prepared.grants_compiler_authority() && !prepared.grants_proof_authority());
    assert!(
        !prepared.grants_publication_authority()
            && !prepared.grants_load_authority()
            && !prepared.grants_launch_authority()
    );
    let intent = prepared.publication_intent();
    let persisted =
        persist_prepared_nominal_worker_publication_v5(&directory.0, &producer(), prepared)
            .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    let record = persisted.storage_record();
    drop(persisted);
    let path = record_path(&directory);
    let original = fs::read(&path).unwrap();
    let redo = path.with_extension("record.redo");
    fs::rename(&path, &redo).unwrap();
    assert!(matches!(
        recover_nominal_worker_publication_v4(&directory.0, &producer(), attempt),
        Err(WorkerV3HsacoPublicationErrorV1::DescriptorSchemaMismatch)
    ));
    assert!(!redo.exists());
    assert_eq!(fs::read(&path).unwrap(), original);
    for _ in 0..2 {
        let recovered =
            recover_nominal_worker_publication_v5(&directory.0, &producer(), attempt).unwrap();
        assert_eq!(
            recovered.outcome(),
            WorkerV3PublicationIntentOutcomeV1::Recovered
        );
        assert_eq!(recovered.storage_record(), record);
        assert_eq!(recovered.publication_intent(), intent);
        assert_eq!(recovered.exact_finalized_hsaco(), exact);
        assert_eq!(recovered.finalized_evidence().identity(), identity);
        assert!(recovered.is_structural_only());
        assert!(!recovered.grants_compiler_authority() && !recovered.grants_proof_authority());
        assert!(
            !recovered.grants_publication_authority()
                && !recovered.grants_load_authority()
                && !recovered.grants_launch_authority()
        );
    }
    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &directory.0,
            &producer(),
            attempt,
            record.identity()
        ),
        Err(StoreError::ReceiptNotDurable)
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
    let other = ProducerIdentity::from_codegen("v5-other", Some(Path::new("v5-other.rs"))).unwrap();
    assert!(recover_nominal_worker_publication_v5(&directory.0, &other, attempt).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    finish_build_attempt(&directory.0, &producer(), attempt).unwrap_err();
    assert!(matches!(
        recover_nominal_worker_publication_v5(&directory.0, &producer(), attempt),
        Err(WorkerV3HsacoPublicationErrorV1::Storage(
            StoreError::Attempt { .. }
        ))
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn corrupt_payload_and_retirement_marker_never_return_recovered_v5() {
    for suffix in ["output", "transcript", "record", "record.retiring"] {
        let directory = TestDirectory::new();
        let (wire, _) = wires(TARGET, "corruption");
        let (attempt, finalized) = finalized(&directory, &wire, 0x71);
        let prepared = prepare_nominal_worker_publication_v5(&producer(), finalized).unwrap();
        persist_prepared_nominal_worker_publication_v5(&directory.0, &producer(), prepared)
            .unwrap();
        let record = record_path(&directory);
        if suffix == "record.retiring" {
            fs::rename(&record, record.with_extension(suffix)).unwrap();
        } else {
            let path = record.with_extension(suffix);
            let mut bytes = fs::read(&path).unwrap();
            bytes[0] ^= 1;
            fs::write(&path, &bytes).unwrap();
        }
        assert!(
            matches!(
                recover_nominal_worker_publication_v5(&directory.0, &producer(), attempt),
                Err(WorkerV3HsacoPublicationErrorV1::Storage(_))
            ),
            "{suffix}"
        );
    }
}

#[test]
fn only_successor_can_retire_structural_v5_restart_state() {
    let directory = TestDirectory::new();
    let (wire, _) = wires(TARGET, "successor");
    let (attempt, finalized) = finalized(&directory, &wire, 0x71);
    let prepared = prepare_nominal_worker_publication_v5(&producer(), finalized).unwrap();
    let stored =
        persist_prepared_nominal_worker_publication_v5(&directory.0, &producer(), prepared)
            .unwrap();
    let record = stored.storage_record();
    drop(stored);
    let successor = begin_build_attempt(
        &directory.0,
        &producer(),
        BuildInvocation::from_bytes([0x73; 32]),
        BuildSession::from_bytes([0x74; 16]),
    )
    .unwrap();
    assert_ne!(attempt, successor);
    assert!(recover_nominal_worker_publication_v5(&directory.0, &producer(), attempt).is_err());
    clear_worker_v3_publication_intent_v1(&directory.0, &producer(), attempt, record.identity())
        .unwrap();
    assert!(recover_nominal_worker_publication_v5(&directory.0, &producer(), attempt).is_err());
    assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_str()
            .unwrap()
            .starts_with(".fe2o3-worker-v3-publication-intent-v1-")
    }));
}

#[test]
fn v1_v3_v4_v5_exact_receipt_artifact_and_typed_recovery_matrix_has_no_fallback() {
    let (v5, v3) = wires(TARGET, "matrix");
    let (v4, _) = super::super::nominal_v4::wires("matrix", |_| {});
    let v1 = slice_descriptor_table_with_workgroup(256);
    for (schema, receipt) in [(1, &v1), (3, &v3), (4, &v4), (5, &v5)] {
        for (object_schema, wire) in [(1, &v1), (3, &v3), (4, &v4), (5, &v5)] {
            // The unchanged V4 suite covers the old 3x3 matrix; this extension
            // exercises each old diagonal plus every new V5 cross edge.
            if schema != 5 && object_schema != 5 && schema != object_schema {
                continue;
            }
            let directory = TestDirectory::new();
            let (attempt, evidence) = source(
                &directory,
                artifact(wire, object_schema, TARGET),
                receipt,
                EvidenceConfig::BASE,
                TARGET,
            );
            let persisted = match schema {
                1 => inspect_protected_worker_v3_hsaco_v1(evidence)
                    .map_err(|e| e.to_string())
                    .and_then(|raw| {
                        finalize_protected_worker_v3_hsaco_v1(raw).map_err(|e| e.to_string())
                    })
                    .map(|value| {
                        let prepared =
                            prepare_protected_worker_v3_hsaco_publication_v1(&producer(), value)
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
                .map(|value| {
                    let prepared =
                        prepare_nominal_worker_publication_v3(&producer(), value).unwrap();
                    persist_prepared_nominal_worker_publication_v3(
                        &directory.0,
                        &producer(),
                        prepared,
                    )
                    .unwrap();
                }),
                4 => finalize_protected_worker_nominal_hsaco_v4(
                    evidence,
                    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V4,
                    &mut free,
                )
                .map_err(|e| e.to_string())
                .map(|value| {
                    let prepared =
                        prepare_nominal_worker_publication_v4(&producer(), value).unwrap();
                    persist_prepared_nominal_worker_publication_v4(
                        &directory.0,
                        &producer(),
                        prepared,
                    )
                    .unwrap();
                }),
                _ => finalize_protected_worker_nominal_hsaco_v5(evidence, SCRATCH, &mut free)
                    .map_err(|e| e.to_string())
                    .map(|value| {
                        let prepared =
                            prepare_nominal_worker_publication_v5(&producer(), value).unwrap();
                        persist_prepared_nominal_worker_publication_v5(
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
                "receipt={schema} object={object_schema}"
            );
            if persisted.is_err() {
                continue;
            }
            for (recovery_schema, result) in [
                (
                    1,
                    recover_protected_worker_v3_hsaco_publication_v1(
                        &directory.0,
                        &producer(),
                        attempt,
                    )
                    .map(|_| ()),
                ),
                (
                    3,
                    recover_nominal_worker_publication_v3(&directory.0, &producer(), attempt)
                        .map(|_| ()),
                ),
                (
                    4,
                    recover_nominal_worker_publication_v4(&directory.0, &producer(), attempt)
                        .map(|_| ()),
                ),
                (
                    5,
                    recover_nominal_worker_publication_v5(&directory.0, &producer(), attempt)
                        .map(|_| ()),
                ),
            ] {
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
