//! Real fixture-worker custody, synthetic ELF/issuer receipts; no protected/GPU credit.
use super::*;
use fe2o3_host::{
    __hardware_test::ApplicationHandoffTwoKernelRosterFixtureV1 as TwoKernelRoster,
    __hardware_test::ApplicationHandoffVecAddRosterFixtureV1 as Roster,
    CompilerGeneratedKernelExpectationRosterEntryV1, CompilerGeneratedKernelExpectationRosterV1,
    RecoveredNominalWorkerV3AdmissionError, RecoveredWorkerV3AdmissionErrorV1,
    admit_recovered_nominal_worker_v3_roster, admit_recovered_worker_v3_roster_v1,
};
use fe2o3_kernel_descriptor::{
    DeviceDescriptorTableV3, KernelAbiLayoutV1, ScalarTypeV1, SourceTypeDescriptorV3,
};
use worker_v3_fixture::{TestDirectory, publish_nominal_worker_v3_fixture_in_directory as publish};

fn free(_: usize) -> Result<(), std::convert::Infallible> {
    Ok(())
}

fn assert_nominal_type(table: &DeviceDescriptorTableV3<'_>, scalar: ScalarTypeV1) {
    assert_eq!(table.kernel_count(), 1);
    let kernel = table.kernel(0, &mut free).unwrap();
    assert_eq!(
        kernel.abi_layout(),
        KernelAbiLayoutV1::new(16, 272, 8).unwrap()
    );
    let argument = kernel.arguments().next(&mut free).unwrap().unwrap();
    assert_eq!(
        table
            .source_type(argument.source_type(), &mut free)
            .unwrap()
            .descriptor(),
        SourceTypeDescriptorV3::SharedSlice(scalar)
    );
}

#[test]
fn nominal_receipt_envelope_survives_retirement_and_host_readmission() {
    let directory = TestDirectory::new();
    let (producer, attempt, published) = publish(&directory, 0x61, ScalarTypeV1::F32);
    let expected_identity = published
        .recovered_evidence()
        .finalized_evidence()
        .identity();
    let expected_artifact = published
        .recovered_evidence()
        .exact_finalized_hsaco()
        .to_vec();
    let expected_descriptor = published
        .recovered_evidence()
        .finalized_evidence()
        .finalized()
        .descriptor_bytes()
        .to_vec();
    let subject = published.compiler_execution_subject_v1().unwrap();
    let carriage = carriage_for_subject(&subject, 0x61);
    let envelope =
        WorkerV3LoadEnvelopeV2::from_published_nominal_hsaco_v3(published, carriage.clone())
            .unwrap();
    let canonical = envelope.encode_canonical().unwrap();
    let decoded = WorkerV3LoadEnvelopeWireV2::decode_canonical(&canonical).unwrap();
    assert_eq!(decoded.encode_canonical().unwrap(), canonical);
    assert_eq!(decoded.compiler_execution_receipt(), &carriage);
    assert_complete_subject_association(
        &subject,
        &decoded
            .reconstructed_compiler_execution_subject_v1()
            .unwrap(),
    );
    assert_subject_and_carriage_are_authority_free(&subject, &carriage);
    let intent = envelope
        .wire()
        .replay()
        .publication_intent_record()
        .identity();
    let readiness = envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    assert_eq!(readiness.exact_envelope_bytes(), canonical);
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &directory.0,
        &producer,
        attempt,
        intent,
        readiness.receipt(),
    )
    .unwrap();
    drop(envelope);

    let recovered = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    assert_eq!(recovered.receipt(), readiness.receipt());
    assert_eq!(recovered.exact_artifact_bytes(), expected_artifact);
    assert_eq!(
        recovered.canonical_evidence_view().exact_canonical_bytes(),
        canonical
    );
    assert_eq!(
        recovered.canonical_evidence_view().binding(),
        readiness.receipt().envelope_binding()
    );
    let inspection = fe2o3_hsaco_finalize::inspect_finalized_nominal_hsaco_v3(
        recovered.exact_artifact_bytes(),
        fe2o3_hsaco_finalize::NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
        &mut free,
    )
    .unwrap();
    assert_eq!(
        inspection.descriptor_table().canonical_bytes(),
        expected_descriptor
    );
    assert_nominal_type(inspection.descriptor_table(), ScalarTypeV1::F32);
    drop(inspection);
    let admitted = admit_recovered_nominal_worker_v3_roster::<Roster>(recovered).unwrap();
    assert_eq!(
        admitted.finalizer_derivation().finalization_identity(),
        expected_identity
    );
    assert_eq!(
        admitted.finalizer_derivation().descriptor_schema_version(),
        3
    );
    assert_complete_subject_association(&subject, admitted.compiler_execution_subject());
    assert_eq!(admitted.compiler_execution_receipt(), &carriage);
    assert_eq!(admitted.entrypoints().len(), 1);
    assert_eq!(admitted.entrypoints()[0].ordinal(), 0);
    assert_nominal_type(&admitted.descriptor_table().unwrap(), ScalarTypeV1::F32);
    assert_eq!(
        admitted
            .physical_kernel(0)
            .unwrap()
            .required_workgroup_size(),
        Some([256, 1, 1])
    );
    assert_eq!(admitted.descriptor_binding(0).unwrap().kernel_index(), 0);
    assert!(admitted.physical_kernel(1).is_none());
    assert!(admitted.authenticates_descriptor_source());
    assert!(!admitted.authenticates_compiler_origin());
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority() && !admitted.grants_launch_authority());
    admitted.revalidate_currentness().unwrap();

    let again = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    let again = admit_recovered_nominal_worker_v3_roster::<Roster>(again).unwrap();
    assert_eq!(again.lineage_identity(), admitted.lineage_identity());
    assert_eq!(
        again.entrypoints()[0].lineage_identity(),
        admitted.entrypoints()[0].lineage_identity()
    );
    drop(again);
    let legacy = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    assert!(admit_recovered_worker_v3_roster_v1::<Roster>(legacy).is_err());

    let (_, _, successor) = publish(&directory, 0x62, ScalarTypeV1::F32);
    assert!(matches!(
        admitted.revalidate_currentness(),
        Err(RecoveredNominalWorkerV3AdmissionError::Custody(
            RecoveredWorkerV3AdmissionErrorV1::CurrentPublication(_)
        ))
    ));
    drop(successor);
}

