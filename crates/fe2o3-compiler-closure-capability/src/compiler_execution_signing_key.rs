use std::fmt;
use std::fs::File;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::FileExt;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
};
use rustix::fs::{Mode, OFlags};
use zeroize::Zeroize;

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution signing-key capability",
    memfd_name: "fe2o3-compiler-execution-signing-key-v1",
};
const KEY_BYTES: usize = 32;
const LENGTH: ImageLength = ImageLength::Exact(KEY_BYTES);

/// Fixed protected-issuer descriptor carrying the sealed signing key.
pub const COMPILER_EXECUTION_SIGNING_KEY_ISSUER_FD_V1: RawFd = 7;

/// Move-only service-owned signing-key custody bound to one caller policy.
///
/// The key seed and descriptor are intentionally inaccessible. This capability
/// can only revalidate itself or clone the same sealed read-only file
/// description for the protected issuer boundary.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionSigningKeyCapabilityV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionSigningKeyCapabilityV1>();
/// ```
pub struct CompilerExecutionSigningKeyCapabilityV1 {
    key: SigningKey,
    image: SealedCapabilityImage,
}

impl fmt::Debug for CompilerExecutionSigningKeyCapabilityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionSigningKeyCapabilityV1")
            .field("authority", &"signing-key-custody-only")
            .field("verifying_key", &self.key.verifying_key().to_bytes())
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionSigningKeyCapabilityV1 {
    /// Creates a sealed read-only key image and zeroizes the supplied seed.
    ///
    /// The seed is zeroized on every success and failure path. Callers remain
    /// responsible for eliminating any copies made before this call.
    pub fn create_and_zeroize(
        seed: &mut [u8; KEY_BYTES],
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, String> {
        let result = Self::create_inner(seed, policy);
        seed.zeroize();
        result
    }

    fn create_inner(
        seed: &[u8; KEY_BYTES],
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, String> {
        let key = SigningKey::from_bytes(seed);
        require_policy_key(&key, policy)?;
        let writable = SealedCapabilityImage::create(seed, ROLE, LENGTH)?;
        let image = reopen_read_only(writable)?;
        let admitted = Self { key, image };
        admitted.revalidate(policy)?;
        Ok(admitted)
    }

    /// Admits an already transferred immutable read-only key image.
    pub fn from_file(
        image: File,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        validate_secret_image(&image)?;
        let key = read_key(&image)?;
        require_policy_key(&key, policy)?;
        let admitted = Self { key, image };
        admitted.revalidate(policy)?;
        Ok(admitted)
    }

    /// Admits and privately retains a key inherited at one fixed descriptor.
    pub fn from_inherited_at(
        child_fd: RawFd,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_inherited_at(child_fd, ROLE, LENGTH)?;
        validate_secret_image(&image)?;
        let key = read_key(&image)?;
        require_policy_key(&key, policy)?;
        let admitted = Self { key, image };
        admitted.revalidate(policy)?;
        Ok(admitted)
    }

    /// Reissues a root-provisioned template into current supervisor-owned key custody.
    ///
    /// The source must remain an anonymous root-owned read-only canonical key image. The caller
    /// must already have the exact dedicated UID/GID named by `deployment`, and `deployment` must
    /// name `policy`; the returned memfd is created under those credentials. Transient seed bytes
    /// are zeroized on every return path.
    pub fn reissue_root_template_for_current_service(
        image: File,
        deployment: &CompilerExecutionSupervisorDeploymentV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, String> {
        Self::reissue_template_for_current_service(image, deployment, policy, 0, 0)
    }

    fn reissue_template_for_current_service(
        image: File,
        deployment: &CompilerExecutionSupervisorDeploymentV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        template_uid: u32,
        template_gid: u32,
    ) -> Result<Self, String> {
        if rustix::process::geteuid().as_raw() != deployment.service_uid()
            || rustix::process::getegid().as_raw() != deployment.service_gid()
        {
            return Err(
                "compiler-execution key reissue process does not have deployment credentials"
                    .into(),
            );
        }
        if !deployment.matches_policy(policy) {
            return Err(
                "compiler-execution key reissue deployment names another issuer policy".into(),
            );
        }
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        validate_template_image(&image, template_uid, template_gid)?;
        let key = read_key(&image)?;
        require_policy_key(&key, policy)?;
        let mut seed = key.to_bytes();
        drop(key);
        drop(image);
        Self::create_and_zeroize(&mut seed, policy)
    }

    /// Revalidates object identity, ownership, mode, seals, bytes, access, and policy binding.
    pub fn revalidate(&self, policy: &CompilerExecutionIssuerPolicyV1) -> Result<(), String> {
        self.revalidate_image()?;
        require_policy_key(&self.key, policy)
    }

    fn revalidate_image(&self) -> Result<(), String> {
        self.image.revalidate()?;
        validate_secret_image(&self.image)?;
        let current = read_key(&self.image)?;
        if current.as_bytes() != self.key.as_bytes() {
            return Err("compiler-execution signing-key bytes changed".to_owned());
        }
        Ok(())
    }

    /// Returns the public Ed25519 key without exposing seed or descriptor custody.
    pub fn verifying_key(&self) -> [u8; KEY_BYTES] {
        self.key.verifying_key().to_bytes()
    }

    /// Clones the same sealed read-only descriptor for one protected transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate_image()?;
        self.image.try_clone_for_transfer()
    }
}

fn reopen_read_only(writable: SealedCapabilityImage) -> Result<SealedCapabilityImage, String> {
    let transferred = writable.try_clone_for_transfer()?;
    let path = PathBuf::from(format!("/proc/self/fd/{}", transferred.as_raw_fd()));
    let read_only = rustix::fs::open(&path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
        .map(File::from)
        .map_err(|error| format!("cannot bind read-only {}: {error}", ROLE.name))?;
    drop(transferred);
    drop(writable);
    SealedCapabilityImage::from_file(read_only, ROLE, LENGTH)
}

fn validate_secret_image(image: &SealedCapabilityImage) -> Result<(), String> {
    let descriptor = image.try_clone_for_transfer()?;
    let metadata = descriptor
        .metadata()
        .map_err(|error| format!("cannot inspect {} ownership: {error}", ROLE.name))?;
    let status = rustix::fs::fcntl_getfl(&descriptor)
        .map_err(|error| format!("cannot inspect {} access: {error}", ROLE.name))?;
    if metadata.nlink() != 0
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.gid() != rustix::process::getegid().as_raw()
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
    {
        return Err(format!(
            "{} is not an anonymous service-owned read-only image",
            ROLE.name
        ));
    }
    Ok(())
}

fn validate_template_image(
    image: &SealedCapabilityImage,
    expected_uid: u32,
    expected_gid: u32,
) -> Result<(), String> {
    let descriptor = image.try_clone_for_transfer()?;
    let metadata = descriptor
        .metadata()
        .map_err(|error| format!("cannot inspect {} template ownership: {error}", ROLE.name))?;
    let status = rustix::fs::fcntl_getfl(&descriptor)
        .map_err(|error| format!("cannot inspect {} template access: {error}", ROLE.name))?;
    if metadata.nlink() != 0
        || metadata.uid() != expected_uid
        || metadata.gid() != expected_gid
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
    {
        return Err(format!(
            "{} is not an anonymous trusted-owner read-only template",
            ROLE.name
        ));
    }
    Ok(())
}

fn read_key(image: &SealedCapabilityImage) -> Result<SigningKey, String> {
    let image = image.try_clone_for_transfer()?;
    let mut seed = [0_u8; KEY_BYTES];
    if let Err(error) = image.read_exact_at(&mut seed, 0) {
        seed.zeroize();
        return Err(format!("cannot read {}: {error}", ROLE.name));
    }
    let key = SigningKey::from_bytes(&seed);
    seed.zeroize();
    Ok(key)
}

fn require_policy_key(
    key: &SigningKey,
    policy: &CompilerExecutionIssuerPolicyV1,
) -> Result<(), String> {
    if key.verifying_key().as_bytes() != policy.verifying_key() {
        return Err(
            "compiler-execution signing key does not match the caller-pinned policy".to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
    };

    use super::*;

    fn policy(seed: u8) -> CompilerExecutionIssuerPolicyV1 {
        let key = SigningKey::from_bytes(&[seed; KEY_BYTES]);
        CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 2).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([3; 32], 3).unwrap(),
            key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[seed.wrapping_add(1); KEY_BYTES])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn deployment(
        policy: &CompilerExecutionIssuerPolicyV1,
        service_uid: u32,
        service_gid: u32,
    ) -> CompilerExecutionSupervisorDeploymentV1 {
        let anchor_uid = if service_uid == 1 { 2 } else { 1 };
        let anchor_gid = if service_gid == 1 { 2 } else { 1 };
        CompilerExecutionSupervisorDeploymentV1::new(
            service_uid,
            service_gid,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(anchor_uid, anchor_gid).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([4; 32], 4).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([5; 32], 5).unwrap(),
            policy,
        )
        .unwrap()
    }

    fn raw_key_image(bytes: &[u8], mode: u32, seals: rustix::fs::SealFlags) -> File {
        let image = rustix::fs::memfd_create(
            "fe2o3-hostile-compiler-execution-signing-key-v1",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map(File::from)
        .unwrap();
        let mut writer = image.try_clone().unwrap();
        writer.write_all(bytes).unwrap();
        drop(writer);
        rustix::fs::fchmod(&image, rustix::fs::Mode::from_raw_mode(mode)).unwrap();
        let initial = seals - rustix::fs::SealFlags::SEAL;
        if !initial.is_empty() {
            rustix::fs::fcntl_add_seals(&image, initial).unwrap();
        }
        if seals.contains(rustix::fs::SealFlags::SEAL) {
            rustix::fs::fcntl_add_seals(&image, rustix::fs::SealFlags::SEAL).unwrap();
        }
        image
    }

    #[test]
    fn seed_is_zeroized_and_exact_key_survives_read_only_transfer() {
        let expected = policy(7);
        let mut seed = [7; KEY_BYTES];
        let capability =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &expected)
                .unwrap();
        assert_eq!(seed, [0; KEY_BYTES]);
        assert_eq!(capability.verifying_key(), *expected.verifying_key());
        capability.revalidate(&expected).unwrap();

        let transferred = capability.try_clone_for_transfer().unwrap();
        assert_eq!(
            rustix::fs::fcntl_getfl(&transferred).unwrap() & OFlags::ACCMODE,
            OFlags::RDONLY
        );
        let recovered =
            CompilerExecutionSigningKeyCapabilityV1::from_file(transferred, &expected).unwrap();
        recovered.revalidate(&expected).unwrap();
        assert!(
            recovered
                .try_clone_for_transfer()
                .unwrap()
                .set_len(0)
                .is_err()
        );
    }

    #[test]
    fn trusted_template_reissues_only_for_exact_current_deployment() {
        if rustix::process::geteuid().is_root() || rustix::process::getegid().is_root() {
            return;
        }
        let expected = policy(7);
        let uid = rustix::process::geteuid().as_raw();
        let gid = rustix::process::getegid().as_raw();
        let exact_deployment = deployment(&expected, uid, gid);
        let mut seed = [7; KEY_BYTES];
        let template =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &expected)
                .unwrap();

        let wrong_service_uid = if uid == 1 { 2 } else { 1 };
        let wrong_deployment = deployment(&expected, wrong_service_uid, gid);
        assert!(
            CompilerExecutionSigningKeyCapabilityV1::reissue_template_for_current_service(
                template.try_clone_for_transfer().unwrap(),
                &wrong_deployment,
                &expected,
                uid,
                gid,
            )
            .is_err()
        );
        assert!(
            CompilerExecutionSigningKeyCapabilityV1::reissue_template_for_current_service(
                template.try_clone_for_transfer().unwrap(),
                &exact_deployment,
                &policy(8),
                uid,
                gid,
            )
            .is_err()
        );

        let reissued =
            CompilerExecutionSigningKeyCapabilityV1::reissue_template_for_current_service(
                template.try_clone_for_transfer().unwrap(),
                &exact_deployment,
                &expected,
                uid,
                gid,
            )
            .unwrap();
        assert_eq!(reissued.verifying_key(), *expected.verifying_key());
        reissued.revalidate(&expected).unwrap();
    }

    #[test]
    fn wrong_policy_rejects_and_still_zeroizes_seed() {
        let mut seed = [7; KEY_BYTES];
        assert!(
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &policy(8))
                .is_err()
        );
        assert_eq!(seed, [0; KEY_BYTES]);
    }

    #[test]
    fn writable_and_ordinary_images_reject() {
        let expected = policy(7);
        let writable = SealedCapabilityImage::create(&[7; KEY_BYTES], ROLE, LENGTH).unwrap();
        let writable = writable.try_clone_for_transfer().unwrap();
        assert_eq!(
            rustix::fs::fcntl_getfl(&writable).unwrap() & OFlags::ACCMODE,
            OFlags::RDWR
        );
        assert!(CompilerExecutionSigningKeyCapabilityV1::from_file(writable, &expected).is_err());

        let path =
            std::env::temp_dir().join(format!("fe2o3-signing-key-ordinary-{}", std::process::id()));
        fs::write(&path, [7; KEY_BYTES]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionSigningKeyCapabilityV1::from_file(
                File::open(&path).unwrap(),
                &expected,
            )
            .is_err()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_length_mode_seals_and_descriptor_flags_reject() {
        let expected = policy(7);

        let short = raw_key_image(
            &[7; KEY_BYTES - 1],
            0o400,
            crate::sealed_image::REQUIRED_SEALS,
        );
        assert!(CompilerExecutionSigningKeyCapabilityV1::from_file(short, &expected).is_err());

        let permissive = raw_key_image(&[7; KEY_BYTES], 0o600, crate::sealed_image::REQUIRED_SEALS);
        assert!(CompilerExecutionSigningKeyCapabilityV1::from_file(permissive, &expected).is_err());

        let incomplete = raw_key_image(
            &[7; KEY_BYTES],
            0o400,
            rustix::fs::SealFlags::WRITE
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::SHRINK,
        );
        assert!(CompilerExecutionSigningKeyCapabilityV1::from_file(incomplete, &expected).is_err());

        let mut seed = [7; KEY_BYTES];
        let capability =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &expected)
                .unwrap();
        let inheritable = capability.try_clone_for_transfer().unwrap();
        rustix::io::fcntl_setfd(&inheritable, rustix::io::FdFlags::empty()).unwrap();
        assert!(
            CompilerExecutionSigningKeyCapabilityV1::from_file(inheritable, &expected).is_err()
        );
    }

    #[test]
    fn debug_never_contains_seed_or_descriptor() {
        let expected = policy(7);
        let mut seed = [7; KEY_BYTES];
        let capability =
            CompilerExecutionSigningKeyCapabilityV1::create_and_zeroize(&mut seed, &expected)
                .unwrap();
        assert_eq!(
            format!("{capability:?}"),
            format!(
                "CompilerExecutionSigningKeyCapabilityV1 {{ authority: \"signing-key-custody-only\", verifying_key: {:?}, .. }}",
                expected.verifying_key()
            )
        );
    }
}
