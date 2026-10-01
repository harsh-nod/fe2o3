//! Public method coverage only; no reconciled or authenticated owner is minted.
use fe2o3_pliron::ProductionReconciledMirPlironSemanticContractV1;

fn semantic(owner: &ProductionReconciledMirPlironSemanticContractV1) -> Result<(), ()> {
    owner.visit_retained_heap_storage_v1(|_, _| Ok(()))
}

#[test]
fn reconciled_semantic_storage_method_is_publicly_nameable() {
    let _: fn(&ProductionReconciledMirPlironSemanticContractV1) -> Result<(), ()> = semantic;
}

#[cfg(feature = "internal-proof-staging")]
fn policy(owner: &fe2o3_pliron::ProductionRefinementStagingPolicyV2) -> Result<(), ()> {
    owner.visit_retained_logical_key_payload_v1(|_, _| Ok(()))
}

#[cfg(feature = "internal-proof-staging")]
#[test]
fn policy_logical_key_method_is_publicly_nameable_with_original_feature() {
    let _: fn(&fe2o3_pliron::ProductionRefinementStagingPolicyV2) -> Result<(), ()> = policy;
}