#[test]
fn nominal_foreign_occurrence_carriage_is_rejected_even_for_identical_artifact() {
    let first = TestDirectory::new();
    let second = TestDirectory::new();
    let (_, _, a) = publish(&first, 0x61, ScalarTypeV1::F32);
    let (_, _, b) = publish(&second, 0x62, ScalarTypeV1::F32);
    assert_eq!(
        a.recovered_evidence().exact_finalized_hsaco(),
        b.recovered_evidence().exact_finalized_hsaco()
    );
    let subject_a = a.compiler_execution_subject_v1().unwrap();
    let subject_b = b.compiler_execution_subject_v1().unwrap();
    assert_ne!(subject_a, subject_b);
    let wrong_carriage = carriage_for_subject(&subject_b, 0x61);
    assert!(matches!(
        WorkerV3LoadEnvelopeV2::from_published_nominal_hsaco_v3(a, wrong_carriage.clone()),
        Err(WorkerV3LoadEnvelopeErrorV2::BindingMismatch {
            field: WorkerV3LoadEnvelopeBindingFieldV2::CompilerExecutionSubject
        })
    ));
    let good = WorkerV3LoadEnvelopeV2::from_published_nominal_hsaco_v3(b, wrong_carriage).unwrap();
    assert_complete_subject_association(
        &subject_b,
        &good
            .wire()
            .reconstructed_compiler_execution_subject_v1()
            .unwrap(),
    );
}

struct Empty;
impl CompilerGeneratedKernelExpectationRosterV1 for Empty {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] = &[];
}
struct Duplicate;
impl CompilerGeneratedKernelExpectationRosterV1 for Duplicate {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] =
        &[Roster::ENTRIES[0], Roster::ENTRIES[0]];
}

#[test]
fn nominal_roster_refusals_and_retained_directory_recovery_preserve_exact_types() {
    let directory = TestDirectory::new();
    let (_, attempt, published) = publish(&directory, 0x71, ScalarTypeV1::U32);
    let subject = published.compiler_execution_subject_v1().unwrap();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_nominal_hsaco_v3(
        published,
        carriage_for_subject(&subject, 0x61),
    )
    .unwrap();
    envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    drop(envelope);
    let recovered = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    assert!(matches!(
        admit_recovered_nominal_worker_v3_roster::<Empty>(recovered),
        Err(RecoveredNominalWorkerV3AdmissionError::Custody(
            RecoveredWorkerV3AdmissionErrorV1::EmptyRoster
        ))
    ));
    let recovered = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    assert!(matches!(
        admit_recovered_nominal_worker_v3_roster::<Duplicate>(recovered),
        Err(RecoveredNominalWorkerV3AdmissionError::Custody(
            RecoveredWorkerV3AdmissionErrorV1::DuplicateRosterEntry { .. }
        ))
    ));

    fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o700)).unwrap();
    let retained =
        RetainedDurableDirectoryV1::admit_service_owned(File::open(&directory.0).unwrap().into())
            .unwrap();
    let displaced = directory.0.with_extension("nominal-displaced");
    fs::rename(&directory.0, &displaced).unwrap();
    let recovered = recover_worker_v3_load_envelope_from_retained_directory_v2(&retained, attempt)
        .map_err(|e| e.to_string())
        .and_then(|recovered| {
            admit_recovered_nominal_worker_v3_roster::<Roster>(recovered).map_err(|e| e.to_string())
        });
    let current = recovered
        .as_ref()
        .ok()
        .map(|value| value.revalidate_currentness());
    fs::rename(&displaced, &directory.0).unwrap();
    let admitted = recovered.unwrap();
    current.unwrap().unwrap();
    assert_nominal_type(&admitted.descriptor_table().unwrap(), ScalarTypeV1::U32);
    assert_complete_subject_association(&subject, admitted.compiler_execution_subject());
}

