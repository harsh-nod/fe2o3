//! Determinism tests for canonical typed lowering receipts.

mod support;

use fe2o3_llvm_handoff::NamedMetadataV1;
use fe2o3_lower_amdgcn_llvm::lower_amdgcn_to_pliron_llvm_v1;

#[test]
fn receipt_is_independent_of_input_collection_and_context_allocation_order() {
    let canonical = support::scalar_handoff();
    let permuted = support::scalar_handoff_permuted();
    assert_eq!(canonical.encode_canonical(), permuted.encode_canonical());

    let first = lower_amdgcn_to_pliron_llvm_v1(&canonical).unwrap();
    let second = lower_amdgcn_to_pliron_llvm_v1(&permuted).unwrap();
    let third = lower_amdgcn_to_pliron_llvm_v1(&canonical).unwrap();
    assert_ne!(first.context_identity(), second.context_identity());
    assert_ne!(second.context_identity(), third.context_identity());
    assert_eq!(first.receipt(), second.receipt());
    assert_eq!(second.receipt(), third.receipt());
    assert_eq!(
        first.construction_inspection().graph_sha256(),
        third.construction_inspection().graph_sha256()
    );
}

#[test]
fn attributes_metadata_and_origins_change_the_canonical_receipt() {
    let baseline = lower_amdgcn_to_pliron_llvm_v1(&support::scalar_handoff()).unwrap();
    let workgroup =
        lower_amdgcn_to_pliron_llvm_v1(&support::handoff_with_required_workgroup_size([2, 1, 1]))
            .unwrap();
    let metadata = lower_amdgcn_to_pliron_llvm_v1(&support::handoff_with_named_metadata(
        NamedMetadataV1::OpenClVersion2_0,
    ))
    .unwrap();
    let origin = lower_amdgcn_to_pliron_llvm_v1(&support::control_flow_handoff()).unwrap();

    assert_ne!(
        baseline.receipt().identity(),
        workgroup.receipt().identity()
    );
    assert_ne!(baseline.receipt().identity(), metadata.receipt().identity());
    assert_ne!(baseline.receipt().identity(), origin.receipt().identity());
}
