use super::*;

#[test]
fn request_quotes_fund_both_inventories_and_every_refusal_turn_without_cleanup_growth() {
    let admission = RootCompilerRequest::preparation_quota().unwrap();
    let refusal = RootCompilerRequest::refusal_quota().unwrap();
    let bytes =
        usize::try_from(fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_TOTAL_BYTES_V1)
            .unwrap();
    assert!(admission.scratch() > 2 * bytes);
    assert!(admission.work() > 12 * 8 * bytes);
    assert!(refusal.work() > 5 * 8 * bytes);
    assert!(RootCompilerRequest::ENVELOPE >= size_of::<Option<Backing>>());
    let first =
        crate::InheritedCompilerExecutionDeploymentV3::original_root_startup_quota(1, 1).unwrap();
    let next =
        crate::InheritedCompilerExecutionDeploymentV3::original_root_startup_quota(2, 1).unwrap();
    let old_first = crate::InheritedCompilerExecutionDeploymentV3::startup_quota(1, 1).unwrap();
    let old_next = crate::InheritedCompilerExecutionDeploymentV3::startup_quota(2, 1).unwrap();
    assert_eq!(first.cleanup_storage(), old_first.cleanup_storage());
    assert_eq!(
        next.request_work() - first.request_work(),
        old_next.request_work() - old_first.request_work()
            + Receiver::TURN_WORK
            + Prepared::maximum_revalidation_quota().unwrap().work()
            + refusal.work()
    );
}

#[test]
fn consuming_preparation_preserves_nested_resource_errors() {
    use fe2o3_build_authority::{
        CompilerApprovalPolicyErrorV1 as Framing, CompilerApprovalPolicyErrorV2 as Policy,
        CompilerRuntimeManifestErrorV1 as Manifest,
    };
    for error in [
        approval_error(ApprovalError::Codec(Policy::Framing(Framing::Charge(
            Resource::Arithmetic,
        )))),
        runtime_error(RuntimeError::Codec(Manifest::Charge(Resource::Accounting))),
        backing_error(BackingError::Runtime(RuntimeError::Resource(
            Resource::Allocation,
        ))),
    ] {
        assert!(matches!(error, Error::Resource(_)));
    }
}
