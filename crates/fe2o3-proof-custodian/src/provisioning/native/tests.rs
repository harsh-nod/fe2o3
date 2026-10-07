use super::*;
use fe2o3_compiler_lineage::{
    NativeConditionalPolicyRootInputV1 as Root, NativeConditionalPolicyRosterInputV1 as Input,
    encode_native_conditional_policy_roster_v1,
};
use fe2o3_kernel_analysis::{
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectWorkerPolicyV1,
    PhysicalMachineRuntimeClosureIdentityV1, PhysicalMachineToolchainIdentityV1,
    PhysicalMachineWorkerExecutableIdentityV1,
};
use std::{
    cell::Cell,
    ffi::OsString,
    fs::File,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
};

fn account() -> Owned {
    Owned::new(Work::new(100_000_000_000), 256 * 1024 * 1024)
}

fn candidate(b: &mut Budget<'_>) -> Candidate {
    // Inert administrator-selected policy/configuration data, never live custody.
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
            source_packet: b"independently approved source",
            roots: &roots,
        },
        POLICY_MAX,
        |_| Ok::<_, std::convert::Infallible>(()),
    )
    .unwrap();
    b.reserve_storage(roster.len()).unwrap();
    let (policy, charge) =
        fe2o3_verifier::encode_native_conditional_root_policy_file_v1(&roster, b).unwrap();
    b.reserve_storage(charge).unwrap();
    let semantic = (Sha256::digest(&policy).into(), policy.len() as u64);
    let analyzer = PhysicalMachineEffectWorkerPolicyV1::new(
        PhysicalMachineWorkerExecutableIdentityV1::calculate(b"worker"),
        PhysicalMachineRuntimeClosureIdentityV1::from_parts([4; 32], 5),
        PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([6; 32]),
        PhysicalMachineToolchainIdentityV1::from_sha256_bytes([7; 32]),
    )
    .unwrap();
    let (application, charge) = Config::new(
        fe2o3_protected_service_profile::ProofControllerCredentialProfileV1::new(61002, 61003)
            .unwrap(),
        [3; 32],
        9,
        analyzer,
        [8; 32],
        [9; 32],
        semantic,
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (manager, charge) =
        ManagerConfig::new(([10; 32], 11), [9; 32], application.identity(), semantic, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    Candidate {
        application,
        manager,
        policy,
    }
}

#[test]
fn native_cli_is_distinct_and_requires_both_independent_pins() {
    use super::super::{Command, parse};
    let args = |parts: &[&str]| parts.iter().map(OsString::from).collect::<Vec<_>>();
    let pin = "ab".repeat(32);
    assert!(matches!(
        parse(&args(&[
            "inspect-native-fixed-resources",
            "policy",
            &pin,
            "out"
        ]))
        .unwrap(),
        Command::InspectNative(_, [0xab, ..], _)
    ));
    assert!(matches!(
        parse(&args(&["install-native", "candidate", &pin])).unwrap(),
        Command::InstallNative(_, [0xab, ..])
    ));
    for parts in [
        vec!["inspect-native-fixed-resources", "out"],
        vec!["inspect-native-fixed-resources", "policy", "out"],
        vec!["install-native", "candidate"],
        vec!["install-native", "candidate", "AB"],
    ] {
        assert!(parse(&args(&parts)).is_err());
    }
}

#[test]
fn native_candidate_roundtrip_rejects_every_byte_mutation_and_legacy() {
    account().with_budget(|b| {
        let value = candidate(b);
        let bytes = value.encode(b).unwrap();
        let recovered = Candidate::decode(&bytes, b).unwrap();
        assert_eq!(recovered.encode(b).unwrap(), bytes);
        let ledger = b.work_ledger_identity_v1();
        let storage = b.storage_account_identity_v1();
        for offset in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            assert!(Candidate::decode(&changed, b).is_err(), "byte {offset}");
        }
        for n in [0, 8, 24, bytes.len() - 1] {
            assert!(Candidate::decode(&bytes[..n], b).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(Candidate::decode(&trailing, b).is_err());
        assert!(Candidate::decode(&vec![0; super::super::CANDIDATE_BYTES], b).is_err());
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.storage_account_identity_v1(), storage);
    });
}

#[test]
fn native_candidate_requires_exact_policy_compiler_and_manager_links() {
    account().with_budget(|b| {
        let mut value = candidate(b);
        let original = value.manager.canonical_bytes().to_owned();
        for (compiler, proof, policy) in [
            (
                [12; 32],
                value.application.identity(),
                value.application.semantic_policy(),
            ),
            ([9; 32], [12; 32], value.application.semantic_policy()),
            (
                [9; 32],
                value.application.identity(),
                ([12; 32], value.policy.len() as u64),
            ),
        ] {
            let (wrong, charge) =
                ManagerConfig::new(([10; 32], 11), compiler, proof, policy, b).unwrap();
            b.reserve_storage(charge.additional_storage()).unwrap();
            value.manager = wrong;
            assert!(value.validate(b).is_err());
        }
        let (manager, charge) = ManagerConfig::decode(&original, b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        value.manager = manager;
        value.policy[0] ^= 1;
        assert!(value.validate(b).is_err());
    });
}

#[test]
fn native_pinned_input_is_finite_exact_and_does_not_accept_aliases() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input");
    std::fs::write(&path, b"approved").unwrap();
    let pin = Sha256::digest(b"approved").into();
    account().with_budget(|b| {
        assert_eq!(read_pinned(&path, pin, 8, b).unwrap(), b"approved");
        assert!(read_pinned(&path, [0; 32], 8, b).is_err());
        assert!(read_pinned(&path, [8; 32], 8, b).is_err());
        assert!(read_pinned(&path, pin, 7, b).is_err());
        let alias = dir.path().join("alias");
        symlink(&path, &alias).unwrap();
        assert!(read_pinned(&alias, pin, 8, b).is_err());
        std::fs::remove_file(&alias).unwrap();
        std::fs::hard_link(&path, &alias).unwrap();
        assert!(read_pinned(&path, pin, 8, b).is_err());
    });
    let mut denied = Owned::new(Work::new(0), 1024);
    denied.with_budget(|b| {
        assert!(read_pinned(Path::new("/never-open-native-policy"), pin, 8, b).is_err())
    });
}

#[test]
fn native_candidate_resource_denials_precede_decode_or_output_creation() {
    let bytes = account().with_budget(|b| candidate(b).encode(b).unwrap());
    for (work, storage) in [(0, 1_000_000), (1_000_000, bytes.len() - 1)] {
        let mut denied = Owned::new(Work::new(work), storage);
        denied.with_budget(|b| {
            if bytes.len() <= storage {
                b.reserve_storage(bytes.len()).unwrap();
            }
            assert!(Candidate::decode(&bytes, b).is_err());
        });
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("must-not-exist");
    Owned::new(Work::new(0), 1024).with_budget(|b| {
        assert!(write_candidate(&path, &bytes, b).is_err());
    });
    assert!(!path.exists());
}

fn parent() -> (tempfile::TempDir, File, u32, u32) {
    let dir = tempfile::tempdir().unwrap();
    let file = File::open(dir.path()).unwrap();
    let m = file.metadata().unwrap();
    (dir, file, m.uid(), m.gid())
}

#[test]
fn native_publication_is_atomic_exact_idempotent_and_never_replaces() {
    let (dir, parent, uid, gid) = parent();
    account().with_budget(|b| {
        let value = candidate(b);
        publication::publish(&parent, &value, uid, gid, b, |_| Ok(())).unwrap();
        let installed = dir.path().join("proof-custodian");
        let inode = installed.metadata().unwrap().ino();
        for (name, bytes) in RECORD_NAMES.into_iter().zip(value.records()) {
            assert_eq!(std::fs::read(installed.join(name)).unwrap(), bytes);
            assert_eq!(
                installed.join(name).metadata().unwrap().mode() & 0o7777,
                0o444
            );
        }
        publication::publish(&parent, &value, uid, gid, b, |_| Ok(())).unwrap();
        assert_eq!(installed.metadata().unwrap().ino(), inode);
        let path = installed.join(RECORD_NAMES[0]);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::write(&path, b"different approval").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
        assert!(publication::publish(&parent, &value, uid, gid, b, |_| Ok(())).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"different approval");
    });
}

#[test]
fn native_publication_rejects_legacy_and_cleans_only_its_unpublished_stage() {
    let (dir, parent, uid, gid) = parent();
    account().with_budget(|b| {
        let value = candidate(b);
        let calls = Cell::new(0);
        assert!(
            publication::publish(&parent, &value, uid, gid, b, |_| {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    Err(io::Error::other("injected prepublication refusal"))
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let legacy = dir.path().join("proof-custodian");
        std::fs::create_dir(&legacy).unwrap();
        std::fs::set_permissions(&legacy, std::fs::Permissions::from_mode(0o755)).unwrap();
        for name in super::super::RECORD_NAMES {
            std::fs::write(legacy.join(name), b"legacy").unwrap();
        }
        assert!(publication::publish(&parent, &value, uid, gid, b, |_| Ok(())).is_err());
        for name in super::super::RECORD_NAMES {
            assert_eq!(std::fs::read(legacy.join(name)).unwrap(), b"legacy");
        }
        assert_eq!(std::fs::read_dir(&legacy).unwrap().count(), 2);
    });
}

#[test]
fn native_publication_refuses_extra_records_and_unowned_staging() {
    let (dir, parent, uid, gid) = parent();
    account().with_budget(|b| {
        let value = candidate(b);
        let orphan = dir.path().join(".proof-custodian-native-unowned");
        std::fs::create_dir(&orphan).unwrap();
        std::fs::write(orphan.join("administrator-evidence"), b"retain").unwrap();
        assert!(publication::publish(&parent, &value, uid, gid, b, |_| Ok(())).is_err());
        assert_eq!(
            std::fs::read(orphan.join("administrator-evidence")).unwrap(),
            b"retain"
        );
        assert!(!dir.path().join("proof-custodian").exists());
        std::fs::remove_dir_all(&orphan).unwrap();

        publication::publish(&parent, &value, uid, gid, b, |_| Ok(())).unwrap();
        let unexpected = dir.path().join("proof-custodian/extra-record");
        std::fs::write(&unexpected, b"retain extra").unwrap();
        assert!(publication::publish(&parent, &value, uid, gid, b, |_| Ok(())).is_err());
        assert_eq!(std::fs::read(unexpected).unwrap(), b"retain extra");
        for (name, bytes) in RECORD_NAMES.into_iter().zip(value.records()) {
            assert_eq!(
                std::fs::read(dir.path().join("proof-custodian").join(name)).unwrap(),
                bytes
            );
        }
    });
}
