use std::fmt;
use std::fs::File;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV1;
use rustix::fs::{Mode, OFlags};
use zeroize::{Zeroize, Zeroizing};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution external-anchor signing-key capability",
    memfd_name: "fe2o3-compiler-execution-external-anchor-signing-key-v1",
};
const KEY_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const DEPLOYMENT_IDENTITY_BYTES: usize = 32;
const WIRE_BYTES: usize = HEADER_BYTES + DEPLOYMENT_IDENTITY_BYTES + KEY_BYTES;
const MAGIC: [u8; 8] = *b"F2O3EAK1";
const VERSION_V1: u16 = 1;
const LENGTH: ImageLength = ImageLength::Exact(WIRE_BYTES);

/// Reserved descriptor carrying external-anchor signing-key custody into the anchor service.
pub const COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1: RawFd = 222;

/// Move-only anchor-owned signing-key custody bound to one exact deployment.
///
/// The canonical image has an anchor-specific role header and embeds the deployment identity,
/// preventing an issuer key image or a key provisioned for another deployment from being admitted.
/// Consuming the capability is the only operation that releases an in-memory signing key.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionExternalAnchorSigningKeyCapabilityV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionExternalAnchorSigningKeyCapabilityV1>();
/// ```
pub struct CompilerExecutionExternalAnchorSigningKeyCapabilityV1 {
    key: SigningKey,
    image: SealedCapabilityImage,
}

