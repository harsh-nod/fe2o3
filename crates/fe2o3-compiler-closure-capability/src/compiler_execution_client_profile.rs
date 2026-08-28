use std::fs::File;
use std::os::fd::AsFd;
use std::os::unix::fs::{FileExt, MetadataExt};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1, COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
    CompilerExecutionClientProfileV1,
};
use rustix::fs::{FileType, Mode, OFlags};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution client-profile capability",
    memfd_name: "fe2o3-compiler-execution-client-profile-v1",
};
const LENGTH: ImageLength = ImageLength::Exact(COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1);
const PERMISSION_AND_SPECIAL_BITS: u32 = 0o7777;
const TRUSTED_FILE_MODE: u32 = 0o444;
const TRUSTED_DIRECTORY_FORBIDDEN_MODE: u32 = 0o022;
const TRUSTED_DIRECTORY_REQUIRED_MODE: u32 = 0o100;
const PRODUCTION_DIRECTORY_COMPONENTS: [&str; 3] = ["etc", "fe2o3", "compiler-execution"];
const PRODUCTION_PROFILE_NAME: &str = "client-profile-v1";

/// Immutable descriptor capability carrying one exact compiler-execution client profile.
///
/// The profile is public trust configuration, not authority. This value grants no compiler,
/// signing, publication, loading, launch, or execution operation.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionClientProfileCapabilityV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionClientProfileCapabilityV1>();
/// ```
pub struct CompilerExecutionClientProfileCapabilityV1 {
    profile: CompilerExecutionClientProfileV1,
    image: SealedCapabilityImage,
}

