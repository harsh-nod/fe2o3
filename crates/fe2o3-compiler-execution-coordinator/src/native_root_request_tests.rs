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
    let (runtime_cleanup_work, runtime_cleanup_storage) =
        RootCompilerRequest::runtime_cleanup_growth(1, 1).unwrap();
    assert_eq!(
        first.cleanup_storage(),
        old_first.cleanup_storage() + cleanup_storage + runtime_cleanup_storage
    );
    assert_eq!(
        first.cleanup_work(),
        old_first.cleanup_work() + cleanup_work + runtime_cleanup_work
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
            + RootCompilerRequest::runtime_turn_quota().unwrap().work()
    );
}

#[test]
fn runtime_turn_and_one_time_issuer_schedule_cover_each_actual_phase() {
    let turn = RootCompilerRequest::runtime_turn_quota().unwrap();
    let startup = RootCompilerRequest::runtime_startup_quota().unwrap();
    let policy = compiler_attempt::Attempt::original_policy_identity_quota().unwrap();
    let continuity = compiler_attempt::Attempt::maximum_continuity_quota().unwrap();
    let step = compiler_attempt::Attempt::runtime_step_quota().unwrap();
    let capture = quota::runtime_capture().unwrap();
    assert!(
        turn.work()
            >= RootCompilerRequest::LOCAL_WORK
                + 2 * policy.work()
                + 2 * continuity.work()
                + step.work()
    );
    assert!(turn.work() >= capture.work());
    assert!(turn.scratch() >= step.scratch() + capture.scratch());
    assert!(startup.work() >= Prepared::maximum_cleanup_guard_quota().unwrap().work());
    let one =
        crate::InheritedCompilerExecutionDeploymentV3::original_root_startup_quota(1, 1).unwrap();
    let two =
        crate::InheritedCompilerExecutionDeploymentV3::original_root_startup_quota(2, 1).unwrap();
    assert_eq!(one.request_storage(), two.request_storage());
    let (first, first_storage) = RootCompilerRequest::runtime_cleanup_growth(1, 1).unwrap();
    let (next, next_storage) = RootCompilerRequest::runtime_cleanup_growth(1, 2).unwrap();
    let late = fe2o3_broker_authority_service::RootPublicationCustodyV3::observation_cleanup_quota(
        fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5,
    )
    .unwrap();
    assert_eq!(
        next - first,
        Cleanup::pump_work(fe2o3_protected_service_spawn::MAX_PROTECTED_SERVICE_PROCESSES_V2)
            .unwrap()
            + Cleanup::shutdown_work()
            + late.retirement_work()
    );
    assert_eq!(next_storage, first_storage);
    assert!(first_storage > late.persistent_storage());
    let (monitor, monitor_storage) = RootCompilerRequest::runtime_cleanup_growth(2, 1).unwrap();
    assert_eq!(monitor - first, late.retirement_work());
    assert_eq!(monitor_storage, first_storage);
    for (monitor, cleanup) in [(0, 1), (1, 0), (usize::MAX, 1), (1, usize::MAX)] {
        assert!(RootCompilerRequest::runtime_cleanup_growth(monitor, cleanup).is_err());
    }
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

#[test]
fn runtime_checkpoint_preserves_nested_resource_errors() {
    use crate::native_runtime_guard::Error as Guard;
    use fe2o3_protected_service_spawn::native_spawn::ProtectedServiceSpawnErrorV2 as Spawn;
    for error in [
        Guard::Resource(Resource::Arithmetic),
        Guard::Spawn(Spawn::Resource(Resource::Accounting)),
        Guard::Inventory(crate::native_runtime_inventory::Error::Resource(
            Resource::Allocation,
        )),
        Guard::Descriptor(crate::native_runtime_descriptors::Error::Resource(
            Resource::Accounting,
        )),
        Guard::Inventory(crate::native_runtime_inventory::Error::Backing(
            BackingError::Runtime(RuntimeError::Resource(Resource::Arithmetic)),
        )),
    ] {
        assert!(matches!(
            helper_error(ProofHelperLaunchError::Runtime(error)),
            Error::Resource(_)
        ));
    }
    assert!(matches!(
        helper_error(ProofHelperLaunchError::Runtime(Guard::Invalid("policy"))),
        Error::Invalid { .. }
    ));
}
