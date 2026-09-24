use super::*;
use crate::native_capability::tests::{
    failure, legacy_policy, legacy_profile, profile, run, sealed,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V2,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2 as PROFILE_STORAGE,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2 as PROFILE_WORK,
    CompilerExecutionClientProfileErrorV2 as ProfileError,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Tree {
    root: PathBuf,
}
impl Tree {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "native-profile-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let mut directory = root.clone();
        for component in tree::COMPONENTS {
            directory.push(component);
            fs::create_dir(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let result = Self { root };
        result.write(PROFILE_NAME, bytes);
        result
    }
    fn file(&self, name: &str) -> PathBuf {
        self.root.join("etc/fe2o3/compiler-execution").join(name)
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        let path = self.file(name);
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o444)).unwrap();
    }
    fn admit(
        &self,
        work: usize,
        storage: usize,
    ) -> (Result<(Capability, Storage)>, usize, usize, usize) {
        run(13, work, storage, |budget| {
            budget.with_prepaid_scope(
                0,
                ENTRY_WORK,
                CompilerExecutionClientProfileCapabilityV2::PRODUCTION_WORK,
                CompilerExecutionClientProfileCapabilityV2::PRODUCTION_STORAGE,
                |budget| {
                    let root = rustix::fs::open(&self.root, tree::DIRECTORY_FLAGS, Mode::empty())
                        .map(File::from)
                        .map_err(|e| Error::io("open fixture root", e))?;
                    let uid = rustix::process::getuid().as_raw();
                    let gid = rustix::process::getgid().as_raw();
                    Ok((
                        from_trusted_tree(root, uid, gid, budget)?,
                        Storage(Capability::RETAINED),
                    ))
                },
            )
        })
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn native_profile_roundtrip_and_fixed_tree_match_the_strict_decoder() {
    let p = profile(7);
    let bytes = *p.canonical_bytes();
    let retained = p.retained_storage();
    let (result, work, floor, peak) = run(
        retained,
        Capability::IO_WORK,
        retained + Capability::IO_STORAGE,
        |b| Capability::create(p, b),
    );
    let (cap, growth) = result.unwrap();
    assert_eq!(work, Capability::IO_WORK);
    assert_eq!(floor, retained);
    assert_eq!(peak, retained + Capability::IO_STORAGE);
    assert_eq!(growth.additional_storage(), Capability::RETAINED - retained);
    let (transfer, charge) = run(Capability::RETAINED, Capability::IO_WORK, 1_000_000, |b| {
        cap.try_clone_for_transfer(b)
    })
    .0
    .unwrap();
    assert_eq!(charge.additional_storage(), Capability::FILE_STORAGE);
    drop(cap);
    let (recovered, delta) = run(Capability::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
        Capability::from_file(transfer, b)
    })
    .0
    .unwrap();
    assert_eq!(recovered.record.canonical_bytes(), &bytes);
    assert_eq!(
        delta.additional_storage(),
        Capability::RETAINED - Capability::FILE_STORAGE
    );
    let tree = Tree::new(&bytes);
    let (result, work, floor, peak) = tree.admit(1_000_000, 1_000_000);
    let (admitted, charge) = result.unwrap();
    assert_eq!(admitted.record.canonical_bytes(), &bytes);
    assert_eq!(charge.additional_storage(), Capability::RETAINED);
    assert_eq!(
        work,
        CompilerExecutionClientProfileCapabilityV2::PRODUCTION_WORK + PROFILE_WORK
    );
    assert_eq!(floor, 13);
    assert_eq!(
        peak,
        13 + CompilerExecutionClientProfileCapabilityV2::PRODUCTION_STORAGE + PROFILE_STORAGE
    );
    assert_eq!(
        COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V2,
        format!("/{}/{}", tree::COMPONENTS.join("/"), PROFILE_NAME)
    );
}

#[test]
fn trusted_root_resource_boundaries_include_native_decode_and_cleanup() {
    let tree = Tree::new(profile(7).canonical_bytes());
    let outer = CompilerExecutionClientProfileCapabilityV2::PRODUCTION_WORK;
    let scratch = CompilerExecutionClientProfileCapabilityV2::PRODUCTION_STORAGE;
    for (work, storage, kind, accepted) in [
        (outer - 1, 1_000_000, 0, ENTRY_WORK),
        (outer, 13 + scratch - 1, 1, outer),
        (outer + PROFILE_WORK - 1, 1_000_000, 2, outer + ENTRY_WORK),
        (
            outer + PROFILE_WORK,
            13 + scratch + PROFILE_STORAGE - 1,
            3,
            outer + PROFILE_WORK,
        ),
        (
            outer + PROFILE_WORK,
            13 + scratch + PROFILE_STORAGE,
            4,
            outer + PROFILE_WORK,
        ),
    ] {
        let (result, used, live, _) = tree.admit(work, storage);
        assert_eq!(live, 13);
        assert_eq!(used, accepted);
        match kind {
            0 => assert!(matches!(
                failure(result),
                Error::Resource(Resource::Work(_))
            )),
            1 => assert!(matches!(
                failure(result),
                Error::Resource(Resource::Storage(_))
            )),
            2 => assert!(matches!(
                failure(result),
                Error::Profile(ProfileError::Resource(Resource::Work(_)))
            )),
            3 => assert!(matches!(
                failure(result),
                Error::Profile(ProfileError::Resource(Resource::Storage(_)))
            )),
            _ => assert!(result.is_ok()),
        }
    }
}

#[test]
fn valid_legacy_profile_never_rescues_missing_or_malformed_native_file() {
    let tree = Tree::new(profile(7).canonical_bytes());
    tree.write("client-profile-v1", legacy_profile(7).canonical_bytes());
    fs::remove_file(tree.file(PROFILE_NAME)).unwrap();
    assert!(matches!(
        failure(tree.admit(1_000_000, 1_000_000).0),
        Error::Io {
            errno: libc::ENOENT,
            ..
        }
    ));
    for bytes in [legacy_profile(7).canonical_bytes(), &[0; BYTES]] {
        tree.write(PROFILE_NAME, bytes);
        assert!(matches!(
            failure(tree.admit(1_000_000, 1_000_000).0),
            Error::Profile(_)
        ));
    }
}

#[test]
fn native_profile_rejects_a_legacy_nested_policy_even_with_correct_outer_hash() {
    let mut bytes = *profile(7).canonical_bytes();
    bytes[32..248].copy_from_slice(legacy_policy(7).canonical_bytes());
    let domain = b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V2\0";
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_le_bytes());
    hash.update(domain);
    hash.update(248_u64.to_le_bytes());
    hash.update(&bytes[..248]);
    bytes[248..].copy_from_slice(&hash.finalize());
    let result = run(Capability::FILE_STORAGE, 1_000_000, 1_000_000, |b| {
        Capability::from_file(sealed(&bytes), b)
    })
    .0;
    assert!(matches!(
        failure(result),
        Error::Profile(ProfileError::Policy(_))
    ));
    let tree = Tree::new(&bytes);
    assert!(matches!(
        failure(tree.admit(1_000_000, 1_000_000).0),
        Error::Profile(ProfileError::Policy(_))
    ));
}

