use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};

struct Configuration {
    client: CompilerExecutionClientProfileV1,
    supervisor: CompilerExecutionSupervisorDeploymentV1,
    anchor: CompilerExecutionExternalAnchorDeploymentV1,
}

impl Configuration {
    fn new(seed: u8) -> Self {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([seed; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 456).unwrap(),
            SigningKey::from_bytes(&[seed + 2; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[seed + 3; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        Self::with_policy(policy, 61_001, 61_002)
    }

    fn with_policy(policy: CompilerExecutionIssuerPolicyV1, uid: u32, gid: u32) -> Self {
        let service =
            CompilerExecutionExternalAnchorServiceIdentityV1::new(61_003, 61_004).unwrap();
        let client =
            CompilerExecutionClientProfileV1::new(uid, gid, service, policy.clone()).unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            uid,
            gid,
            service,
            CompilerExecutionIssuerMeasurementV1::new([0x31; 32], 1024).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x32; 32], 2048).unwrap(),
            &policy,
        )
        .unwrap();
        let anchor = CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy,
            CompilerExecutionIssuerMeasurementV1::new([0x33; 32], 4096).unwrap(),
        )
        .unwrap();
        Self {
            client,
            supervisor,
            anchor,
        }
    }

    fn write(&self, directory: &Path) {
        for ((name, _), bytes) in FILES.into_iter().zip([
            self.client.canonical_bytes().as_slice(),
            self.supervisor.canonical_bytes().as_slice(),
            self.anchor.canonical_bytes().as_slice(),
        ]) {
            let path = directory.join(name);
            if path.exists() {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o444)).unwrap();
        }
    }
}

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let tree = Self(path);
        fs::set_permissions(&tree.0, fs::Permissions::from_mode(0o700)).unwrap();
        let mut directory = tree.0.clone();
        for component in DIRECTORIES {
            directory.push(component);
            fs::create_dir(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Configuration::new(0x51).write(&directory);
        tree
    }
    fn directory(&self) -> PathBuf {
        self.0.join("etc/fe2o3/compiler-execution")
    }
    fn admit(&self) -> Result<ProductionCompilerExecutionDeploymentV1, String> {
        ProductionCompilerExecutionDeploymentV1::open_tree(
            &self.0,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        )
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exact_original_owners_survive_and_are_send_sync() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ProductionCompilerExecutionDeploymentV1>();
    let tree = Tree::new();
    let admitted = tree.admit().unwrap();
    assert_eq!(admitted.policy(), Configuration::new(0x51).client.policy());
    admitted.revalidate().unwrap();
    assert_eq!(admitted.tree.directories.len(), 4);
    assert_eq!(admitted.tree.files.len(), 3);
    // Revalidation does not create a new configuration owner or discard the original descriptors.
    let original = identity(&admitted.tree.files[0].file).unwrap();
    admitted.revalidate().unwrap();
    assert_eq!(identity(&admitted.tree.files[0].file).unwrap(), original);
}

#[test]
fn canonical_cross_configuration_substitution_is_rejected() {
    for member in 0..3 {
        let tree = Tree::new();
        let mut wrong = Configuration::new(0x51);
        let alternate = Configuration::new(0x61);
        match member {
            0 => wrong.client = alternate.client,
            1 => wrong.supervisor = alternate.supervisor,
            _ => wrong.anchor = alternate.anchor,
        }
        wrong.write(&tree.directory());
        assert!(tree.admit().is_err(), "substituted member {member}");
    }
}

#[test]
fn client_supervisor_uid_gid_and_anchor_credentials_must_match() {
    for field in 0..3 {
        let tree = Tree::new();
        let mut wrong = Configuration::new(0x51);
        wrong.client = CompilerExecutionClientProfileV1::new(
            if field == 0 { 62_001 } else { 61_001 },
            if field == 1 { 62_002 } else { 61_002 },
            CompilerExecutionExternalAnchorServiceIdentityV1::new(
                if field == 2 { 62_003 } else { 61_003 },
                61_004,
            )
            .unwrap(),
            wrong.client.policy().clone(),
        )
        .unwrap();
        wrong.write(&tree.directory());
        assert!(tree.admit().is_err(), "credential field {field}");
    }
}

#[test]
fn application_cannot_be_the_issuer_service_uid() {
    let tree = Tree::new();
    let cfg = Configuration::with_policy(
        Configuration::new(0x51).client.policy().clone(),
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    );
    cfg.write(&tree.directory());
    assert!(tree.admit().is_err());
}

#[test]
fn identical_byte_file_replacement_is_not_original_provenance() {
    for (name, _) in FILES {
        let tree = Tree::new();
        let admitted = tree.admit().unwrap();
        let path = tree.directory().join(name);
        let bytes = fs::read(&path).unwrap();
        fs::rename(&path, path.with_extension("old")).unwrap();
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        assert!(admitted.revalidate().is_err(), "replaced {name}");
    }
}

#[test]
fn replacement_of_each_directory_is_detected() {
    for depth in 0..3 {
        let tree = Tree::new();
        let admitted = tree.admit().unwrap();
        let mut path = tree.0.clone();
        for component in &DIRECTORIES[..=depth] {
            path.push(component);
        }
        let moved = path.with_extension("old");
        fs::rename(&path, &moved).unwrap();
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(admitted.revalidate().is_err(), "directory depth {depth}");
    }
}

#[test]
fn replacement_during_the_read_phase_fails_the_actual_post_check() {
    let tree = Tree::new();
    let admitted = tree.admit().unwrap();
    let directory = tree.directory();
    let result = admitted.tree.revalidate_with(
        &[
            admitted.profile.profile().canonical_bytes(),
            admitted.supervisor.deployment().canonical_bytes(),
            admitted.anchor.deployment().canonical_bytes(),
        ],
        || {
            fs::rename(&directory, directory.with_extension("old")).unwrap();
            fs::create_dir(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
            Configuration::new(0x51).write(&directory);
        },
    );
    assert!(result.is_err());
}

#[test]
fn unrelated_directory_changes_do_not_invalidate_original_records() {
    let tree = Tree::new();
    let admitted = tree.admit().unwrap();
    fs::create_dir(tree.directory().join("unrelated")).unwrap();
    fs::write(tree.directory().join("unrelated/file"), b"unrelated").unwrap();
    admitted.revalidate().unwrap();
}

#[test]
fn ambient_root_replacement_is_detected() {
    let tree = Tree::new();
    let admitted = tree.admit().unwrap();
    let moved = tree.0.with_extension("old");
    fs::rename(&tree.0, &moved).unwrap();
    fs::create_dir(&tree.0).unwrap();
    fs::set_permissions(&tree.0, fs::Permissions::from_mode(0o700)).unwrap();
    let result = admitted.revalidate();
    fs::remove_dir(&tree.0).unwrap();
    fs::rename(&moved, &tree.0).unwrap();
    assert!(result.is_err());
}

#[test]
fn in_place_mutation_and_mode_or_link_drift_fail_closed() {
    for mutation in 0..4 {
        let tree = Tree::new();
        let admitted = tree.admit().unwrap();
        let path = tree.directory().join(FILES[0].0);
        match mutation {
            0 => {
                let mut bytes = fs::read(&path).unwrap();
                bytes[0] ^= 1;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&path, bytes).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
            }
            1 => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            2 => fs::hard_link(&path, path.with_extension("alias")).unwrap(),
            _ => fs::set_permissions(tree.directory(), fs::Permissions::from_mode(0o775)).unwrap(),
        }
        assert!(admitted.revalidate().is_err(), "mutation {mutation}");
        assert!(tree.admit().is_err(), "fresh mutation {mutation}");
    }
}

#[test]
fn symlinks_and_wrong_lengths_are_rejected() {
    for mutation in 0..3 {
        let tree = Tree::new();
        let path = tree.directory().join(FILES[1].0);
        let original = path.with_extension("old");
        fs::rename(&path, &original).unwrap();
        match mutation {
            0 => std::os::unix::fs::symlink(&original, &path).unwrap(),
            1 => {
                fs::write(&path, []).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
            }
            _ => {
                let directory = tree.directory();
                let moved = directory.with_extension("old");
                fs::rename(&directory, &moved).unwrap();
                std::os::unix::fs::symlink(&moved, &directory).unwrap();
            }
        }
        assert!(tree.admit().is_err());
    }
}

#[test]
fn full_namespace_maps_exclude_root_looking_user_remaps() {
    assert!(full_identity_mapping(b"         0          0 4294967295\n"));
    for bytes in [
        b"0 1000 1".as_slice(),
        b"0 0 1",
        b"0 0 4294967296",
        b"0 0 4294967295\n1 1 1",
        b"",
        b"a 0 1",
    ] {
        assert!(!full_identity_mapping(bytes));
    }
}

#[test]
fn retained_namespace_substitution_is_detected() {
    let mut namespace = NamespaceCustody::open().unwrap();
    std::mem::swap(&mut namespace.mount, &mut namespace.user);
    assert!(namespace.revalidate().is_err());
}

#[test]
#[ignore = "writes test-key public configuration for an isolated root-owned qualification namespace"]
fn write_fixed_path_qualification_fixture() {
    let directory = std::env::var_os("FE2O3_PRODUCTION_DEPLOYMENT_FIXTURE_OUT").unwrap();
    Configuration::new(0x51).write(Path::new(&directory));
}

#[test]
#[ignore = "requires explicitly provisioned fixed paths inside a private real-root namespace"]
fn fixed_production_paths() {
    if std::env::var("FE2O3_PRODUCTION_DEPLOYMENT_OVERMAP").as_deref() == Ok("overmap") {
        let control =
            PathBuf::from(std::env::var_os("FE2O3_PRODUCTION_DEPLOYMENT_CONTROL").unwrap());
        // SAFETY: gettid has no pointer arguments or side effects.
        let tid = unsafe { libc::syscall(libc::SYS_gettid) };
        fs::write(
            control.join("overmap-request"),
            format!("{} {tid}\n", std::process::id()),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !control.join("overmapped").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "private map-overmount helper timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    let result = ProductionCompilerExecutionDeploymentV1::open();
    match std::env::var("FE2O3_PRODUCTION_DEPLOYMENT_EXPECT")
        .unwrap()
        .as_str()
    {
        "accept" => {
            let owner = result.unwrap();
            assert_eq!(owner.policy(), Configuration::new(0x51).client.policy());
            owner.revalidate().unwrap();
        }
        "reject" => assert!(result.is_err()),
        "reject-map" => assert!(result.unwrap_err().contains("remapped user namespaces")),
        _ => panic!("explicit qualification expectation required"),
    }
}