impl CompilerExecutionClientProfileCapabilityV1 {
    /// Admits and seals the sole root-owned production client profile.
    ///
    /// Every fixed path component is opened descriptor-relatively with `O_NOFOLLOW`, must be
    /// root-owned, non-group/world-writable, traversable by its owner, and free of POSIX ACLs or
    /// file capabilities. The final object must be a root-owned, single-link, exact mode-0444
    /// regular file with stable metadata and exactly the canonical profile length.
    pub fn from_production_profile() -> Result<Self, String> {
        let root = rustix::fs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|error| {
            format!(
                "cannot open production compiler-execution profile root for {}: {error}",
                COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1
            )
        })?;
        Self::from_trusted_tree(
            root,
            &PRODUCTION_DIRECTORY_COMPONENTS,
            PRODUCTION_PROFILE_NAME,
            0,
            0,
        )
    }

    /// Creates and seals the exact canonical client-profile image.
    pub fn create(profile: CompilerExecutionClientProfileV1) -> Result<Self, String> {
        let image = SealedCapabilityImage::create(profile.canonical_bytes(), ROLE, LENGTH)?;
        let admitted = Self { profile, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an already transferred immutable client-profile image.
    pub fn from_file(image: File) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        let profile = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { profile, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Borrows the exact public client profile.
    pub const fn profile(&self) -> &CompilerExecutionClientProfileV1 {
        &self.profile
    }

    /// Revalidates descriptor identity, mode, seals, length, bytes, and canonical profile equality.
    pub fn revalidate(&self) -> Result<(), String> {
        let current = decode(&self.image.read_exact_bytes()?)?;
        if current != self.profile {
            return Err("compiler-execution client-profile capability bytes changed".to_owned());
        }
        Ok(())
    }

    /// Clones the same sealed descriptor for one authenticated broker transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }

    fn from_trusted_tree(
        mut directory: File,
        directory_components: &[&str],
        profile_name: &str,
        expected_uid: u32,
        expected_gid: u32,
    ) -> Result<Self, String> {
        validate_trusted_directory(&directory, expected_uid, expected_gid, "profile root")?;
        for component in directory_components {
            let next = rustix::fs::openat(
                &directory,
                *component,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map(File::from)
            .map_err(|error| {
                format!("cannot open trusted client-profile directory {component:?}: {error}")
            })?;
            validate_trusted_directory(&next, expected_uid, expected_gid, component)?;
            directory = next;
        }
        let profile = rustix::fs::openat(
            &directory,
            profile_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|error| format!("cannot open trusted client profile {profile_name:?}: {error}"))?;
        let before =
            validate_trusted_profile_file(&profile, expected_uid, expected_gid, profile_name)?;
        let mut bytes = [0_u8; COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1];
        let mut offset = 0;
        while offset < bytes.len() {
            let read = profile
                .read_at(&mut bytes[offset..], offset as u64)
                .map_err(|error| format!("cannot read trusted client profile: {error}"))?;
            if read == 0 {
                return Err("trusted client profile ended before its exact length".to_owned());
            }
            offset += read;
        }
        let after =
            validate_trusted_profile_file(&profile, expected_uid, expected_gid, profile_name)?;
        if before != after {
            return Err("trusted client profile changed while it was read".to_owned());
        }
        let decoded = decode(&bytes)?;
        Self::create(decoded)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrustedFileSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

fn validate_trusted_directory(
    directory: &File,
    expected_uid: u32,
    expected_gid: u32,
    label: &str,
) -> Result<(), String> {
    let descriptor_flags = rustix::io::fcntl_getfd(directory)
        .map_err(|error| format!("cannot inspect trusted directory {label:?}: {error}"))?;
    let status = rustix::fs::fcntl_getfl(directory)
        .map_err(|error| format!("cannot inspect trusted directory {label:?}: {error}"))?;
    let stat = rustix::fs::fstat(directory)
        .map_err(|error| format!("cannot inspect trusted directory {label:?}: {error}"))?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != expected_uid
        || stat.st_gid != expected_gid
        || stat.st_nlink == 0
        || stat.st_mode & TRUSTED_DIRECTORY_FORBIDDEN_MODE != 0
        || stat.st_mode & TRUSTED_DIRECTORY_REQUIRED_MODE == 0
    {
        return Err(format!(
            "trusted client-profile directory {label:?} has invalid descriptor, type, owner, mode, or link state"
        ));
    }
    require_absent_xattrs(directory, "trusted client-profile directory")
}

fn validate_trusted_profile_file(
    profile: &File,
    expected_uid: u32,
    expected_gid: u32,
    label: &str,
) -> Result<TrustedFileSnapshot, String> {
    let descriptor_flags = rustix::io::fcntl_getfd(profile)
        .map_err(|error| format!("cannot inspect trusted client profile {label:?}: {error}"))?;
    let status = rustix::fs::fcntl_getfl(profile)
        .map_err(|error| format!("cannot inspect trusted client profile {label:?}: {error}"))?;
    let metadata = profile
        .metadata()
        .map_err(|error| format!("cannot inspect trusted client profile {label:?}: {error}"))?;
    let snapshot = TrustedFileSnapshot {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    };
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(snapshot.mode) != FileType::RegularFile
        || snapshot.uid != expected_uid
        || snapshot.gid != expected_gid
        || snapshot.links != 1
        || snapshot.mode & PERMISSION_AND_SPECIAL_BITS != TRUSTED_FILE_MODE
        || snapshot.length != COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1 as u64
    {
        return Err(format!(
            "trusted client profile {label:?} has invalid descriptor, type, owner, mode, link count, or length"
        ));
    }
    require_absent_xattrs(profile, "trusted client-profile file")?;
    Ok(snapshot)
}

fn require_absent_xattrs(object: &impl AsFd, label: &str) -> Result<(), String> {
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = 0_u8;
        match rustix::fs::fgetxattr(object, attribute, std::slice::from_mut(&mut byte)) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(format!(
                    "{label} has forbidden capability or POSIX ACL attribute {attribute:?}"
                ));
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect {label} extended attribute {attribute:?}: {error}"
                ));
            }
        }
    }
    Ok(())
}

