use std::fs::File;
use std::os::fd::RawFd;
use std::process::Command;

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1, CompilerExecutionServiceLaunchManifestV1,
};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution service launch capability",
    memfd_name: "fe2o3-compiler-execution-service-launch-v1",
};
const LENGTH: ImageLength = ImageLength::Exact(COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1);

/// Reserved descriptor carrying the launch manifest into the static protected issuer.
pub const COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_CHILD_FD_V1: RawFd = 8;

/// Immutable descriptor capability carrying one exact service launch manifest.
///
/// The manifest is inert coordination evidence. This value grants no process, issuer, signing,
/// compiler, artifact-publication, loading, launch, or execution authority.
pub struct CompilerExecutionServiceLaunchCapabilityV1 {
    manifest: CompilerExecutionServiceLaunchManifestV1,
    image: SealedCapabilityImage,
}

impl CompilerExecutionServiceLaunchCapabilityV1 {
    /// Creates and seals the exact canonical launch manifest.
    pub fn create(manifest: CompilerExecutionServiceLaunchManifestV1) -> Result<Self, String> {
        let image = SealedCapabilityImage::create(manifest.canonical_bytes(), ROLE, LENGTH)?;
        let admitted = Self { manifest, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an already transferred immutable launch-manifest image.
    pub fn from_file(image: File) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        let manifest = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { manifest, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits the manifest inherited at the canonical protected-issuer descriptor number.
    pub fn from_inherited_child() -> Result<Self, String> {
        Self::from_inherited_at(COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_CHILD_FD_V1)
    }

    /// Retains a private close-on-exec duplicate of one inherited manifest descriptor.
    pub fn from_inherited_at(child_fd: RawFd) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_inherited_at(child_fd, ROLE, LENGTH)?;
        let manifest = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { manifest, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Borrows the exact canonical launch manifest.
    pub const fn manifest(&self) -> &CompilerExecutionServiceLaunchManifestV1 {
        &self.manifest
    }

    /// Revalidates descriptor identity, mode, seals, length, bytes, and canonical equality.
    pub fn revalidate(&self) -> Result<(), String> {
        let current = decode(&self.image.read_exact_bytes()?)?;
        if current != self.manifest {
            return Err("compiler-execution service launch capability bytes changed".to_owned());
        }
        Ok(())
    }

    /// Clones the same sealed descriptor for one protected-supervisor transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }

    /// Installs the manifest at one explicitly selected child descriptor.
    pub fn inherit_for_child_at(
        &self,
        command: &mut Command,
        child_fd: RawFd,
    ) -> Result<(), String> {
        self.revalidate()?;
        self.image.inherit_for_child_at(command, child_fd)
    }

    /// Installs the manifest at canonical protected-issuer FD 8.
    pub fn inherit_for_child(&self, command: &mut Command) -> Result<(), String> {
        self.inherit_for_child_at(
            command,
            COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_CHILD_FD_V1,
        )
    }
}

fn decode(bytes: &[u8]) -> Result<CompilerExecutionServiceLaunchManifestV1, String> {
    CompilerExecutionServiceLaunchManifestV1::decode(bytes).map_err(|error| {
        format!("compiler-execution service launch capability is not canonical: {error}")
    })
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionClientProcessIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1,
    };

    use super::*;
    use crate::sealed_image::REQUIRED_SEALS;

    fn manifest(seed: u8) -> CompilerExecutionServiceLaunchManifestV1 {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
        )
        .unwrap();
        CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(1234, 1000, 1001).unwrap(),
            &policy,
        )
    }

    #[test]
    fn exact_manifest_is_sealed_transferred_and_revalidated() {
        let expected = manifest(7);
        let capability =
            CompilerExecutionServiceLaunchCapabilityV1::create(expected.clone()).unwrap();
        assert_eq!(capability.manifest(), &expected);
        assert_eq!(
            rustix::fs::fcntl_get_seals(capability.image.as_file()).unwrap(),
            REQUIRED_SEALS
        );
        assert!(capability.image.as_file().set_len(0).is_err());
        let recovered = CompilerExecutionServiceLaunchCapabilityV1::from_file(
            capability.try_clone_for_transfer().unwrap(),
        )
        .unwrap();
        assert_eq!(recovered.manifest(), &expected);
        recovered.revalidate().unwrap();
    }

    #[test]
    fn ordinary_file_and_distinct_manifest_substitution_fail_closed() {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-execution-launch-hostile-{}",
            std::process::id()
        ));
        fs::write(&path, manifest(7).canonical_bytes()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionServiceLaunchCapabilityV1::from_file(File::open(&path).unwrap())
                .is_err()
        );
        fs::remove_file(path).unwrap();

        let first = CompilerExecutionServiceLaunchCapabilityV1::create(manifest(7)).unwrap();
        let second = CompilerExecutionServiceLaunchCapabilityV1::create(manifest(8)).unwrap();
        assert_ne!(first.manifest().identity(), second.manifest().identity());
    }

    #[test]
    fn inherited_manifest_is_retained_privately() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let expected = manifest(7);
        let capability =
            CompilerExecutionServiceLaunchCapabilityV1::create(expected.clone()).unwrap();
        let child_fd = 511;
        let installed =
            rustix::io::fcntl_dupfd_cloexec(capability.image.as_file(), child_fd).unwrap();
        assert_eq!(installed.as_raw_fd(), child_fd);
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();

        let retained =
            CompilerExecutionServiceLaunchCapabilityV1::from_inherited_at(child_fd).unwrap();
        drop(installed);
        assert_eq!(retained.manifest(), &expected);
        assert!(
            rustix::io::fcntl_getfd(retained.image.as_file())
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }

    #[test]
    fn canonical_child_installation_uses_fd_8_and_exact_bytes() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let expected = manifest(7);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-execution-launch-expected-{}",
            std::process::id()
        ));
        fs::write(&path, expected.canonical_bytes()).unwrap();

        let capability = CompilerExecutionServiceLaunchCapabilityV1::create(expected).unwrap();
        let mut command = std::process::Command::new("/usr/bin/cmp");
        command
            .arg("-s")
            .arg(format!(
                "/proc/self/fd/{COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_CHILD_FD_V1}"
            ))
            .arg(&path);
        capability.inherit_for_child(&mut command).unwrap();
        drop(capability);
        assert!(command.status().unwrap().success());
        fs::remove_file(path).unwrap();
    }
}
