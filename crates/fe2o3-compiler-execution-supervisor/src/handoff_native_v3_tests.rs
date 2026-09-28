use super::ProtectedIssuerHandoffErrorV3 as HandoffError;
use super::*;
use crate::authority_v2_test_process::{IO_TIMEOUT, pair, send_packet};
use crate::handoff_v2::tests::references;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::fd::AsFd;

include!("handoff_native_io_tests.rs");

#[test]
fn shared_wire_decodes_as_v3_but_rejects_an_actual_v2_policy_identity() {
    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy,
        CompilerExecutionServiceLaunchManifestV2 as OtherManifest,
        CompilerExecutionSupervisorHandoffV2 as OtherFrame,
        sealed_static_issuer_runtime_measurement_v1,
    };
    let fixture = crate::tests::Fixture::new("v3-handoff-wire-family");
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let account = budget.work_ledger_identity_v1();
    let policy = crate::program_v3::tests::policy(
        fixture.issuer_measurement(),
        sealed_static_issuer_runtime_measurement_v1(),
        7,
        &mut budget,
    );
    let (other, delta) = OtherPolicy::new(
        7,
        fixture.issuer_measurement(),
        sealed_static_issuer_runtime_measurement_v1(),
        SigningKey::from_bytes(&crate::program_v3::tests::SEED)
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_ne!(policy.identity().as_bytes(), other.identity().as_bytes());
    let client = Client::new(43, 65_532, 65_532).unwrap();
    let submitter = Client::new(42, 65_532, 65_532).unwrap();
    let anchor = Anchor::new(65_534, 65_534).unwrap();
    let (manifest, delta) = OtherManifest::new(client, anchor, &other, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let (frame, delta) = OtherFrame::new(submitter, manifest, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let (send, receive) = pair();
    let (first, second) = pair();
    send_packet(
        &send,
        frame.canonical_bytes(),
        &[first.as_fd(), second.as_fd()],
    )
    .unwrap();
    let (payload, rights) = transport::receive(&receive, Instant::now() + IO_TIMEOUT).unwrap();
    budget
        .reserve_storage(BYTES + 2 * Accepted::CONTROL_STORAGE)
        .unwrap();
    let floor = budget.storage();
    let (decoded, delta) = Frame::decode(&payload, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let manifest: &Manifest = decoded.launch_manifest();
    let start = budget.work();
    let floor = budget.storage();
    assert!(!manifest.matches_policy(&policy, &mut budget).unwrap());
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        budget.work() - start,
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3
    );
    assert!(budget.work_ledger_identity_v1() == account);
    drop(rights);
    let (manifest, delta) = Manifest::new(client, anchor, &policy, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let (frame, delta) = Frame::new(submitter, manifest, &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let (decoded, delta) = Frame::decode(frame.canonical_bytes(), &mut budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert!(
        decoded
            .launch_manifest()
            .matches_policy(&policy, &mut budget)
            .unwrap()
    );
}

#[path = "handoff_native_v3_fixture_tests.rs"]
pub(crate) mod fixture;