fn decode(bytes: &[u8]) -> Result<CompilerExecutionClientProfileV1, String> {
    CompilerExecutionClientProfileV1::decode(bytes).map_err(|error| {
        format!("compiler-execution client-profile capability is not canonical: {error}")
    })
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    };

    use super::*;
    use crate::sealed_image::{REQUIRED_SEALS, SealedCapabilityImage};

    fn profile(seed: u8) -> CompilerExecutionClientProfileV1 {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
        )
        .unwrap();
        CompilerExecutionClientProfileV1::new(1_234, 5_678, policy).unwrap()
    }

    fn trusted_tree() -> (PathBuf, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "fe2o3-compiler-execution-client-profile-tree-{}-{nonce}",
            std::process::id()
        ));
        let directory = root.join("etc/fe2o3/compiler-execution");
        fs::create_dir_all(&directory).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        for path in [root.join("etc"), root.join("etc/fe2o3"), directory.clone()] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let file = directory.join(PRODUCTION_PROFILE_NAME);
        fs::write(&file, profile(7).canonical_bytes()).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(TRUSTED_FILE_MODE)).unwrap();
        (root, file)
    }

    fn admit_tree(root: &Path) -> Result<CompilerExecutionClientProfileCapabilityV1, String> {
        let root = rustix::fs::open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .unwrap();
        CompilerExecutionClientProfileCapabilityV1::from_trusted_tree(
            root,
            &PRODUCTION_DIRECTORY_COMPONENTS,
            PRODUCTION_PROFILE_NAME,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        )
    }

    #[test]
    fn exact_profile_is_sealed_transferred_and_revalidated() {
        let expected = profile(7);
        let capability =
            CompilerExecutionClientProfileCapabilityV1::create(expected.clone()).unwrap();
        assert_eq!(capability.profile(), &expected);
        assert_eq!(
            rustix::fs::fcntl_get_seals(capability.image.as_file()).unwrap(),
            REQUIRED_SEALS
        );
        assert!(capability.image.as_file().set_len(0).is_err());
        let transferred = capability.try_clone_for_transfer().unwrap();
        let recovered = CompilerExecutionClientProfileCapabilityV1::from_file(transferred).unwrap();
        assert_eq!(recovered.profile(), &expected);
        recovered.revalidate().unwrap();
    }

    #[test]
    fn mutable_and_malformed_sealed_images_fail_closed() {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-execution-client-profile-hostile-{}",
            std::process::id()
        ));
        fs::write(&path, profile(7).canonical_bytes()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionClientProfileCapabilityV1::from_file(File::open(&path).unwrap())
                .is_err()
        );
        fs::remove_file(path).unwrap();

        let mut malformed = *profile(7).canonical_bytes();
        malformed[0] ^= 1;
        let malformed_image = SealedCapabilityImage::create(&malformed, ROLE, LENGTH).unwrap();
        assert!(
            CompilerExecutionClientProfileCapabilityV1::from_file(
                malformed_image.try_clone_for_transfer().unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn independently_sealed_profile_substitution_remains_distinct() {
        let first = CompilerExecutionClientProfileCapabilityV1::create(profile(7)).unwrap();
        let second = CompilerExecutionClientProfileCapabilityV1::create(profile(8)).unwrap();
        assert_ne!(first.profile(), second.profile());
        assert_ne!(first.profile().identity(), second.profile().identity());
    }

    #[test]
    fn trusted_tree_admission_rejects_mutable_aliased_and_symlinked_inputs() {
        let (root, file) = trusted_tree();
        let admitted = admit_tree(&root).unwrap();
        assert_eq!(admitted.profile(), &profile(7));

        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(admit_tree(&root).is_err());
        fs::set_permissions(&file, fs::Permissions::from_mode(TRUSTED_FILE_MODE)).unwrap();

        let alias = file.with_extension("alias");
        fs::hard_link(&file, &alias).unwrap();
        assert!(admit_tree(&root).is_err());
        fs::remove_file(alias).unwrap();

        let original = file.with_extension("original");
        fs::rename(&file, &original).unwrap();
        std::os::unix::fs::symlink(&original, &file).unwrap();
        assert!(admit_tree(&root).is_err());
        fs::remove_file(&file).unwrap();
        fs::rename(original, &file).unwrap();

        let directory = root.join("etc/fe2o3/compiler-execution");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o775)).unwrap();
        assert!(admit_tree(&root).is_err());
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();

        fs::remove_dir_all(root).unwrap();
    }
}
