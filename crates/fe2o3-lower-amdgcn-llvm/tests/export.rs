//! Owner-borrowing canonical graph serialization tests.

mod support;

use fe2o3_llvm_worker_handoff::MeasuredLlvmLldBuildV1;
use fe2o3_lower_amdgcn_llvm::{
    GraphExportErrorV1, LiveGraphSerializationErrorV1, LiveGraphSerializationRequestV1,
    lower_amdgcn_to_pliron_llvm_v1,
};

fn serialize(
    lowered: &fe2o3_lower_amdgcn_llvm::LoweredAmdgcnPlironLlvmV1,
) -> fe2o3_lower_amdgcn_llvm::AdmittedLiveGraphSerializationV1 {
    lowered
        .acquire_worker_serialization_v1(
            LiveGraphSerializationRequestV1::new(
                lowered.receipt().identity(),
                lowered.non_graph_envelope().identity(),
            ),
            MeasuredLlvmLldBuildV1::exact(),
        )
        .serialize_and_admit_v1()
        .unwrap()
}

#[test]
fn fresh_serialization_binds_exact_envelope_graph_assembly_and_worker() {
    let source = support::tiled_data_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
    let first = serialize(&lowered);
    let second = serialize(&lowered);

    assert_eq!(first.receipt(), second.receipt());
    assert_eq!(first.assembly(), second.assembly());
    assert_eq!(
        first.worker_admission().admission_identity(),
        second.worker_admission().admission_identity()
    );
    assert_eq!(
        first.receipt().non_graph_envelope_identity(),
        lowered.non_graph_envelope().identity()
    );
    assert_eq!(
        first.receipt().graph_handoff_identity(),
        first.worker_admission().handoff_identity()
    );
    assert_eq!(
        first.receipt().graph_inspection(),
        lowered.construction_inspection()
    );
    assert_eq!(first.receipt().assembly_sha256(), first.assembly().sha256());
    assert_eq!(
        first.receipt().worker_admission_identity(),
        first.worker_admission().admission_identity()
    );
    assert_eq!(
        first.retained_graph_export().canonical_handoff_identity(),
        first.receipt().graph_handoff_identity()
    );
    first
        .retained_graph_export()
        .revalidate_against(&lowered)
        .unwrap();
    assert_ne!(first.receipt().identity().as_bytes(), [0; 32]);
}

#[test]
fn retained_export_rejects_equivalent_fresh_owner_substitution() {
    let source = support::scalar_handoff();
    let owner = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
    let equivalent_but_foreign = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
    let serialized = serialize(&owner);
    let equivalent_serialized = serialize(&equivalent_but_foreign);

    assert_eq!(
        serialized
            .retained_graph_export()
            .canonical_handoff_identity(),
        equivalent_serialized
            .retained_graph_export()
            .canonical_handoff_identity()
    );
    assert!(matches!(
        serialized
            .retained_graph_export()
            .revalidate_against(&equivalent_but_foreign),
        Err(LiveGraphSerializationErrorV1::RetainedGraphOwnerMismatch)
    ));

    let (retained, receipt, assembly, worker) = serialized.into_retained_parts();
    retained.revalidate_against(&owner).unwrap();
    assert_eq!(
        retained.canonical_handoff_identity(),
        receipt.graph_handoff_identity()
    );
    assert_eq!(assembly.sha256(), receipt.assembly_sha256());
    assert_eq!(
        worker.admission_identity(),
        receipt.worker_admission_identity()
    );
    assert!(!retained.grants_artifact_authority());
}

#[test]
fn serialization_rejects_envelope_and_receipt_identity_substitution() {
    let first = lower_amdgcn_to_pliron_llvm_v1(&support::scalar_handoff()).unwrap();
    let second = lower_amdgcn_to_pliron_llvm_v1(&support::gemm_control_flow_handoff()).unwrap();

    let error = first
        .acquire_worker_serialization_v1(
            LiveGraphSerializationRequestV1::new(
                first.receipt().identity(),
                second.non_graph_envelope().identity(),
            ),
            MeasuredLlvmLldBuildV1::exact(),
        )
        .serialize_and_admit_v1()
        .unwrap_err();
    assert!(matches!(
        error,
        LiveGraphSerializationErrorV1::Graph(
            GraphExportErrorV1::NonGraphEnvelopeIdentitySubstitution
        )
    ));

    let error = first
        .acquire_worker_serialization_v1(
            LiveGraphSerializationRequestV1::new(
                second.receipt().identity(),
                first.non_graph_envelope().identity(),
            ),
            MeasuredLlvmLldBuildV1::exact(),
        )
        .serialize_and_admit_v1()
        .unwrap_err();
    assert!(matches!(
        error,
        LiveGraphSerializationErrorV1::Graph(GraphExportErrorV1::ReceiptIdentitySubstitution)
    ));
}

#[test]
fn detached_source_replacement_cannot_change_owner_output() {
    let mut source = support::tiled_data_handoff();
    let original_identity = source.identity();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
    source = support::gemm_control_flow_handoff();

    let serialized = serialize(&lowered);
    assert_eq!(lowered.source_identity(), original_identity);
    assert_ne!(source.identity(), original_identity);
    assert_ne!(
        serialized.worker_admission().handoff_identity(),
        source.identity()
    );
}

#[test]
fn source_item_evidence_cannot_supply_graph_or_worker_output() {
    let first_source = support::scalar_handoff();
    let second_source = support::scalar_handoff_with_empty_item_evidence();
    assert_ne!(first_source.identity(), second_source.identity());

    let first_owner = lower_amdgcn_to_pliron_llvm_v1(&first_source).unwrap();
    let second_owner = lower_amdgcn_to_pliron_llvm_v1(&second_source).unwrap();
    assert_eq!(
        first_owner.non_graph_envelope().identity(),
        second_owner.non_graph_envelope().identity()
    );
    assert_ne!(
        first_owner.receipt().identity(),
        second_owner.receipt().identity()
    );

    let first = serialize(&first_owner);
    let second = serialize(&second_owner);
    assert_eq!(first.receipt(), second.receipt());
    assert_eq!(first.assembly(), second.assembly());
    assert_eq!(first.worker_admission(), second.worker_admission());
}

#[test]
fn live_serialization_rejects_substituted_worker_build_policy() {
    let owner = lower_amdgcn_to_pliron_llvm_v1(&support::scalar_handoff()).unwrap();
    let substituted = MeasuredLlvmLldBuildV1::new(
        "22.1.9",
        fe2o3_llvm_worker_handoff::EXACT_LLVM_BUILD_IDENTITY_V1,
        fe2o3_llvm_worker_handoff::EXACT_LLD_VERSION_V1,
        fe2o3_llvm_worker_handoff::EXACT_LLD_BUILD_IDENTITY_V1,
        true,
    );
    let error = owner
        .acquire_worker_serialization_v1(
            LiveGraphSerializationRequestV1::new(
                owner.receipt().identity(),
                owner.non_graph_envelope().identity(),
            ),
            substituted,
        )
        .serialize_and_admit_v1()
        .unwrap_err();
    assert!(matches!(error, LiveGraphSerializationErrorV1::Worker(_)));
}
