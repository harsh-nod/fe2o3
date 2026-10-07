use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

type Deployment<'work> = ProductionCompilerExecutionDeploymentV3<'work>;
const PREFIX: usize = 79;

struct Configuration {
    profile: Profile,
    supervisor: Supervisor,
    anchor: Anchor,
}
impl Configuration {
    fn new(seed: u8) -> Self {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (policy, storage) = Policy::new(
            7,
            Measurement::new([seed; 32], 123).unwrap(),
            Measurement::new([seed + 1; 32], 456).unwrap(),
            SigningKey::from_bytes(&[seed + 2; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[seed + 3; 32])
                .verifying_key()
                .to_bytes(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let service = Service::new(61_003, 61_004).unwrap();
        let (supervisor, storage) = Supervisor::new(
            61_001,
            61_002,
            service,
            Measurement::new([0x31; 32], 1024).unwrap(),
            Measurement::new([0x32; 32], 2048).unwrap(),
            &policy,
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let (anchor, storage) = Anchor::new(
            &supervisor,
            &policy,
            Measurement::new([0x33; 32], 4096).unwrap(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let (profile, storage) =
            Profile::new(61_001, 61_002, service, policy, &mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        Self {
            profile,
            supervisor,
            anchor,
        }
    }
    fn bytes(&self) -> [&[u8]; 3] {
        [
            self.profile.canonical_bytes(),
            self.supervisor.canonical_bytes(),
            self.anchor.canonical_bytes(),
        ]
    }
}

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fe2o3-native-production-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let mut directory = root.clone();
        for component in super::super::DIRECTORIES {
            directory.push(component);
            fs::create_dir(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let value = Self(root);
        for (index, bytes) in Configuration::new(0x51).bytes().into_iter().enumerate() {
            value.write(index, bytes);
        }
        value
    }
    fn path(&self, index: usize) -> PathBuf {
        self.0
            .join("etc/fe2o3/compiler-execution")
            .join(FILES[index].0)
    }
    fn write(&self, index: usize, bytes: &[u8]) {
        let path = self.path(index);
        if path.exists() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o444)).unwrap();
    }
    fn open<'work>(
        &self,
        budget: &mut Budget<'work>,
    ) -> Result<(
        Deployment<'work>,
        ProductionCompilerExecutionDeploymentStorageV3,
    )> {
        // Test-only alternate tree ownership does not qualify the public root-owned path.
        Deployment::open_tree(
            &self.0,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
            budget,
        )
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_configuration_keeps_original_files_policy_and_account() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Deployment<'_>>();
    let tree = Tree::new();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(PREFIX).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (value, storage) = tree.open(&mut budget).unwrap();
    assert!(value.tree.single_read && value.namespace.single_read);
    assert_eq!(budget.storage(), PREFIX);
    assert_eq!(
        storage.additional_storage(),
        value.retained_storage().unwrap()
    );
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    assert_eq!(value.policy(), Configuration::new(0x51).profile.policy());
    let identity = super::super::identity(&value.tree.files[0].file).unwrap();
    value.revalidate(&mut budget).unwrap();
    assert_eq!(
        identity,
        super::super::identity(&value.tree.files[0].file).unwrap()
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), PREFIX + storage.additional_storage());
    drop(value);
    budget
        .release_storage(storage.additional_storage())
        .unwrap();
    assert_eq!(budget.storage(), PREFIX);
}

#[test]
fn native_configuration_rejects_reciprocal_policy_member_substitution() {
    for member in 0..3 {
        let tree = Tree::new();
        tree.write(member, Configuration::new(0x61).bytes()[member]);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(PREFIX).unwrap();
        assert!(tree.open(&mut budget).is_err());
        assert!(budget.storage() >= PREFIX + Deployment::IO_STORAGE);
        assert!(budget.work() >= Deployment::IO_WORK);
    }
}

#[test]
fn native_configuration_rejects_legacy_wire_even_under_native_names() {
    let tree = Tree::new();
    let policy = fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV1::new(
        7,
        Measurement::new([0x51; 32], 123).unwrap(),
        Measurement::new([0x52; 32], 456).unwrap(),
        SigningKey::from_bytes(&[0x53; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x54; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap();
    let old = fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1::new(
        61_001,
        61_002,
        Service::new(61_003, 61_004).unwrap(),
        policy,
    )
    .unwrap();
    tree.write(0, old.canonical_bytes());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(matches!(
        tree.open(&mut budget),
        Err(ProductionCompilerExecutionDeploymentErrorV3::Profile(_))
    ));
}

#[test]
fn native_configuration_never_falls_back_to_legacy_path() {
    let tree = Tree::new();
    for index in 0..3 {
        fs::rename(
            tree.path(index),
            tree.path(index)
                .with_file_name(super::super::FILES[index].0),
        )
        .unwrap();
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(matches!(
        tree.open(&mut budget),
        Err(ProductionCompilerExecutionDeploymentErrorV3::Filesystem(_))
    ));
}

#[test]
fn original_native_paths_bytes_and_metadata_are_revalidated() {
    for mutation in 0..3 {
        let tree = Tree::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (value, storage) = tree.open(&mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        match mutation {
            0 => {
                let bytes = value.profile.canonical_bytes().to_vec();
                fs::rename(
                    tree.path(0),
                    tree.path(0).with_file_name("retired-native-profile"),
                )
                .unwrap();
                tree.write(0, &bytes);
            }
            1 => tree.write(0, Configuration::new(0x61).profile.canonical_bytes()),
            _ => fs::set_permissions(tree.path(0), fs::Permissions::from_mode(0o666)).unwrap(),
        }
        assert!(value.revalidate(&mut budget).is_err());
        assert!(budget.storage() >= storage.additional_storage() + Deployment::IO_STORAGE);
    }
}

#[test]
fn native_configuration_exact_and_one_short_limits_preserve_terminal_history() {
    let tree = Tree::new();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(PREFIX).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = tree.open(&mut budget);
        let ok = result.is_ok();
        drop(result);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            ok,
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_work(),
            budget.failed_storage(),
        )
    };
    let baseline = run(usize::MAX, 1_000_000);
    assert!(baseline.0);
    assert_eq!(baseline.2, PREFIX);
    let exact = run(baseline.1, baseline.3);
    assert_eq!(exact, baseline);
    let work_short = run(baseline.1 - 1, baseline.3);
    assert!(!work_short.0 && work_short.4.is_some());
    assert!(work_short.2 > PREFIX);
    let storage_short = run(baseline.1, baseline.3 - 1);
    assert!(!storage_short.0 && storage_short.5.is_some());
    assert!(storage_short.2 > PREFIX);
    let entry_short = run(Deployment::IO_WORK - 1, baseline.3);
    assert!(!entry_short.0 && entry_short.4.is_some());
    assert_eq!(entry_short.2, PREFIX);
}

#[test]
fn native_configuration_refuses_a_new_fully_funded_work_account() {
    let tree = Tree::new();
    let mut original_work = Work::new(usize::MAX);
    let mut original = Budget::new(&mut original_work, 1_000_000);
    let (value, storage) = tree.open(&mut original).unwrap();
    original
        .reserve_storage(storage.additional_storage())
        .unwrap();
    let original_storage = original.storage();
    let mut foreign_work = Work::new(usize::MAX);
    let mut foreign = Budget::new(&mut foreign_work, 1_000_000);
    foreign
        .reserve_storage(storage.additional_storage())
        .unwrap();
    assert!(matches!(
        value.revalidate(&mut foreign),
        Err(ProductionCompilerExecutionDeploymentErrorV3::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(foreign.storage(), storage.additional_storage());
    assert_eq!(foreign.work(), Deployment::IO_WORK);
    assert_eq!(original.storage(), original_storage);
    value.revalidate(&mut original).unwrap();
}

#[test]
fn native_app_proof_profile_pins_original_config_policy_and_paths() {
    use super::super::native_proof_profile::{
        ProductionNativeApplicationProofProfileErrorV1 as ProofError,
        ProductionNativeApplicationProofProfileV1 as Proof,
    };
    use fe2o3_compiler_execution_protocol::{
        NativeApplicationProofCustodianConfigurationV1 as Config,
        NativeProofCustodianConfigurationPartsV1 as Parts,
    };
    use sha2::Digest;
    for mutation in 0..7 {
        let tree = Tree::new();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 2_000_000);
        let (compiler, charge) = tree.open(&mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let mut parts = Parts {
            credentials: (62_021, 62_022),
            controller: ([1; 32], 1234),
            analyzer_executable: ([2; 32], 5678),
            analyzer_runtime_closure: ([3; 32], 9012),
            analyzer_identity: [4; 32],
            toolchain_identity: [5; 32],
            verus_identity: [6; 32],
            compiler_policy_identity: *compiler.policy().identity().as_bytes(),
            semantic_policy: (sha2::Sha256::digest([8; 648]).into(), 648),
        };
        if mutation == 1 {
            parts.compiler_policy_identity = [99; 32];
        }
        if mutation == 2 {
            parts.credentials.0 = compiler.profile().supervisor_uid();
        }
        if mutation == 3 {
            parts.semantic_policy.1 = 131_689;
        }
        let (config, _) = Config::new(parts, &mut budget).unwrap();
        let directory = tree.0.join("etc/fe2o3/proof-custodian");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        let file = directory.join("application-native-deployment-v1");
        fs::write(&file, config.canonical_bytes()).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
        let policy_file = directory.join("native-conditional-root-policy-v1");
        fs::write(&policy_file, [if mutation == 5 { 9 } else { 8 }; 648]).unwrap();
        fs::set_permissions(&policy_file, fs::Permissions::from_mode(0o444)).unwrap();
        let result = Proof::open_test_tree(
            &tree.0,
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
            &compiler,
            &mut budget,
        );
        if (1..=3).contains(&mutation) || mutation == 5 {
            assert!(matches!(result, Err(ProofError::Binding)));
            continue;
        }
        let (profile, charge) = result.unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(
            profile.configuration().canonical_bytes(),
            config.canonical_bytes()
        );
        profile.revalidate(&compiler, &mut budget).unwrap();
        assert_eq!(profile.semantic_policy_bytes(), &[8; 648]);
        if mutation == 6 {
            fs::rename(&policy_file, directory.join("old-native-policy")).unwrap();
            fs::write(&policy_file, [8; 648]).unwrap();
            fs::set_permissions(&policy_file, fs::Permissions::from_mode(0o444)).unwrap();
            assert!(matches!(
                profile.revalidate(&compiler, &mut budget),
                Err(ProofError::Filesystem(_))
            ));
        } else if mutation == 4 {
            fs::rename(&file, directory.join("old-native-profile")).unwrap();
            fs::write(&file, config.canonical_bytes()).unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
            assert!(matches!(
                profile.revalidate(&compiler, &mut budget),
                Err(ProofError::Filesystem(_))
            ));
        } else {
            let mut foreign_work = Work::new(usize::MAX);
            let mut foreign = Budget::new(&mut foreign_work, 2_000_000);
            foreign.reserve_storage(budget.storage()).unwrap();
            assert!(matches!(
                profile.revalidate(&compiler, &mut foreign),
                Err(ProofError::Resource(Resource::Accounting))
            ));
            assert_eq!(foreign.work(), 0);
        }
    }
}
