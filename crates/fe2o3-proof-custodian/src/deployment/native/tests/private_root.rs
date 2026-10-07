use super::*;
use fe2o3_compiler_lineage::{
    MAX_NATIVE_CONDITIONAL_STORAGE_V1, NativeConditionalPolicyRootInputV1 as Root,
    NativeConditionalPolicyRosterInputV1 as Input, encode_native_conditional_policy_roster_v1,
};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;

mod manager;

struct Fixture {
    config: Vec<u8>,
    policy: Vec<u8>,
    image: Vec<u8>,
    compiler: [Vec<u8>; 3],
}
impl Fixture {
    fn new() -> Self {
        require_exact_root_identity_v1().unwrap();
        assert_eq!(
            std::env::var("FE2O3_NATIVE_PROOF_PRIVATE_FIXTURE").as_deref(),
            Ok("1")
        );
        let original: u64 = std::env::var("FE2O3_NATIVE_PROOF_HOST_MNTNS_INODE")
            .unwrap()
            .parse()
            .unwrap();
        assert_ne!(original, 0);
        assert_ne!(
            original,
            fs::metadata("/proc/thread-self/ns/mnt").unwrap().ino()
        );
        for directory in ["/etc", "/usr/libexec"] {
            assert_eq!(
                rustix::fs::fstatfs(&File::open(directory).unwrap())
                    .unwrap()
                    .f_type as i64,
                libc::TMPFS_MAGIC as i64
            );
        }
        for directory in ["/etc/fe2o3", "/usr/libexec/fe2o3"] {
            assert_eq!(
                fs::symlink_metadata(directory).unwrap_err().kind(),
                io::ErrorKind::NotFound
            );
            fs::create_dir(directory).unwrap();
            fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::create_dir("/etc/fe2o3/proof-custodian").unwrap();
        fs::set_permissions(
            "/etc/fe2o3/proof-custodian",
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        // This roster is inert fixture content, never a kernel or proof admission.
        let signers = [[1; 32]];
        let roots = [Root {
            semantic_root: 0,
            kernel_binding: [2; 32],
            effect_signers: &signers,
            effect_toolchain: [[3; 32]; 5],
            formula_verifying_key: [4; 32],
            formula_toolchain: [[5; 32]; 5],
            formula_boundary: 2,
        }];
        let roster = encode_native_conditional_policy_roster_v1(
            Input {
                source_packet: b"private deployment fixture, not execution authority",
                roots: &roots,
            },
            MAX_NATIVE_CONDITIONAL_STORAGE_V1,
            |_| Ok::<_, std::convert::Infallible>(()),
        )
        .unwrap();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(roster.len()).unwrap();
        let (policy, charge) =
            fe2o3_verifier::encode_native_conditional_root_policy_file_v1(&roster, &mut budget)
                .unwrap();
        budget.reserve_storage(charge).unwrap();
        let image =
            fs::read(std::env::var_os("FE2O3_NATIVE_PROOF_CONTROLLER_IMAGE").unwrap()).unwrap();
        assert!(image.len() <= MAX_CONTROLLER as usize);
        fe2o3_runtime_protocol::sealed_static_application_identity_v1(&image).unwrap();
        let (compiler, compiler_identity) = manager::compiler_records();
        let (config, _) = Config::new(
            ProofControllerCredentialProfileV1::new(61000, 61000).unwrap(),
            Sha256::digest(&image).into(),
            image.len() as u64,
            analyzer(),
            [6; 32],
            compiler_identity,
            (Sha256::digest(&policy).into(), policy.len() as u64),
            &mut budget,
        )
        .unwrap();
        Self {
            config: config.canonical_bytes().to_vec(),
            policy,
            image,
            compiler,
        }
    }
    fn entries(&self) -> [(&'static str, &[u8], u32); 3] {
        [
            (CONFIG_PATH, &self.config, 0o444),
            (POLICY_PATH, &self.policy, 0o444),
            (CONTROLLER_PATH, &self.image, 0o555),
        ]
    }
    fn write(path: &str, bytes: &[u8], mode: u32) {
        if Path::new(path).exists() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    fn install(&self) {
        for (path, bytes, mode) in self.entries() {
            Self::write(path, bytes, mode);
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all("/etc/fe2o3");
        let _ = fs::remove_dir_all("/usr/libexec/fe2o3");
    }
}

#[test]
#[ignore = "real root and private tmpfs mounts only; use qualify-native-proof-deployment.sh"]
fn fixed_native_proof_deployment_positive_and_replacement_controls() {
    let fixture = Fixture::new();
    fixture.install();
    {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(73).unwrap();
        let (owner, charge) = Deployment::open(&mut budget).unwrap();
        assert_eq!(
            budget.storage(),
            73,
            "open returns the full unreserved owner charge"
        );
        assert_eq!(
            charge.additional_storage(),
            owner.retained_storage().unwrap()
        );
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let floor = budget.storage();
        owner.revalidate(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        let (policy, retained) = owner.clone_policy_for_handoff(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(retained).unwrap();
        assert_eq!(policy.metadata().unwrap().uid(), 0);
        assert_eq!(policy.metadata().unwrap().mode() & 0o777, 0o400);
        drop(policy);
        budget.release_storage(retained).unwrap();
        let mut other_work = Work::new(WORK);
        let mut other = Budget::new(&mut other_work, STORAGE);
        other.reserve_storage(floor).unwrap();
        assert!(
            owner.revalidate(&mut other).is_err(),
            "different original account must refuse"
        );
        owner.revalidate(&mut budget).unwrap();
        budget.release_storage(charge.additional_storage()).unwrap();
        assert!(
            owner.revalidate(&mut budget).is_err(),
            "unpaid retained owner must refuse"
        );
    }
    for (path, bytes, mode) in fixture.entries() {
        fixture.install();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (owner, charge) = Deployment::open(&mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let old = format!("{path}.original");
        fs::rename(path, &old).unwrap();
        Fixture::write(path, bytes, mode);
        assert!(
            owner.revalidate(&mut budget).is_err(),
            "same-byte replacement: {path}"
        );
        drop(owner);
        fs::remove_file(old).unwrap();
    }
    for (path, _, _) in fixture.entries() {
        fixture.install();
        fs::set_permissions(path, fs::Permissions::from_mode(0o666)).unwrap();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        assert!(
            Deployment::open(&mut budget).is_err(),
            "writable installed leaf: {path}"
        );
    }
    fixture.install();
    let mut changed = fixture.policy.clone();
    changed[40] ^= 1;
    Fixture::write(POLICY_PATH, &changed, 0o444);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    assert!(
        Deployment::open(&mut budget).is_err(),
        "independent policy measurement mismatch"
    );
}
