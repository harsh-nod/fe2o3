use super::*;

#[test]
fn request_quotes_fund_original_inputs_two_native_slots_and_every_refusal_turn() {
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
    let (cleanup_work, cleanup_storage) = RootCompilerRequest::cleanup_growth().unwrap();
    assert_eq!(
        first.cleanup_storage(),
        old_first.cleanup_storage() + cleanup_storage
    );
    assert_eq!(
        first.cleanup_work(),
        old_first.cleanup_work() + cleanup_work
    );
    assert!(RootCompilerRequest::launch_quota().unwrap().scratch() > admission.scratch());
    let guard = Prepared::maximum_cleanup_guard_quota().unwrap();
    let launch = RootCompilerRequest::launch_quota().unwrap();
    assert!(launch.work() >= 2 * guard.work());
    assert!(launch.scratch() >= guard.scratch());
    assert!(cleanup_work >= 2 * Cleanup::GUARD_CLONE_WORK);
    assert_eq!(
        next.request_work() - first.request_work(),
        old_next.request_work() - old_first.request_work()
            + Receiver::TURN_WORK
            + Prepared::maximum_revalidation_quota().unwrap().work()
            + refusal.work()
    );
}

#[test]
fn helper_peer_separation_rejects_either_alias_not_only_equal_pairs() {
    let helper = Credentials::new(42001, 42002).unwrap();
    for (uid, gid) in [(42001, 42004), (42003, 42002), (42001, 42002)] {
        assert!(matches!(
            require_separate_helper_peer(helper, Credentials::new(uid, gid).unwrap()),
            Err(Error::Invalid {
                reason: "proof helper and original compiler peer credentials overlap",
                ..
            })
        ));
    }
    require_separate_helper_peer(helper, Credentials::new(42003, 42004).unwrap()).unwrap();
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
