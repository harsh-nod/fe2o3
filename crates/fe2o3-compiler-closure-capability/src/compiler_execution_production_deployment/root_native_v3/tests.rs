use super::*;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn root_identity_rejects_every_changed_real_saved_effective_and_filesystem_id() {
    assert!(root_ids([0; 6], 0, 0));
    for index in 0..6 {
        let mut ids = [0; 6];
        ids[index] = 1;
        assert!(!root_ids(ids, 0, 0));
    }
    for changed in [-1, 1, libc::c_long::MAX] {
        assert!(!root_ids([0; 6], changed, 0));
        assert!(!root_ids([0; 6], 0, changed));
    }
}

fn fixture_bytes(seed: u8) -> [Vec<u8>; 3] {
    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
        CompilerExecutionIssuerMeasurementV1 as Measurement,
    };
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
        Measurement::new([seed + 4; 32], 1024).unwrap(),
        Measurement::new([seed + 5; 32], 2048).unwrap(),
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
        Measurement::new([seed + 6; 32], 4096).unwrap(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    let (profile, storage) = Profile::new(61_001, 61_002, service, policy, &mut budget).unwrap();
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    [
        profile.canonical_bytes().to_vec(),
        supervisor.canonical_bytes().to_vec(),
        anchor.canonical_bytes().to_vec(),
    ]
}

struct PrivateRootFixture;
impl PrivateRootFixture {
    const ROOT: &'static str = "/etc/fe2o3";
    const DIRECTORY: &'static str = "/etc/fe2o3/compiler-execution";
    fn new() -> Self {
        require_root().expect("this explicit fixture requires exact root IDs");
        assert_eq!(
            std::env::var("FE2O3_ROOT_NATIVE_PRIVATE_FIXTURE").as_deref(),
            Ok("1")
        );
        let host: u64 = std::env::var("FE2O3_ROOT_NATIVE_HOST_MNTNS_INODE")
            .expect("record original host mount namespace before unshare")
            .parse()
            .unwrap();
        assert_ne!(host, 0);
        assert_ne!(
            host,
            fs::metadata("/proc/thread-self/ns/mnt").unwrap().ino(),
            "fixture must not run in original host mount namespace"
        );
        assert_eq!(
            rustix::fs::fstatfs(&File::open("/etc").unwrap())
                .unwrap()
                .f_type as i64,
            libc::TMPFS_MAGIC as i64,
            "fixture requires a private tmpfs /etc"
        );
        assert_eq!(
            fs::symlink_metadata(Self::ROOT).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        fs::create_dir(Self::ROOT).unwrap();
        let value = Self;
        fs::set_permissions(Self::ROOT, fs::Permissions::from_mode(0o755)).unwrap();
        fs::create_dir(Self::DIRECTORY).unwrap();
        fs::set_permissions(Self::DIRECTORY, fs::Permissions::from_mode(0o755)).unwrap();
        value.install(&fixture_bytes(0x41));
        value
    }
    fn path(index: usize) -> PathBuf {
        Path::new(Self::DIRECTORY).join(FILES[index].0)
    }
    fn write(index: usize, bytes: &[u8]) {
        let path = Self::path(index);
        if path.exists() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o444)).unwrap();
    }
    fn install(&self, bytes: &[Vec<u8>; 3]) {
        for (index, bytes) in bytes.iter().enumerate() {
            Self::write(index, bytes);
        }
    }
}
impl Drop for PrivateRootFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(Self::ROOT);
    }
}

#[test]
#[ignore = "explicit private tmpfs /etc root fixture; never install on host"]
fn private_root_fixed_native_installation_positive_and_replacement_controls() {
    let fixture = PrivateRootFixture::new();
    let original = fixture_bytes(0x41);
    let replacement = fixture_bytes(0x51);
    // Exercise the public fixed path, original account, and namespace checks.
    actual_root_installed_native_deployment_revalidates_original_account();
    for index in 0..3 {
        fixture.install(&original);
        PrivateRootFixture::write(index, &replacement[index]);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        assert!(
            RootProductionCompilerExecutionDeploymentV3::open(&mut budget).is_err(),
            "cross-policy native member {index} must reject"
        );
    }
    for index in 0..3 {
        fixture.install(&original);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (retained, storage) =
            RootProductionCompilerExecutionDeploymentV3::open(&mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let old = PrivateRootFixture::path(index).with_extension("old");
        fs::rename(PrivateRootFixture::path(index), &old).unwrap();
        PrivateRootFixture::write(index, &original[index]);
        assert!(
            retained.revalidate(&mut budget).is_err(),
            "identical-byte path replacement {index}"
        );
        drop(retained);
        fs::remove_file(old).unwrap();
    }
    fixture.install(&original);
    {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (retained, storage) =
            RootProductionCompilerExecutionDeploymentV3::open(&mut budget).unwrap();
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        let directory = Path::new(PrivateRootFixture::DIRECTORY);
        let old = directory.with_extension("old");
        fs::rename(directory, &old).unwrap();
        fs::create_dir(directory).unwrap();
        fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).unwrap();
        fixture.install(&original);
        assert!(
            retained.revalidate(&mut budget).is_err(),
            "ancestor replacement must reject"
        );
        drop(retained);
        fs::remove_dir_all(old).unwrap();
    }
    for mode in [0o644, 0o666] {
        fixture.install(&original);
        fs::set_permissions(
            PrivateRootFixture::path(0),
            fs::Permissions::from_mode(mode),
        )
        .unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        assert!(RootProductionCompilerExecutionDeploymentV3::open(&mut budget).is_err());
    }
    fixture.install(&original);
    let legacy = PrivateRootFixture::path(0).with_file_name("client-profile-v1");
    fs::rename(PrivateRootFixture::path(0), &legacy).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(RootProductionCompilerExecutionDeploymentV3::open(&mut budget).is_err());
}