#[test]
fn every_directory_component_rejects_writable_modes_and_symlinks() {
    let tree = Tree::new(profile(7).canonical_bytes());
    for relative in ["", "etc", "etc/fe2o3", "etc/fe2o3/compiler-execution"] {
        let path = tree.root.join(relative);
        for mode in [0o775, 0o757, 0o600] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(tree.admit(1_000_000, 1_000_000).0.is_ok());
        if !relative.is_empty() {
            let moved = path.with_extension("original");
            fs::rename(&path, &moved).unwrap();
            symlink(&moved, &path).unwrap();
            assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
            fs::remove_file(&path).unwrap();
            fs::rename(&moved, &path).unwrap();
        }
    }
}

#[test]
fn final_file_rejects_modes_links_symlinks_lengths_and_fifo() {
    let bytes = *profile(7).canonical_bytes();
    let tree = Tree::new(&bytes);
    let path = tree.file(PROFILE_NAME);
    for mode in [0o400, 0o644, 0o1444] {
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    let alias = tree.file("alias");
    fs::hard_link(&path, &alias).unwrap();
    assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
    fs::remove_file(&alias).unwrap();
    fs::rename(&path, &alias).unwrap();
    symlink(&alias, &path).unwrap();
    assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
    fs::remove_file(&path).unwrap();
    fs::rename(&alias, &path).unwrap();
    for bytes in [&[][..], &bytes[..BYTES - 1], &[0; BYTES + 1][..]] {
        tree.write(PROFILE_NAME, bytes);
        assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
    }
    fs::remove_file(&path).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &path,
        rustix::fs::FileType::Fifo,
        Mode::from_raw_mode(0o444),
        0,
    )
    .unwrap();
    assert!(tree.admit(1_000_000, 1_000_000).0.is_err());
}

