//! Exact inert readiness joins, not protected child/profile execution evidence.
use super::*;
use crate::program_v3::tests::{SEED, policy};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3 as SCRATCH,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3 as WORK,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerPolicyV2 as OldPolicy,
    CompilerExecutionServiceLaunchManifestV2 as OldManifest,
    CompilerExecutionServiceReadyV2 as OldReadiness,
    sealed_static_issuer_runtime_measurement_v1 as runtime,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const WORK_LIMIT: usize = 1_000_000;
const STORAGE_LIMIT: usize = 10_000_000;
const PID: u32 = 91;

fn manifest(policy: &Policy, client_pid: u32, b: &mut Budget<'_>) -> Manifest {
    let (manifest, delta) = Manifest::new(
        Client::new(client_pid, 65_532, 65_532).unwrap(),
        Anchor::new(65_534, 65_534).unwrap(),
        policy,
        b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    manifest
}

fn readiness(manifest: &Manifest, policy: &Policy, b: &mut Budget<'_>) -> Readiness {
    let (ready, delta) = Readiness::new(PID, manifest, policy, b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    ready
}

#[test]
fn readiness_join_checks_exact_pid_launch_policy_and_actual_v2_wire() {
    let fixture = crate::tests::Fixture::new("v3-process-readiness");
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.charge_work(17).unwrap();
    b.reserve_storage(19).unwrap();
    let account = b.work_ledger_identity_v1();
    let p = policy(fixture.issuer_measurement(), runtime(), 7, &mut b);
    let other_policy = policy(fixture.issuer_measurement(), runtime(), 8, &mut b);
    let launch = manifest(&p, 43, &mut b);
    let other_launch = manifest(&p, 44, &mut b);
    let ready = readiness(&launch, &p, &mut b);
    for (pid, launch, policy, valid) in [
        (PID, &launch, &p, true),
        (PID + 1, &launch, &p, false),
        (PID, &other_launch, &p, false),
        (PID, &launch, &other_policy, false),
    ] {
        let floor = b.storage();
        let start = b.work();
        let result = validate_readiness_binding(&ready, pid, launch, policy, &mut b);
        if valid {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(Error::State(
                    "native readiness names another launch or policy"
                ))
            ));
        }
        assert_eq!(b.work() - start, WORK);
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == account);
    }

    let (old_policy, delta) = OldPolicy::new(
        7,
        fixture.issuer_measurement(),
        runtime(),
        SigningKey::from_bytes(&SEED).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let (old_launch, delta) = OldManifest::new(
        launch.client(),
        launch.external_anchor_service(),
        &old_policy,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let (old_ready, delta) = OldReadiness::new(PID, &old_launch, &old_policy, &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    b.reserve_storage(READY_BYTES).unwrap();
    let (decoded, delta) = Readiness::decode(old_ready.canonical_bytes(), &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let floor = b.storage();
    let start = b.work();
    assert!(matches!(
        validate_readiness_binding(&decoded, PID, &launch, &p, &mut b),
        Err(Error::State(
            "native readiness names another launch or policy"
        ))
    ));
    assert_eq!(b.work() - start, WORK);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == account);
}

#[test]
fn readiness_join_preserves_original_account_on_input_and_quota_refusals() {
    for fault in ["input", "work", "scratch"] {
        let fixture = crate::tests::Fixture::new("v3-process-readiness-resource");
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.charge_work(17).unwrap();
        b.reserve_storage(19).unwrap();
        let account = b.work_ledger_identity_v1();
        let p = policy(fixture.issuer_measurement(), runtime(), 7, &mut b);
        let launch = manifest(&p, 43, &mut b);
        let ready = readiness(&launch, &p, &mut b);
        match fault {
            "input" => b.release_storage(20).unwrap(),
            "work" => b.charge_work(WORK_LIMIT - b.work() - (WORK - 1)).unwrap(),
            _ => b
                .reserve_storage(STORAGE_LIMIT - b.storage() - SCRATCH + 1)
                .unwrap(),
        }
        let floor = b.storage();
        let start = b.work();
        let result = validate_readiness_binding(&ready, PID, &launch, &p, &mut b);
        match fault {
            "input" => {
                assert!(matches!(
                    result,
                    Err(Error::Readiness(ReadyError::Resource(Resource::Accounting)))
                ));
                assert_eq!(b.work() - start, 8);
            }
            "work" => {
                assert!(matches!(
                    result,
                    Err(Error::Readiness(ReadyError::Resource(Resource::Work(_))))
                ));
                assert_eq!(b.work() - start, 8);
                assert_eq!(b.failed_work(), Some(WORK_LIMIT + 1));
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(Error::Readiness(ReadyError::Resource(Resource::Storage(_))))
                ));
                assert_eq!(b.work() - start, WORK);
                assert_eq!(b.failed_storage(), Some(STORAGE_LIMIT + 1));
            }
        }
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == account);
    }
}