#[test]
fn root_admission_budget_denials_precede_credentials_or_fixed_path_io() {
    type Deployment<'w> = RootProductionCompilerExecutionDeploymentV3<'w>;
    let mut work = Work::new(Deployment::IO_WORK - 1);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(17).unwrap();
    assert!(matches!(
        Deployment::open(&mut budget),
        Err(ErrorV3::Resource(_))
    ));
    assert_eq!(budget.storage(), 17);

    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, Deployment::IO_STORAGE - 1);
    assert!(matches!(
        Deployment::open(&mut budget),
        Err(ErrorV3::Resource(_))
    ));
    assert_eq!(budget.work(), Deployment::IO_WORK);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn unprivileged_caller_cannot_admit_root_configuration() {
    if rustix::process::getuid().is_root() {
        return;
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(
        matches!(RootProductionCompilerExecutionDeploymentV3::open(&mut budget),
        Err(ErrorV3::Filesystem(ref message)) if message.contains("exact stable root"))
    );
    assert_eq!(
        budget.work(),
        RootProductionCompilerExecutionDeploymentV3::IO_WORK
    );
    assert_eq!(
        budget.storage(),
        RootProductionCompilerExecutionDeploymentV3::IO_STORAGE
    );
}

struct Tree(PathBuf);
impl Tree {
    const ROSTER: [(&'static str, usize); 3] = [
        ("client-profile-v3", 3),
        ("supervisor-deployment-v3", 3),
        ("anchor-deployment-v3", 3),
    ];
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fe2o3-root-native-tree-{}-{}",
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
        for (name, _) in Self::ROSTER {
            fs::write(directory.join(name), b"abc").unwrap();
            fs::set_permissions(directory.join(name), fs::Permissions::from_mode(0o444)).unwrap();
        }
        Self(root)
    }
    fn leaf(&self) -> PathBuf {
        self.0
            .join("etc/fe2o3/compiler-execution/client-profile-v3")
    }
    fn open(&self) -> std::result::Result<(InstalledTree, Vec<Vec<u8>>), String> {
        // Filesystem mechanics only: no root owner or V3 protocol admission is constructed.
        InstalledTree::open_with_read_mode(
            &self.0,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
            Self::ROSTER,
            true,
        )
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn bounded_installed_reads_reject_short_or_trailing_payload() {
    let tree = Tree::new();
    let file = File::open(tree.leaf()).unwrap();
    assert_eq!(
        super::super::read_installed(&file, 3, true).unwrap(),
        b"abc"
    );
    assert!(super::super::read_installed(&file, 2, true).is_err());
    assert!(super::super::read_installed(&file, 4, true).is_err());
    assert!(super::super::read_installed(&file, 0, true).is_err());
}

#[test]
fn bounded_native_tree_retains_current_path_and_exact_roster() {
    let tree = Tree::new();
    let (retained, bytes) = tree.open().unwrap();
    assert!(retained.single_read);
    assert_eq!(retained.file_roster, Tree::ROSTER);
    assert_eq!(bytes, vec![b"abc".to_vec(); 3]);
    retained.revalidate(&[b"abc", b"abc", b"abc"]).unwrap();
    fs::rename(tree.leaf(), tree.leaf().with_extension("old")).unwrap();
    fs::write(tree.leaf(), b"abc").unwrap();
    fs::set_permissions(tree.leaf(), fs::Permissions::from_mode(0o444)).unwrap();
    assert!(retained.revalidate(&[b"abc", b"abc", b"abc"]).is_err());
}

#[test]
fn bounded_native_tree_rejects_byte_drift_and_legacy_only_roster() {
    let tree = Tree::new();
    let (retained, _) = tree.open().unwrap();
    assert!(retained.revalidate(&[b"abd", b"abc", b"abc"]).is_err());
    fs::rename(tree.leaf(), tree.leaf().with_file_name("client-profile-v1")).unwrap();
    assert!(tree.open().is_err());
}

#[test]
#[ignore = "requires independently provisioned root-owned fixed V3 deployment and exact root"]
fn actual_root_installed_native_deployment_revalidates_original_account() {
    let mut other_work = Work::new(usize::MAX);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(31).unwrap();
    let (value, storage) = RootProductionCompilerExecutionDeploymentV3::open(&mut budget).unwrap();
    assert_eq!(budget.storage(), 31);
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    value.revalidate(&mut budget).unwrap();
    let mut other = Budget::new(&mut other_work, 1_000_000);
    other.reserve_storage(storage.additional_storage()).unwrap();
    assert!(matches!(
        value.revalidate(&mut other),
        Err(ErrorV3::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 31 + storage.additional_storage());
}