impl fmt::Debug for CompilerExecutionExternalAnchorSigningKeyCapabilityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionExternalAnchorSigningKeyCapabilityV1")
            .field("authority", &"external-anchor-signing-key-custody-only")
            .field("verifying_key", &self.key.verifying_key().to_bytes())
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionExternalAnchorSigningKeyCapabilityV1 {
    /// Creates a role- and deployment-bound sealed image and zeroizes the supplied seed.
    ///
    /// The seed and every temporary wire buffer are zeroized on success and failure. Callers remain
    /// responsible for eliminating copies made before this call.
    pub fn create_and_zeroize(
        seed: &mut [u8; KEY_BYTES],
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<Self, String> {
        let result = Self::create_inner(seed, deployment);
        seed.zeroize();
        result
    }

    fn create_inner(
        seed: &[u8; KEY_BYTES],
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<Self, String> {
        let key = SigningKey::from_bytes(seed);
        require_deployment_key(&key, deployment)?;
        let bytes = encode(seed, deployment);
        let writable = SealedCapabilityImage::create(bytes.as_slice(), ROLE, LENGTH)?;
        let image = reopen_read_only(writable)?;
        let admitted = Self { key, image };
        admitted.revalidate(deployment)?;
        Ok(admitted)
    }

    /// Admits an already transferred immutable read-only anchor-key image.
    pub fn from_file(
        image: File,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        validate_secret_image(&image)?;
        let key = read_key(&image, deployment)?;
        let admitted = Self { key, image };
        admitted.revalidate(deployment)?;
        Ok(admitted)
    }

    /// Reissues a root-provisioned template into current service-owned key custody.
    ///
    /// The source must remain an anonymous root-owned read-only canonical key image. The caller
    /// must already have the exact dedicated UID/GID named by `deployment`; the returned memfd is
    /// created under those credentials. Transient seed bytes are zeroized on every return path.
    pub fn reissue_root_template_for_current_service(
        image: File,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<Self, String> {
        Self::reissue_template_for_current_service(image, deployment, 0, 0)
    }

    fn reissue_template_for_current_service(
        image: File,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
        template_uid: u32,
        template_gid: u32,
    ) -> Result<Self, String> {
        let service = deployment.service();
        if rustix::process::geteuid().as_raw() != service.uid()
            || rustix::process::getegid().as_raw() != service.gid()
        {
            return Err(
                "external-anchor key reissue process does not have deployment credentials".into(),
            );
        }
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        validate_template_image(&image, template_uid, template_gid)?;
        let key = read_key(&image, deployment)?;
        let mut seed = key.to_bytes();
        drop(key);
        drop(image);
        Self::create_and_zeroize(&mut seed, deployment)
    }

    /// Admits and privately retains the key inherited at the canonical descriptor.
    pub fn from_inherited(
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<Self, String> {
        Self::from_inherited_at(
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
            deployment,
        )
    }

    /// Admits and privately retains the key inherited at one fixed descriptor.
    pub fn from_inherited_at(
        descriptor: RawFd,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_inherited_at(descriptor, ROLE, LENGTH)?;
        validate_secret_image(&image)?;
        let key = read_key(&image, deployment)?;
        let admitted = Self { key, image };
        admitted.revalidate(deployment)?;
        Ok(admitted)
    }

    /// Revalidates object identity, ownership, seals, bytes, role, and deployment binding.
    pub fn revalidate(
        &self,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<(), String> {
        self.image.revalidate()?;
        validate_secret_image(&self.image)?;
        let current = read_key(&self.image, deployment)?;
        if current.as_bytes() != self.key.as_bytes() {
            return Err("compiler-execution external-anchor signing-key bytes changed".to_owned());
        }
        Ok(())
    }

    /// Returns the public key without exposing seed or descriptor custody.
    pub fn verifying_key(&self) -> [u8; KEY_BYTES] {
        self.key.verifying_key().to_bytes()
    }

    /// Clones the same sealed read-only descriptor for one protected transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.image.revalidate()?;
        validate_secret_image(&self.image)?;
        self.image.try_clone_for_transfer()
    }

    /// Consumes exact deployment-bound custody and releases the in-memory key to the anchor.
    pub fn into_signing_key(
        self,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> Result<SigningKey, String> {
        self.revalidate(deployment)?;
        let Self { key, image } = self;
        drop(image);
        Ok(key)
    }
}

fn encode(
    seed: &[u8; KEY_BYTES],
    deployment: &CompilerExecutionExternalAnchorDeploymentV1,
) -> Zeroizing<[u8; WIRE_BYTES]> {
    let mut bytes = Zeroizing::new([0_u8; WIRE_BYTES]);
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
    bytes[12..16].copy_from_slice(&(WIRE_BYTES as u32).to_le_bytes());
    bytes[HEADER_BYTES..HEADER_BYTES + DEPLOYMENT_IDENTITY_BYTES]
        .copy_from_slice(deployment.identity().as_bytes());
    bytes[HEADER_BYTES + DEPLOYMENT_IDENTITY_BYTES..].copy_from_slice(seed);
    bytes
}

fn read_key(
    image: &SealedCapabilityImage,
    deployment: &CompilerExecutionExternalAnchorDeploymentV1,
) -> Result<SigningKey, String> {
    let image = image.try_clone_for_transfer()?;
    let mut bytes = Zeroizing::new([0_u8; WIRE_BYTES]);
    image
        .read_exact_at(bytes.as_mut_slice(), 0)
        .map_err(|error| format!("cannot read {}: {error}", ROLE.name))?;
    validate_header(bytes.as_slice())?;
    if bytes[HEADER_BYTES..HEADER_BYTES + DEPLOYMENT_IDENTITY_BYTES]
        != *deployment.identity().as_bytes()
    {
        return Err("external-anchor signing-key capability targets another deployment".to_owned());
    }
    let mut seed = Zeroizing::new([0_u8; KEY_BYTES]);
    seed.copy_from_slice(&bytes[HEADER_BYTES + DEPLOYMENT_IDENTITY_BYTES..]);
    let key = SigningKey::from_bytes(&seed);
    require_deployment_key(&key, deployment)?;
    Ok(key)
}

fn validate_header(bytes: &[u8]) -> Result<(), String> {
    if bytes[..8] != MAGIC {
        return Err("external-anchor signing-key capability has invalid role magic".to_owned());
    }
    if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != VERSION_V1 {
        return Err("external-anchor signing-key capability has unsupported version".to_owned());
    }
    if bytes[10..12].iter().any(|byte| *byte != 0)
        || bytes[16..HEADER_BYTES].iter().any(|byte| *byte != 0)
    {
        return Err("external-anchor signing-key capability has nonzero reserved bytes".to_owned());
    }
    if u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize != WIRE_BYTES {
        return Err(
            "external-anchor signing-key capability has noncanonical declared length".to_owned(),
        );
    }
    Ok(())
}

fn require_deployment_key(
    key: &SigningKey,
    deployment: &CompilerExecutionExternalAnchorDeploymentV1,
) -> Result<(), String> {
    if key.verifying_key().as_bytes() != deployment.verifying_key() {
        return Err(
            "external-anchor signing key does not match the admitted deployment".to_owned(),
        );
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;

    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
    };

    use super::*;

    fn deployment(
        anchor_seed: u8,
        service_uid: u32,
    ) -> CompilerExecutionExternalAnchorDeploymentV1 {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x11; 32], 4096).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x22; 32], 8192).unwrap(),
            SigningKey::from_bytes(&[0x33; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[anchor_seed; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            1001,
            1002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(service_uid, 1004).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x54; 32], 12288).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 16384).unwrap(),
            &policy,
        )
        .unwrap();
        CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy,
            CompilerExecutionIssuerMeasurementV1::new([0x66; 32], 32768).unwrap(),
        )
        .unwrap()
    }

    fn current_service_deployment(anchor_seed: u8) -> CompilerExecutionExternalAnchorDeploymentV1 {
        let service_uid = rustix::process::geteuid().as_raw();
        let service_gid = rustix::process::getegid().as_raw();
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x11; 32], 4096).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x22; 32], 8192).unwrap(),
            SigningKey::from_bytes(&[0x33; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[anchor_seed; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        let service =
            CompilerExecutionExternalAnchorServiceIdentityV1::new(service_uid, service_gid)
                .unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            if service_uid == 1 { 2 } else { 1 },
            if service_gid == 1 { 2 } else { 1 },
            service,
            CompilerExecutionIssuerMeasurementV1::new([0x54; 32], 12288).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 16384).unwrap(),
            &policy,
        )
        .unwrap();
        CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy,
            CompilerExecutionIssuerMeasurementV1::new([0x66; 32], 32768).unwrap(),
        )
        .unwrap()
    }

    fn raw_image(bytes: &[u8], mode: u32, seals: rustix::fs::SealFlags) -> File {
        let image = rustix::fs::memfd_create(
            "fe2o3-hostile-external-anchor-signing-key-v1",
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

    fn read_only(image: File) -> File {
        let path = PathBuf::from(format!("/proc/self/fd/{}", image.as_raw_fd()));
        rustix::fs::open(&path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
            .map(File::from)
            .unwrap()
    }

    #[test]
    fn seed_is_zeroized_and_exact_key_survives_consuming_transfer() {
        let expected = deployment(7, 1003);
        let mut seed = [7; KEY_BYTES];
        let capability = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed, &expected,
        )
        .unwrap();
        assert_eq!(seed, [0; KEY_BYTES]);
        assert_eq!(capability.verifying_key(), *expected.verifying_key());

        let recovered = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
            capability.try_clone_for_transfer().unwrap(),
            &expected,
        )
        .unwrap();
        let key = recovered.into_signing_key(&expected).unwrap();
        assert_eq!(key.verifying_key().as_bytes(), expected.verifying_key());
    }

    #[test]
    fn trusted_template_reissues_under_exact_current_service_credentials() {
        if rustix::process::geteuid().is_root() || rustix::process::getegid().is_root() {
            return;
        }
        let expected = current_service_deployment(7);
        let mut seed = [7; KEY_BYTES];
        let template = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed, &expected,
        )
        .unwrap();
        let current_uid = rustix::process::geteuid().as_raw();
        let current_gid = rustix::process::getegid().as_raw();
        let reissued = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::reissue_template_for_current_service(
            template.try_clone_for_transfer().unwrap(),
            &expected,
            current_uid,
            current_gid,
        )
        .unwrap();
        reissued.revalidate(&expected).unwrap();
        assert_eq!(reissued.verifying_key(), *expected.verifying_key());

        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::reissue_template_for_current_service(
                template.try_clone_for_transfer().unwrap(),
                &expected,
                current_uid.wrapping_add(1),
                current_gid,
            )
            .is_err()
        );
    }

    #[test]
    fn wrong_key_or_deployment_rejects_and_creation_zeroizes() {
        let expected = deployment(7, 1003);
        let mut wrong_seed = [8; KEY_BYTES];
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
                &mut wrong_seed,
                &expected,
            )
            .is_err()
        );
        assert_eq!(wrong_seed, [0; KEY_BYTES]);

        let mut seed = [7; KEY_BYTES];
        let capability = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed, &expected,
        )
        .unwrap();
        let other_deployment = deployment(7, 2003);
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
                capability.try_clone_for_transfer().unwrap(),
                &other_deployment,
            )
            .is_err()
        );
    }

    #[test]
    fn every_wire_mutation_and_issuer_role_shape_rejects() {
        let expected = deployment(7, 1003);
        let canonical = encode(&[7; KEY_BYTES], &expected);
        for offset in 0..WIRE_BYTES {
            let mut mutated = *canonical;
            mutated[offset] ^= 1;
            let image = raw_image(&mutated, 0o400, crate::sealed_image::REQUIRED_SEALS);
            assert!(
                CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
                    read_only(image),
                    &expected,
                )
                .is_err(),
                "mutation at byte {offset} was admitted"
            );
        }

        let issuer_shape = raw_image(&[7; KEY_BYTES], 0o400, crate::sealed_image::REQUIRED_SEALS);
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
                read_only(issuer_shape),
                &expected,
            )
            .is_err()
        );
    }

    #[test]
    fn writable_ordinary_malformed_and_inheritable_images_reject() {
        let expected = deployment(7, 1003);
        let canonical = encode(&[7; KEY_BYTES], &expected);

        let writable = SealedCapabilityImage::create(canonical.as_slice(), ROLE, LENGTH).unwrap();
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
                writable.try_clone_for_transfer().unwrap(),
                &expected,
            )
            .is_err()
        );

        let path = std::env::temp_dir().join(format!(
            "fe2o3-external-anchor-signing-key-ordinary-{}",
            std::process::id()
        ));
        fs::write(&path, canonical.as_slice()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
                File::open(&path).unwrap(),
                &expected,
            )
            .is_err()
        );
        fs::remove_file(path).unwrap();

        let short = raw_image(
            &canonical[..WIRE_BYTES - 1],
            0o400,
            crate::sealed_image::REQUIRED_SEALS,
        );
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(short, &expected)
                .is_err()
        );

        let permissive = raw_image(
            canonical.as_slice(),
            0o600,
            crate::sealed_image::REQUIRED_SEALS,
        );
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(permissive, &expected)
                .is_err()
        );

        let incomplete = raw_image(
            canonical.as_slice(),
            0o400,
            rustix::fs::SealFlags::WRITE
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::SHRINK,
        );
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(incomplete, &expected)
                .is_err()
        );

        let mut seed = [7; KEY_BYTES];
        let capability = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed, &expected,
        )
        .unwrap();
        let inheritable = capability.try_clone_for_transfer().unwrap();
        rustix::io::fcntl_setfd(&inheritable, rustix::io::FdFlags::empty()).unwrap();
        assert!(
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_file(
                inheritable,
                &expected,
            )
            .is_err()
        );
    }

    #[test]
    fn inherited_key_is_retained_privately() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let expected = deployment(7, 1003);
        let mut seed = [7; KEY_BYTES];
        let capability = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed, &expected,
        )
        .unwrap();
        let installed = rustix::io::fcntl_dupfd_cloexec(
            capability.try_clone_for_transfer().unwrap(),
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
        )
        .unwrap();
        assert_eq!(
            installed.as_raw_fd(),
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1
        );
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();
        let retained =
            CompilerExecutionExternalAnchorSigningKeyCapabilityV1::from_inherited(&expected)
                .unwrap();
        drop(installed);
        assert_eq!(
            retained
                .into_signing_key(&expected)
                .unwrap()
                .verifying_key()
                .as_bytes(),
            expected.verifying_key()
        );
    }

    #[test]
    fn debug_never_contains_seed_or_descriptor() {
        let expected = deployment(7, 1003);
        let mut seed = [7; KEY_BYTES];
        let capability = CompilerExecutionExternalAnchorSigningKeyCapabilityV1::create_and_zeroize(
            &mut seed, &expected,
        )
        .unwrap();
        let debug = format!("{capability:?}");
        assert_eq!(
            debug,
            format!(
                "CompilerExecutionExternalAnchorSigningKeyCapabilityV1 {{ authority: \
                 \"external-anchor-signing-key-custody-only\", verifying_key: {:?}, .. }}",
                expected.verifying_key()
            )
        );
        assert!(!debug.contains("descriptor"));
    }
}
