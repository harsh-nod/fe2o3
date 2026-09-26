use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V3 as PROFILE_PATH,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3 as PROFILE_STORAGE,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3 as PROFILE_WORK,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
    CompilerExecutionClientProfileErrorV3 as ProfileError,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV3 as Policy,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use ed25519_dalek::SigningKey;
use std::{error::Error as _, os::unix::fs::MetadataExt};

type Cap = super::CompilerExecutionClientProfileCapabilityV3;
const PROFILE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V3\0";

fn profile_failure<T>(result: super::Result<T>) -> ProfileError {
    match crate::native_capability::tests::failure(result) {
        super::Error::ProfileV3(error) => error,
        error => panic!("expected V3 profile error: {error:?}"),
    }
}
fn retained_profile(seed: u8, budget: &mut super::Budget<'_>) -> super::Profile {
    let (policy, charge) = Policy::new(
        u64::from(seed),
        Measurement::new([seed; 32], 123).unwrap(),
        Measurement::new([seed + 1; 32], 456).unwrap(),
        SigningKey::from_bytes(&[seed; 32]).verifying_key().to_bytes(),
        SigningKey::from_bytes(&[seed + 1; 32]).verifying_key().to_bytes(),
        budget,
    ).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let (profile, delta) = super::Profile::new(
        1000, 1000, Service::new(1001, 1001).unwrap(), policy, budget,
    ).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    profile
}
fn profile(seed: u8) -> super::Profile {
    let mut work = Work::new(POLICY_WORK + PROFILE_WORK);
    let mut budget = super::Budget::new(&mut work, 100_000);
    retained_profile(seed, &mut budget)
}

// Both nominal families exercise the same trusted-tree and snapshot invariants.
include!("native_profile_tests.rs");

#[test]
fn v3_public_ownership_chain_keeps_original_ledger_inode_offsets_and_charges() {
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let profile = retained_profile(7, &mut budget);
    let bytes = *profile.canonical_bytes();
    let policy_identity = profile.policy().identity();
    let profile_charge = profile.retained_storage();
    let (cap, delta) = Cap::create(profile, &mut budget).unwrap();
    assert_eq!(budget.storage(), 19 + profile_charge);
    assert_eq!(delta.additional_storage() + profile_charge, cap.retained_storage());
    budget.reserve_storage(delta.additional_storage()).unwrap();
    let retained = cap.retained_storage();
    let actual_policy: &Policy = cap.profile().policy();
    assert_eq!(actual_policy.identity(), policy_identity);
    assert_eq!(cap.profile().canonical_bytes(), &bytes);
    cap.revalidate(&mut budget).unwrap();
    let (file, full) = cap.try_clone_for_transfer(&mut budget).unwrap();
    assert_eq!(full.additional_storage(), Cap::FILE_STORAGE);
    budget.reserve_storage(full.additional_storage()).unwrap();
    let inode = file.metadata().unwrap().ino();
    let device = file.metadata().unwrap().dev();
    assert_eq!(inode, cap.0.image.as_file().metadata().unwrap().ino());
    assert_eq!(device, cap.0.image.as_file().metadata().unwrap().dev());
    rustix::fs::seek(&file, rustix::fs::SeekFrom::Start(37)).unwrap();
    cap.revalidate(&mut budget).unwrap();
    assert_eq!(rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(), 37);
    drop(cap);
    budget.release_storage(retained).unwrap();
    let (readmitted, delta) = Cap::from_file(file, &mut budget).unwrap();
    assert_eq!(budget.storage(), 19 + Cap::FILE_STORAGE);
    assert_eq!(delta.additional_storage(), retained - Cap::FILE_STORAGE);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(readmitted.profile().canonical_bytes(), &bytes);
    assert_eq!(readmitted.0.image.as_file().metadata().unwrap().ino(), inode);
    assert_eq!(readmitted.0.image.as_file().metadata().unwrap().dev(), device);
    assert_eq!(rustix::fs::seek(readmitted.0.image.as_file(), rustix::fs::SeekFrom::Current(0)).unwrap(), 37);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.work(), POLICY_WORK + 2 * PROFILE_WORK + 5 * Cap::IO_WORK);
    drop(readmitted);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn v3_sealed_decode_uses_original_nested_budget_and_typed_errors() {
    let bytes = *profile(7).canonical_bytes();
    let floor = 19 + Cap::FILE_STORAGE;
    let peak = floor + Cap::IO_STORAGE + PROFILE_STORAGE;
    for case in 0..3 {
        let work = Cap::IO_WORK + PROFILE_WORK - usize::from(case == 0);
        let (result, used, live, observed_peak) = run(floor, work, peak - usize::from(case == 1), |b| {
            let ledger = b.work_ledger_identity_v1();
            let result = Cap::from_file(sealed(&bytes), b);
            assert!(b.work_ledger_identity_v1() == ledger);
            result
        });
        assert_eq!(live, floor);
        match case {
            0 => {
                assert!(matches!(profile_failure(result), ProfileError::Resource(Resource::Work(_))));
                assert_eq!(used, Cap::IO_WORK + ENTRY_WORK);
            }
            1 => {
                assert!(matches!(profile_failure(result), ProfileError::Resource(Resource::Storage(_))));
                assert_eq!(used, Cap::IO_WORK + PROFILE_WORK);
            }
            _ => {
                let (cap, delta) = result.unwrap();
                assert_eq!(delta.additional_storage(), cap.retained_storage() - Cap::FILE_STORAGE);
                assert_eq!(used, Cap::IO_WORK + PROFILE_WORK);
                assert_eq!(observed_peak, peak);
            }
        }
    }
}

