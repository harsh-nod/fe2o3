use super::*;
use crate::deployment::native_manager::{
    CONFIG_PATH as MANAGER_CONFIG, IMAGE_PATH as MANAGER_IMAGE,
    NativeApplicationManagerConfigurationV1 as ManagerConfig,
    ProductionNativeApplicationManagerDeploymentV1 as Manager,
};
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Compiler;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProfileV3 as Profile,
    CompilerExecutionExternalAnchorDeploymentV3 as Anchor,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Supervisor,
};

pub(super) fn compiler_records() -> ([Vec<u8>; 3], [u8; 32]) {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let (policy, charge) = Policy::new(
        7,
        Measurement::new([21; 32], 123).unwrap(),
        Measurement::new([22; 32], 456).unwrap(),
        ed25519_dalek::SigningKey::from_bytes(&[23; 32])
            .verifying_key()
            .to_bytes(),
        ed25519_dalek::SigningKey::from_bytes(&[24; 32])
            .verifying_key()
            .to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let identity = *policy.identity().as_bytes();
    let service = Service::new(61003, 61004).unwrap();
    let (supervisor, charge) = Supervisor::new(
        61001,
        61002,
        service,
        Measurement::new([25; 32], 1024).unwrap(),
        Measurement::new([26; 32], 2048).unwrap(),
        &policy,
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (anchor, charge) = Anchor::new(
        &supervisor,
        &policy,
        Measurement::new([27; 32], 4096).unwrap(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (profile, charge) = Profile::new(61001, 61002, service, policy, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    (
        [
            profile.canonical_bytes().to_vec(),
            supervisor.canonical_bytes().to_vec(),
            anchor.canonical_bytes().to_vec(),
        ],
        identity,
    )
}

fn install(fixture: &Fixture) -> Vec<u8> {
    fixture.install();
    let directory = "/etc/fe2o3/compiler-execution";
    fs::create_dir(directory).unwrap();
    fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).unwrap();
    for (name, bytes) in [
        "client-profile-v3",
        "supervisor-deployment-v3",
        "anchor-deployment-v3",
    ]
    .into_iter()
    .zip(&fixture.compiler)
    {
        Fixture::write(&format!("{directory}/{name}"), bytes, 0o444);
    }
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(fixture.config.len()).unwrap();
    let (proof, charge) = Config::decode(&fixture.config, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (manager, charge) = ManagerConfig::new(
        (
            Sha256::digest(&fixture.image).into(),
            fixture.image.len() as u64,
        ),
        proof.compiler_policy_identity(),
        proof.identity(),
        proof.semantic_policy(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    // A pinned static image is sufficient for installed-image custody, not for
    // running-manager admission. This fixture never executes the substituted image.
    Fixture::write(MANAGER_IMAGE, &fixture.image, 0o555);
    Fixture::write(MANAGER_CONFIG, manager.canonical_bytes(), 0o444);
    manager.canonical_bytes().to_vec()
}

#[test]
#[ignore = "real root and private tmpfs mounts only; use qualify-native-proof-deployment.sh run-manager"]
fn fixed_native_manager_installation_and_original_owner_controls() {
    let fixture = Fixture::new();
    let config = install(&fixture);
    for replacement in [None, Some(MANAGER_CONFIG), Some(MANAGER_IMAGE)] {
        Fixture::write(MANAGER_IMAGE, &fixture.image, 0o555);
        Fixture::write(MANAGER_CONFIG, &config, 0o444);
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, STORAGE);
        b.reserve_storage(73).unwrap();
        let (compiler, charge) = Compiler::open(&mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let (proof, charge) = Deployment::open(&mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let floor = b.storage();
        let (manager, charge) = Manager::open(&compiler, &proof, &mut b).unwrap();
        assert_eq!(b.storage(), floor, "full owner charge is unreserved");
        b.reserve_storage(charge.additional_storage()).unwrap();
        let retained = b.storage();
        manager.revalidate(&mut b).unwrap();
        assert_eq!(b.storage(), retained);
        if let Some(path) = replacement {
            let old = format!("{path}.original");
            fs::rename(path, &old).unwrap();
            let (bytes, mode) = if path == MANAGER_IMAGE {
                (&fixture.image, 0o555)
            } else {
                (&config, 0o444)
            };
            Fixture::write(path, bytes, mode);
            assert!(
                manager.revalidate(&mut b).is_err(),
                "same-byte replacement: {path}"
            );
            fs::remove_file(old).unwrap();
        } else {
            let mut other_work = Work::new(WORK);
            let mut other = Budget::new(&mut other_work, STORAGE);
            other.reserve_storage(retained).unwrap();
            assert!(
                manager.revalidate(&mut other).is_err(),
                "original account differs"
            );
            b.release_storage(charge.additional_storage()).unwrap();
            assert!(
                manager.revalidate(&mut b).is_err(),
                "unpaid manager custody"
            );
            b.reserve_storage(charge.additional_storage()).unwrap();
            manager.revalidate(&mut b).unwrap();
            assert!(
                manager.admit_running(&mut b).is_err(),
                "installed bytes are not a sealed running manager"
            );
        }
    }
    Fixture::write(MANAGER_IMAGE, &fixture.image, 0o555);
    Fixture::write(MANAGER_CONFIG, &config, 0o666);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let (compiler, charge) = Compiler::open(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (proof, charge) = Deployment::open(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert!(
        Manager::open(&compiler, &proof, &mut b).is_err(),
        "writable manager configuration"
    );
}
