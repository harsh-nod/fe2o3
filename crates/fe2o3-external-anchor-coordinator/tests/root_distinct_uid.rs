use std::fs::{self, File};
use std::time::{Duration, Instant};

use ed25519_dalek::SigningKey;
use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV1;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
};
use fe2o3_external_anchor_coordinator::PreparedExternalAnchorOccurrenceV1;
use fe2o3_external_anchor_protocol::{
    AnchorDecisionV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1, PinnedAnchorKeyV1,
    TransactionDigestV1,
};
use fe2o3_external_anchor_provisioner::ExternalAnchorProvisioningReadyDispositionV1;
use rustix::process::{Gid, Uid};
use sha2::{Digest, Sha256};

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
#[ignore = "requires root plus FE2O3_ROOT_ANCHOR_HELPER/DAEMON/UID/GID"]
fn real_distinct_uid_helper_daemon_exchange_and_restart() {
    assert!(rustix::process::geteuid().is_root());
    assert!(rustix::process::getegid().is_root());
    let helper_path = std::env::var_os("FE2O3_ROOT_ANCHOR_HELPER").unwrap();
    let daemon_path = std::env::var_os("FE2O3_ROOT_ANCHOR_DAEMON").unwrap();
    let service_uid = parse_id("FE2O3_ROOT_ANCHOR_UID");
    let service_gid = parse_id("FE2O3_ROOT_ANCHOR_GID");
    let helper_measurement = measurement(&fs::read(&helper_path).unwrap());
    let daemon_measurement = measurement(&fs::read(&daemon_path).unwrap());
    let state_root = tempfile::tempdir().unwrap();
    fs::set_permissions(
        state_root.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .unwrap();
    rustix::fs::chown(
        state_root.path(),
        Some(Uid::from_raw(service_uid)),
        Some(Gid::from_raw(service_gid)),
    )
    .unwrap();

    let (deployment, provisioning, key_template, pinned, supervisor, policy) = manifests(
        service_uid,
        service_gid,
        daemon_measurement,
        helper_measurement,
    );
    let first = PreparedExternalAnchorOccurrenceV1::prepare(
        File::open(&helper_path).unwrap(),
        File::open(&daemon_path).unwrap(),
        File::open(state_root.path()).unwrap(),
        deployment.clone(),
        provisioning.clone(),
        key_template,
    )
    .unwrap()
    .launch(TIMEOUT)
    .unwrap();
    assert_eq!(
        first.disposition(),
        ExternalAnchorProvisioningReadyDispositionV1::Initialized
    );
    first.validate_continuity().unwrap();
    let transfer = first
        .try_clone_for_supervisor(&supervisor, &policy)
        .unwrap();
    let (endpoint, pidfd) = transfer.into_ordered_descriptors();
    let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
        .prepare(TransactionDigestV1::from_bytes([0x42; 32]), &pinned)
        .unwrap()
        .begin_advance(CallerNonceV1::from_bytes([0x24; 32]), &pinned)
        .unwrap();
    let challenge = pending.challenge().as_bytes();
    send_exact(&endpoint, challenge);
    let mut observation = [0_u8; fe2o3_external_anchor_protocol::ANCHOR_OBSERVATION_WIRE_LEN_V1];
    receive_exact(&endpoint, &mut observation);
    assert!(matches!(
        pending.verify(&observation).unwrap(),
        AnchorDecisionV1::Commit(_)
    ));
    drop(endpoint);
    drop(pidfd);
    first.shutdown().unwrap();
    assert!(state_root.path().join("anchor-state-v1").is_file());

    let (_, _, second_key_template, _, _, _) = manifests(
        service_uid,
        service_gid,
        daemon_measurement,
        helper_measurement,
    );
    let second = PreparedExternalAnchorOccurrenceV1::prepare(
        File::open(helper_path).unwrap(),
        File::open(daemon_path).unwrap(),
        File::open(state_root.path()).unwrap(),
        deployment,
        provisioning,
        second_key_template,
    )
    .unwrap()
    .launch(TIMEOUT)
    .unwrap();
    assert_eq!(
        second.disposition(),
        ExternalAnchorProvisioningReadyDispositionV1::Existing
    );
    second.validate_continuity().unwrap();
    second.shutdown().unwrap();
}

fn parse_id(name: &str) -> u32 {
    let value = std::env::var(name).unwrap().parse::<u32>().unwrap();
    assert!(value != 0 && value != u32::MAX);
    value
}

fn measurement(bytes: &[u8]) -> CompilerExecutionIssuerMeasurementV1 {
    CompilerExecutionIssuerMeasurementV1::new(Sha256::digest(bytes).into(), bytes.len() as u64)
        .unwrap()
}

fn manifests(
    service_uid: u32,
    service_gid: u32,
    daemon: CompilerExecutionIssuerMeasurementV1,
    helper: CompilerExecutionIssuerMeasurementV1,
) -> (
    CompilerExecutionExternalAnchorDeploymentV1,
    CompilerExecutionExternalAnchorProvisioningV1,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV1,
    PinnedAnchorKeyV1,
    CompilerExecutionSupervisorDeploymentV1,
    CompilerExecutionIssuerPolicyV1,
) {
    let mut seed = [0x37; 32];
    let signing = SigningKey::from_bytes(&seed);
    let pinned = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
    let policy = CompilerExecutionIssuerPolicyV1::new(
        1,
        CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([2; 32], 2).unwrap(),
        SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
        signing.verifying_key().to_bytes(),
    )
    .unwrap();
    let service =
        CompilerExecutionExternalAnchorServiceIdentityV1::new(service_uid, service_gid).unwrap();
    let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
        service_uid.checked_add(1).unwrap(),
        service_gid,
        service,
        CompilerExecutionIssuerMeasurementV1::new([3; 32], 3).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([4; 32], 4).unwrap(),
        &policy,
    )
    .unwrap();
    let deployment =
        CompilerExecutionExternalAnchorDeploymentV1::new(&supervisor, &policy, daemon).unwrap();
    let provisioning =
        CompilerExecutionExternalAnchorProvisioningV1::new(&deployment, helper).unwrap();
    let key = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
        &mut seed,
        &deployment,
    )
    .unwrap();
    (deployment, provisioning, key, pinned, supervisor, policy)
}

fn receive_exact(endpoint: &impl std::os::fd::AsFd, bytes: &mut [u8]) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match rustix::net::recv(endpoint, &mut *bytes, rustix::net::RecvFlags::empty()) {
            Ok((count, message_length)) => {
                assert_eq!(count, bytes.len());
                assert_eq!(message_length, bytes.len());
                return;
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("receive anchor observation: {error}"),
        }
    }
}

fn send_exact(endpoint: &impl std::os::fd::AsFd, bytes: &[u8]) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match rustix::net::send(endpoint, bytes, rustix::net::SendFlags::empty()) {
            Ok(count) => {
                assert_eq!(count, bytes.len());
                return;
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("send anchor challenge: {error}"),
        }
    }
}