#[test]
fn legacy_descriptor_cannot_enter_nominal_host_admission() {
    let worker_v3_fixture::PublishedWorkerV3Fixture {
        directory,
        attempt,
        published,
        ..
    } = worker_v3_fixture::published_worker_v3_fixture();
    let subject = published.compiler_execution_subject_v1().unwrap();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(
        published,
        carriage_for_subject(&subject, 0x61),
    )
    .unwrap();
    envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    drop(envelope);
    let recovered = recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    assert!(matches!(
        admit_recovered_nominal_worker_v3_roster::<Roster>(recovered),
        Err(RecoveredNominalWorkerV3AdmissionError::DescriptorSchema)
    ));
}

struct Reordered;
impl CompilerGeneratedKernelExpectationRosterV1 for Reordered {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] =
        &[TwoKernelRoster::ENTRIES[1], TwoKernelRoster::ENTRIES[0]];
}
struct Substituted;
impl CompilerGeneratedKernelExpectationRosterV1 for Substituted {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] =
        &[TwoKernelRoster::ENTRIES[0], Roster::ENTRIES[0]];
}
struct Missing;
impl CompilerGeneratedKernelExpectationRosterV1 for Missing {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] =
        &[TwoKernelRoster::ENTRIES[0]];
}
struct Extra;
impl CompilerGeneratedKernelExpectationRosterV1 for Extra {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1] = &[
        TwoKernelRoster::ENTRIES[0],
        TwoKernelRoster::ENTRIES[1],
        Roster::ENTRIES[0],
    ];
}

#[test]
fn nominal_multi_kernel_readmission_resolves_physical_order_and_requires_exact_roster() {
    let directory = TestDirectory::new();
    let (_, attempt, published) =
        worker_v3_fixture::publish_two_kernel_nominal_worker_v3_fixture_in_directory(
            &directory, 0x73,
        );
    let subject = published.compiler_execution_subject_v1().unwrap();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_nominal_hsaco_v3(
        published,
        carriage_for_subject(&subject, 0x61),
    )
    .unwrap();
    envelope
        .persist_durable_replay_custody_v2(&directory.0)
        .unwrap();
    drop(envelope);
    let recover = || recover_worker_v3_load_envelope_v2(&directory.0, attempt).unwrap();
    let admitted = admit_recovered_nominal_worker_v3_roster::<TwoKernelRoster>(recover()).unwrap();
    assert_eq!(admitted.entrypoints().len(), 2);
    let table = admitted.descriptor_table().unwrap();
    assert_eq!(table.kernel_count(), 2);
    for (ordinal, physical_index) in [1, 0].into_iter().enumerate() {
        let kernel = table.kernel(ordinal, &mut free).unwrap();
        let physical = admitted.physical_kernel(ordinal).unwrap();
        assert_eq!(
            kernel.abi_layout(),
            KernelAbiLayoutV1::new(16, 272, 8).unwrap()
        );
        assert_eq!(physical.kernarg_segment_size(), 272);
        assert_eq!(physical.required_workgroup_size(), Some([256, 1, 1]));
        assert_eq!(admitted.entrypoints()[ordinal].ordinal(), ordinal);
        assert_eq!(physical.name(), kernel.entry_name());
        assert_eq!(
            admitted.descriptor_binding(ordinal).unwrap().kernel_index(),
            physical_index
        );
    }
    assert_ne!(
        admitted.entrypoints()[0].lineage_identity(),
        admitted.entrypoints()[1].lineage_identity()
    );
    admitted.revalidate_currentness().unwrap();
    let again = admit_recovered_nominal_worker_v3_roster::<TwoKernelRoster>(recover()).unwrap();
    assert_eq!(again.lineage_identity(), admitted.lineage_identity());
    assert!(matches!(
        admit_recovered_nominal_worker_v3_roster::<Reordered>(recover()),
        Err(RecoveredNominalWorkerV3AdmissionError::Custody(
            RecoveredWorkerV3AdmissionErrorV1::RosterEntryReordered {
                expected_ordinal: 0,
                actual_ordinal: 1
            }
        ))
    ));
    assert!(matches!(
        admit_recovered_nominal_worker_v3_roster::<Substituted>(recover()),
        Err(RecoveredNominalWorkerV3AdmissionError::Custody(
            RecoveredWorkerV3AdmissionErrorV1::RosterEntrySubstituted { ordinal: 1 }
        ))
    ));
    assert!(matches!(
        admit_recovered_nominal_worker_v3_roster::<Missing>(recover()),
        Err(RecoveredNominalWorkerV3AdmissionError::Custody(
            RecoveredWorkerV3AdmissionErrorV1::RosterLengthMismatch {
                expected: 1,
                actual: 2
            }
        ))
    ));
    assert!(matches!(
        admit_recovered_nominal_worker_v3_roster::<Extra>(recover()),
        Err(RecoveredNominalWorkerV3AdmissionError::Custody(
            RecoveredWorkerV3AdmissionErrorV1::RosterLengthMismatch {
                expected: 3,
                actual: 2
            }
        ))
    ));
}