#[test]
fn profile_revalidation_rejects_identical_byte_inode_substitution() {
    let p = profile(7);
    let bytes = *p.canonical_bytes();
    let (mut cap, _) = run(p.retained_storage(), 1_000_000, 1_000_000, |b| {
        Capability::create(p, b)
    })
    .0
    .unwrap();
    cap.image.replace_file_for_test(sealed(&bytes));
    assert!(matches!(
        failure(
            run(Capability::RETAINED, 1_000_000, 1_000_000, |b| cap
                .revalidate(b))
            .0
        ),
        Error::Rejected("sealed image identity or length changed")
    ));
}

#[test]
fn post_read_metadata_and_snapshot_errors_precede_malformed_wire() {
    for mutate_mode in [false, true] {
        let tree = Tree::new(&[0; BYTES]);
        let file = File::open(tree.file(PROFILE_NAME)).unwrap();
        let result =
            read_profile(
                &file,
                rustix::process::getuid().as_raw(),
                rustix::process::getgid().as_raw(),
                |file| {
                    if mutate_mode {
                        rustix::fs::fchmod(file, Mode::RUSR).unwrap();
                    } else {
                        file.set_times(fs::FileTimes::new().set_modified(
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1),
                        ))
                        .unwrap();
                    }
                },
            );
        let expected = if mutate_mode {
            "invalid trusted profile descriptor, type, owner, mode, links, or length"
        } else {
            "trusted native profile changed while reading"
        };
        assert!(matches!(failure(result), Error::Rejected(reason) if reason == expected));
    }
}

#[test]
fn trusted_validators_reject_wrong_owners_and_descriptor_access_modes() {
    let tree = Tree::new(profile(7).canonical_bytes());
    let directory = File::open(&tree.root).unwrap();
    let path = tree.file(PROFILE_NAME);
    let file = File::open(&path).unwrap();
    let uid = rustix::process::getuid().as_raw();
    let gid = rustix::process::getgid().as_raw();
    for (u, g) in [(uid.wrapping_add(1), gid), (uid, gid.wrapping_add(1))] {
        assert!(tree::validate_directory(&directory, u, g).is_err());
        assert!(tree::validate_file(&file, u, g, BYTES).is_err());
    }
    let path_only = rustix::fs::open(
        &path,
        rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .unwrap();
    assert!(tree::validate_file(&path_only, uid, gid, BYTES).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let writable = fs::OpenOptions::new().write(true).open(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(tree::validate_file(&writable, uid, gid, BYTES).is_err());
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    assert!(tree::validate_file(&file, uid, gid, BYTES).is_err());
}