#[test]
fn v3_capability_entry_floors_and_io_quotas_precede_sealed_admission() {
    use crate::native_capability::tests::transfer_boundaries;
    let p = profile(7);
    let prepaid = p.retained_storage();
    transfer_boundaries(prepaid, Cap::IO_WORK, Cap::IO_STORAGE, |b| {
        Cap::create(profile(7), b).map(|(cap, delta)| {
            assert_eq!(prepaid + delta.additional_storage(), cap.retained_storage());
        })
    });
    let (cap, _) = run(prepaid, 1_000_000, 1_000_000, |b| Cap::create(p, b)).0.unwrap();
    transfer_boundaries(cap.retained_storage(), Cap::IO_WORK, Cap::IO_STORAGE, |b| cap.revalidate(b));
    transfer_boundaries(cap.retained_storage(), Cap::IO_WORK, Cap::IO_STORAGE, |b| {
        cap.try_clone_for_transfer(b).map(|(_, charge)| {
            assert_eq!(charge.additional_storage(), Cap::FILE_STORAGE);
        })
    });
    let result = run(Cap::FILE_STORAGE - 1, 1_000_000, 1_000_000, |b| {
        Cap::from_file(sealed(cap.profile().canonical_bytes()), b)
    });
    assert!(matches!(failure(result.0), Error::Resource(Resource::Accounting)));
    assert_eq!((result.1, result.2), (ENTRY_WORK, Cap::FILE_STORAGE - 1));

    type V2 = crate::CompilerExecutionClientProfileCapabilityV2;
    assert_eq!(Cap::FILE_STORAGE, V2::FILE_STORAGE);
    assert_eq!(Cap::IO_WORK, V2::IO_WORK);
    assert_eq!(Cap::IO_STORAGE, V2::IO_STORAGE);
    assert_eq!(Cap::PRODUCTION_WORK, V2::PRODUCTION_WORK);
    assert_eq!(Cap::PRODUCTION_STORAGE, V2::PRODUCTION_STORAGE);
    assert_eq!(std::mem::size_of::<Cap>(), std::mem::size_of::<V2>());
}

#[test]
fn v3_never_falls_back_to_v2_or_v1_and_sealed_crossfamily_is_typed() {
    let p = profile(7);
    let tree = Tree::new(p.canonical_bytes());
    let v2 = crate::native_capability::tests::profile(7);
    let v1 = legacy_profile(7);
    tree.write("client-profile-v2", v2.canonical_bytes());
    tree.write("client-profile-v1", v1.canonical_bytes());
    fs::remove_file(tree.file(PROFILE_NAME)).unwrap();
    assert!(matches!(failure(tree.admit(1_000_000, 1_000_000).0), Error::Io { errno: libc::ENOENT, .. }));
    for older in [v2.canonical_bytes(), v1.canonical_bytes(), &[0; BYTES]] {
        tree.write(PROFILE_NAME, older);
        assert!(matches!(profile_failure(tree.admit(1_000_000, 1_000_000).0), ProfileError::Framing(_)));
        let error = failure(run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
            Cap::from_file(sealed(older), b)
        }).0);
        assert!(matches!(&error, Error::ProfileV3(ProfileError::Framing(_))));
        assert!(error.source().unwrap().is::<ProfileError>());
    }
    let reverse = run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
        crate::CompilerExecutionClientProfileCapabilityV2::from_file(sealed(p.canonical_bytes()), b)
    }).0;
    assert!(matches!(failure(reverse), Error::Profile(_)));
}

#[test]
fn v3_tree_and_sealed_inputs_reject_v2_policy_under_correct_v3_outer_hash() {
    let mut bytes = *profile(7).canonical_bytes();
    bytes[32..248].copy_from_slice(crate::native_capability::tests::policy(7).canonical_bytes());
    let mut hash = Sha256::new();
    hash.update((PROFILE_DOMAIN.len() as u64).to_le_bytes());
    hash.update(PROFILE_DOMAIN);
    hash.update(248_u64.to_le_bytes());
    hash.update(&bytes[..248]);
    bytes[248..].copy_from_slice(&hash.finalize());
    let tree = Tree::new(&bytes);
    assert!(matches!(profile_failure(tree.admit(1_000_000, 1_000_000).0), ProfileError::Policy(_)));
    let sealed = run(Cap::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
        Cap::from_file(sealed(&bytes), b)
    }).0;
    assert!(matches!(profile_failure(sealed), ProfileError::Policy(_)));
}

#[test]
fn opened_root_pins_tree_even_when_its_path_is_replaced() {
    let p = profile(7);
    let tree = Tree::new(p.canonical_bytes());
    let root = rustix::fs::open(&tree.root, tree::DIRECTORY_FLAGS, Mode::empty()).map(File::from).unwrap();
    let moved = tree.root.with_extension("pinned");
    fs::rename(&tree.root, &moved).unwrap();
    fs::create_dir(&tree.root).unwrap();
    let (result, _, live, _) = run(19, 1_000_000, 1_000_000, |b| {
        b.with_prepaid_scope(0, ENTRY_WORK, Cap::PRODUCTION_WORK, Cap::PRODUCTION_STORAGE, |b| {
            from_trusted_tree(root, rustix::process::getuid().as_raw(), rustix::process::getgid().as_raw(), b)
        })
    });
    fs::remove_dir_all(&moved).unwrap();
    assert_eq!(live, 19);
    assert_eq!(result.unwrap().record.canonical_bytes(), p.canonical_bytes());
    assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
}
