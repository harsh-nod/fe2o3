//! Owner-controlled canonical graph export tests.

mod support;

use fe2o3_lower_amdgcn_llvm::{
    GraphExportErrorV1, GraphExportRequestV1, lower_amdgcn_to_pliron_llvm_v1,
};

#[test]
fn fresh_export_binds_exact_source_receipt_and_live_graph() {
    let source = support::tiled_data_handoff();
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&source).unwrap();
    let request =
        GraphExportRequestV1::new(lowered.source_identity(), lowered.receipt().identity());
    let first = lowered.export_graph_v1(request).unwrap();
    let second = lowered.export_graph_v1(request).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.source_handoff(), &source);
    assert_eq!(first.source_identity(), source.identity());
    assert_eq!(first.graph_receipt(), lowered.receipt());
    assert_eq!(first.graph_inspection(), lowered.construction_inspection());
    assert_ne!(first.identity().as_bytes(), [0; 32]);
    assert!(!first.grants_artifact_authority());
}

#[test]
fn export_rejects_source_and_receipt_identity_substitution() {
    let first = lower_amdgcn_to_pliron_llvm_v1(&support::scalar_handoff()).unwrap();
    let second = lower_amdgcn_to_pliron_llvm_v1(&support::gemm_control_flow_handoff()).unwrap();

    assert!(matches!(
        first.export_graph_v1(GraphExportRequestV1::new(
            second.source_identity(),
            first.receipt().identity(),
        )),
        Err(GraphExportErrorV1::SourceIdentitySubstitution)
    ));
    assert!(matches!(
        first.export_graph_v1(GraphExportRequestV1::new(
            first.source_identity(),
            second.receipt().identity(),
        )),
        Err(GraphExportErrorV1::ReceiptIdentitySubstitution)
    ));
}
