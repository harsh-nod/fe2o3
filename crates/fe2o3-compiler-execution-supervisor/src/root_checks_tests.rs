use super::*;
use rustix::fs::{Mode, XattrFlags};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn credentials() -> IssuerServiceCredentialProfileV1 {
    IssuerServiceCredentialProfileV1::new(7, 8).unwrap()
}

fn snapshot() -> RootSnapshot {
    RootSnapshot {
        device: 11,
        inode: 12,
        mode: libc::S_IFDIR | 0o700,
        uid: 7,
        gid: 8,
        links: 2,
    }
}

fn invalid(reason: &'static str) -> Result<(), RootCheckError> {
    Err(RootCheckError::Invalid(reason))
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-root-checks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        let fixture = Self(path);
        fixture.mode(0o700);
        let root = fixture.open();
        for &name in &FORBIDDEN_XATTRS[1..] {
            match rustix::fs::fremovexattr(&root, name) {
                Ok(()) | Err(Errno::NODATA | Errno::OPNOTSUPP) => {}
                Err(error) => panic!("cannot clear fixture xattr {name:?}: {error}"),
            }
        }
        fixture
    }

    fn open(&self) -> File {
        File::open(&self.0).unwrap()
    }

    fn mode(&self, mode: u32) {
        fs::set_permissions(&self.0, fs::Permissions::from_mode(mode)).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn owner(root: &File) -> Option<IssuerServiceCredentialProfileV1> {
    let stat = rustix::fs::fstat(root).unwrap();
    IssuerServiceCredentialProfileV1::new(stat.st_uid, stat.st_gid).ok()
}

#[test]
fn snapshot_covers_each_retained_field_without_owned_storage() {
    assert!(!std::mem::needs_drop::<RootSnapshot>());
    assert!(!std::mem::needs_drop::<RootCheckError>());
    let original = snapshot();
    for field in 0..6 {
        let mut changed = original;
        match field {
            0 => changed.device ^= 1,
            1 => changed.inode ^= 1,
            2 => changed.mode ^= 1,
            3 => changed.uid ^= 1,
            4 => changed.gid ^= 1,
            _ => changed.links ^= 1,
        }
        assert_ne!(changed, original);
    }
}

#[test]
fn descriptor_and_metadata_faults_keep_the_legacy_precedence() {
    let mut value = RootSnapshot {
        mode: libc::S_IFREG | 0o777,
        uid: 9,
        gid: 10,
        links: 0,
        ..snapshot()
    };
    assert_eq!(
        validate_snapshot(FdFlags::empty(), OFlags::PATH, value, credentials()),
        invalid("descriptor is inheritable")
    );
    assert_eq!(
        validate_snapshot(FdFlags::CLOEXEC, OFlags::PATH, value, credentials()),
        invalid("descriptor is not read-only directory custody")
    );
    assert_eq!(
        validate_snapshot(FdFlags::CLOEXEC, OFlags::RDONLY, value, credentials()),
        invalid("object is not a directory")
    );
    value.mode = libc::S_IFDIR | 0o777;
    assert_eq!(
        validate_snapshot(FdFlags::CLOEXEC, OFlags::RDONLY, value, credentials()),
        invalid("owner does not match the service UID and GID")
    );
    value.uid = credentials().uid();
    value.gid = credentials().gid();
    assert_eq!(
        validate_snapshot(FdFlags::CLOEXEC, OFlags::RDONLY, value, credentials()),
        invalid("mode is not exactly 0700")
    );
    value.mode = snapshot().mode;
    assert_eq!(
        validate_snapshot(FdFlags::CLOEXEC, OFlags::RDONLY, value, credentials()),
        invalid("directory is unlinked")
    );
    value.links = 1;
    assert_eq!(
        validate_snapshot(FdFlags::CLOEXEC, OFlags::RDONLY, value, credentials()),
        Ok(())
    );
}

#[test]
fn access_mode_is_masked_but_path_custody_is_always_rejected() {
    for flags in [OFlags::WRONLY, OFlags::RDWR, OFlags::PATH] {
        assert_eq!(
            validate_snapshot(FdFlags::CLOEXEC, flags, snapshot(), credentials()),
            invalid("descriptor is not read-only directory custody")
        );
    }
    for flags in [
        OFlags::RDONLY,
        OFlags::RDONLY | OFlags::NONBLOCK,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW,
    ] {
        assert_eq!(
            validate_snapshot(FdFlags::CLOEXEC, flags, snapshot(), credentials()),
            Ok(())
        );
    }
}

#[test]
fn both_owner_axes_all_mode_bits_and_zero_links_are_checked_exactly() {
    for (uid, gid) in [(9, 8), (7, 9), (0, 8), (7, 0)] {
        assert_eq!(
            validate_snapshot(
                FdFlags::CLOEXEC,
                OFlags::RDONLY,
                RootSnapshot {
                    uid,
                    gid,
                    ..snapshot()
                },
                credentials()
            ),
            invalid("owner does not match the service UID and GID")
        );
    }
    for bit in 0..12 {
        let value = RootSnapshot {
            mode: snapshot().mode ^ (1 << bit),
            ..snapshot()
        };
        assert_eq!(
            validate_snapshot(FdFlags::CLOEXEC, OFlags::RDONLY, value, credentials()),
            invalid("mode is not exactly 0700")
        );
    }
    for links in [1, 2, u64::MAX] {
        assert_eq!(
            validate_snapshot(
                FdFlags::CLOEXEC,
                OFlags::RDONLY,
                RootSnapshot {
                    links,
                    ..snapshot()
                },
                credentials()
            ),
            Ok(())
        );
    }
}

#[test]
fn xattr_probe_names_order_and_all_results_are_frozen() {
    assert_eq!(
        FORBIDDEN_XATTRS,
        [
            c"security.capability",
            c"system.posix_acl_access",
            c"system.posix_acl_default",
        ]
    );
    for errno in [Errno::NODATA, Errno::OPNOTSUPP] {
        assert_eq!(classify_xattr(Err(errno)), Ok(()));
    }
    for result in [Ok(0), Ok(1), Ok(2), Err(Errno::RANGE)] {
        assert_eq!(
            classify_xattr(result),
            invalid("directory has a forbidden capability or POSIX ACL")
        );
    }
    for errno in [
        Errno::ACCESS,
        Errno::PERM,
        Errno::IO,
        Errno::INTR,
        Errno::BADF,
    ] {
        assert_eq!(
            classify_xattr(Err(errno)),
            Err(RootCheckError::Io {
                operation: "inspect protected issuer root extended attributes",
                errno,
            })
        );
    }
}

#[test]
fn real_directory_snapshot_and_nonblocking_status_preserve_metadata() {
    let fixture = Fixture::new();
    let root = fixture.open();
    let Some(credentials) = owner(&root) else {
        return;
    };
    let stat = rustix::fs::fstat(&root).unwrap();
    let expected = RootSnapshot {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
    };
    assert_eq!(inspect(&root, credentials), Ok(expected));
    let flags = rustix::fs::fcntl_getfl(&root).unwrap();
    rustix::fs::fcntl_setfl(&root, flags | OFlags::NONBLOCK).unwrap();
    assert_eq!(inspect(&root, credentials), Ok(expected));
    let other_uid = if credentials.uid() == 1 { 2 } else { 1 };
    let other_gid = if credentials.gid() == 1 { 2 } else { 1 };
    for wrong in [
        IssuerServiceCredentialProfileV1::new(other_uid, credentials.gid()).unwrap(),
        IssuerServiceCredentialProfileV1::new(credentials.uid(), other_gid).unwrap(),
    ] {
        assert_eq!(
            inspect(&root, wrong),
            Err(RootCheckError::Invalid(
                "owner does not match the service UID and GID"
            ))
        );
    }
}

#[test]
fn real_hostile_descriptors_fail_before_owner_and_mode() {
    let fixture = Fixture::new();
    let path = fixture.0.join("ordinary");
    fs::write(&path, b"root predicate fixture").unwrap();
    for file in [
        OpenOptions::new().write(true).open(&path).unwrap(),
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap(),
    ] {
        assert_eq!(
            inspect(&file, credentials()),
            Err(RootCheckError::Invalid(
                "descriptor is not read-only directory custody"
            ))
        );
    }
    let file = File::open(&path).unwrap();
    assert_eq!(
        inspect(&file, credentials()),
        Err(RootCheckError::Invalid("object is not a directory"))
    );
    let path_only = rustix::fs::open(&fixture.0, OFlags::PATH | OFlags::CLOEXEC, Mode::empty())
        .map(File::from)
        .unwrap();
    assert_eq!(
        inspect(&path_only, credentials()),
        Err(RootCheckError::Invalid(
            "descriptor is not read-only directory custody"
        ))
    );
    rustix::io::fcntl_setfd(&path_only, FdFlags::empty()).unwrap();
    assert_eq!(
        inspect(&path_only, credentials()),
        Err(RootCheckError::Invalid("descriptor is inheritable"))
    );
    rustix::io::fcntl_setfd(&path_only, FdFlags::CLOEXEC).unwrap();
}

#[test]
fn real_mode_changes_and_unlinked_directory_keep_predicate_order() {
    let fixture = Fixture::new();
    let root = fixture.open();
    let Some(credentials) = owner(&root) else {
        return;
    };
    fixture.mode(0o750);
    fs::remove_dir(&fixture.0).unwrap();
    assert_eq!(rustix::fs::fstat(&root).unwrap().st_nlink, 0);
    assert_eq!(
        inspect(&root, credentials),
        Err(RootCheckError::Invalid("mode is not exactly 0700"))
    );
    rustix::fs::fchmod(&root, Mode::from_raw_mode(0o700)).unwrap();
    assert_eq!(
        inspect(&root, credentials),
        Err(RootCheckError::Invalid("directory is unlinked"))
    );
}

#[test]
fn real_one_byte_xattr_probe_distinguishes_absent_empty_small_and_large_values() {
    let fixture = Fixture::new();
    let root = fixture.open();
    let name = c"user.fe2o3-root-checks";
    assert_eq!(require_absent_xattr(&root, name), Ok(()));
    for bytes in [b"".as_slice(), b"a".as_slice(), b"ab".as_slice()] {
        match rustix::fs::fsetxattr(&root, name, bytes, XattrFlags::empty()) {
            Ok(()) => {}
            Err(Errno::OPNOTSUPP) => return,
            Err(error) => panic!("cannot install fixture xattr: {error}"),
        }
        assert_eq!(
            require_absent_xattr(&root, name),
            invalid("directory has a forbidden capability or POSIX ACL")
        );
    }
    // Unrelated xattrs remain allowed by the legacy root contract.
    if let Some(credentials) = owner(&root) {
        assert!(inspect(&root, credentials).is_ok());
    }
    rustix::fs::fremovexattr(&root, name).unwrap();
    assert_eq!(require_absent_xattr(&root, name), Ok(()));
}

#[test]
fn real_access_and_default_acls_reject_even_when_mode_stays_0700() {
    let fixture = Fixture::new();
    let root = fixture.open();
    let stat = rustix::fs::fstat(&root).unwrap();
    let named_uid = if stat.st_uid == 1 { 2 } else { 1 };
    // Linux POSIX ACL xattr v2: owner rwx, named user/group/mask/other no access.
    let mut acl = [0_u8; 44];
    acl[..4].copy_from_slice(&2_u32.to_le_bytes());
    for (bytes, (tag, permissions, id)) in acl[4..].chunks_exact_mut(8).zip([
        (1_u16, 7_u16, u32::MAX),
        (2, 0, named_uid),
        (4, 0, u32::MAX),
        (16, 0, u32::MAX),
        (32, 0, u32::MAX),
    ]) {
        bytes[..2].copy_from_slice(&tag.to_le_bytes());
        bytes[2..4].copy_from_slice(&permissions.to_le_bytes());
        bytes[4..].copy_from_slice(&id.to_le_bytes());
    }
    for name in [c"system.posix_acl_access", c"system.posix_acl_default"] {
        match rustix::fs::fsetxattr(&root, name, &acl, XattrFlags::empty()) {
            Ok(()) => {}
            Err(Errno::OPNOTSUPP) => return,
            Err(error) => panic!("cannot install fixture ACL: {error}"),
        }
        assert_eq!(rustix::fs::fstat(&root).unwrap().st_mode & 0o7777, 0o700);
        assert_eq!(
            require_absent_xattr(&root, name),
            invalid("directory has a forbidden capability or POSIX ACL")
        );
        if let Some(credentials) = owner(&root) {
            assert_eq!(
                inspect(&root, credentials),
                Err(RootCheckError::Invalid(
                    "directory has a forbidden capability or POSIX ACL"
                ))
            );
        }
        rustix::fs::fremovexattr(&root, name).unwrap();
    }
}
