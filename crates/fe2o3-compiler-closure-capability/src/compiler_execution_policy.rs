use std::fs::File;
use std::os::fd::RawFd;
use std::process::Command;

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, CompilerExecutionIssuerPolicyV1,
};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution policy capability",
    memfd_name: "fe2o3-compiler-execution-policy-v1",
};
const LENGTH: ImageLength = ImageLength::Exact(COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1);

/// Reserved descriptor used to pass the caller-pinned issuer policy into rustc.
pub const COMPILER_EXECUTION_POLICY_CHILD_FD_V1: RawFd = 202;

/// Immutable descriptor capability carrying one exact compiler-execution issuer policy.
///
/// The policy is public trust configuration, not a signing capability. This value grants no
/// compiler, artifact-publication, load, launch, or execution authority.
pub struct CompilerExecutionPolicyCapabilityV1 {
    policy: CompilerExecutionIssuerPolicyV1,
    image: SealedCapabilityImage,
}

impl CompilerExecutionPolicyCapabilityV1 {
    /// Creates and seals the exact canonical policy image.
    pub fn create(policy: CompilerExecutionIssuerPolicyV1) -> Result<Self, String> {
        let image = SealedCapabilityImage::create(policy.canonical_bytes(), ROLE, LENGTH)?;
        let admitted = Self { policy, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an already transferred immutable policy image.
    pub fn from_file(image: File) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        let policy = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { policy, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits the policy inherited at the canonical rustc descriptor number.
    pub fn from_inherited_child() -> Result<Self, String> {
        Self::from_inherited_at(COMPILER_EXECUTION_POLICY_CHILD_FD_V1)
    }

    /// Retains a private close-on-exec duplicate of one inherited policy descriptor.
    pub fn from_inherited_at(child_fd: RawFd) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_inherited_at(child_fd, ROLE, LENGTH)?;
        let policy = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { policy, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Borrows the exact caller-pinned policy.
    pub const fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        &self.policy
    }

    /// Revalidates descriptor identity, mode, seals, length, bytes, and canonical policy equality.
    pub fn revalidate(&self) -> Result<(), String> {
        let current = decode(&self.image.read_exact_bytes()?)?;
        if current != self.policy {
            return Err("compiler-execution policy capability bytes changed".to_owned());
        }
        Ok(())
    }

    /// Clones the same sealed descriptor for one broker or service transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }

    /// Installs this exact immutable policy at a reserved child descriptor.
    pub fn inherit_for_child_at(
        &self,
        command: &mut Command,
        child_fd: RawFd,
    ) -> Result<(), String> {
        self.revalidate()?;
        self.image.inherit_for_child_at(command, child_fd)
    }

    /// Installs this policy at the canonical rustc child descriptor.
    pub fn inherit_for_child(&self, command: &mut Command) -> Result<(), String> {
        self.inherit_for_child_at(command, COMPILER_EXECUTION_POLICY_CHILD_FD_V1)
    }
}

fn decode(bytes: &[u8]) -> Result<CompilerExecutionIssuerPolicyV1, String> {
    CompilerExecutionIssuerPolicyV1::decode(bytes)
        .map_err(|error| format!("compiler-execution policy capability is not canonical: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1;

    use super::*;
    use crate::sealed_image::REQUIRED_SEALS;

    fn policy(seed: u8) -> CompilerExecutionIssuerPolicyV1 {
        let key = SigningKey::from_bytes(&[seed; 32]);
        CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[seed.wrapping_add(1); 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn exact_policy_is_sealed_transferred_and_revalidated() {
        let expected = policy(7);
        let capability = CompilerExecutionPolicyCapabilityV1::create(expected.clone()).unwrap();
        assert_eq!(capability.policy(), &expected);
        assert_eq!(
            rustix::fs::fcntl_get_seals(capability.image.as_file()).unwrap(),
            REQUIRED_SEALS
        );
        assert!(capability.image.as_file().set_len(0).is_err());
        let transferred = capability.try_clone_for_transfer().unwrap();
        let recovered = CompilerExecutionPolicyCapabilityV1::from_file(transferred).unwrap();
        assert_eq!(recovered.policy(), &expected);
        recovered.revalidate().unwrap();
    }

    #[test]
    fn ordinary_mutable_images_fail_and_distinct_policies_remain_distinct() {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-execution-policy-hostile-{}",
            std::process::id()
        ));
        fs::write(&path, policy(7).canonical_bytes()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionPolicyCapabilityV1::from_file(File::open(&path).unwrap()).is_err()
        );
        fs::remove_file(path).unwrap();

        let first = CompilerExecutionPolicyCapabilityV1::create(policy(7)).unwrap();
        let second = CompilerExecutionPolicyCapabilityV1::create(policy(8)).unwrap();
        assert_ne!(first.policy(), second.policy());
        assert_ne!(first.policy().identity(), second.policy().identity(),);
    }

    #[test]
    fn inherited_policy_is_retained_at_a_private_descriptor() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let expected = policy(7);
        let capability = CompilerExecutionPolicyCapabilityV1::create(expected.clone()).unwrap();
        let child_fd = 511;
        let installed =
            rustix::io::fcntl_dupfd_cloexec(capability.image.as_file(), child_fd).unwrap();
        assert_eq!(installed.as_raw_fd(), child_fd);
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();

        let retained = CompilerExecutionPolicyCapabilityV1::from_inherited_at(child_fd).unwrap();
        drop(installed);
        assert_eq!(retained.policy(), &expected);
        assert!(
            rustix::io::fcntl_getfd(retained.image.as_file())
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }

    #[test]
    fn child_receives_only_the_requested_policy_descriptor() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let capability = CompilerExecutionPolicyCapabilityV1::create(policy(7)).unwrap();
        let child_fd = 511;
        let mut command = std::process::Command::new("/bin/sh");
        command.arg("-c").arg(format!(
            "test $(wc -c </proc/self/fd/{child_fd}) -eq {COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1}"
        ));
        capability
            .inherit_for_child_at(&mut command, child_fd)
            .unwrap();
        assert!(command.status().unwrap().success());
        capability.revalidate().unwrap();
    }

    #[test]
    fn canonical_child_installation_uses_fd_202_and_exact_bytes() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let expected = policy(7);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-execution-policy-expected-{}",
            std::process::id()
        ));
        fs::write(&path, expected.canonical_bytes()).unwrap();

        let capability = CompilerExecutionPolicyCapabilityV1::create(expected).unwrap();
        let mut command = std::process::Command::new("/usr/bin/cmp");
        command
            .arg("-s")
            .arg(format!(
                "/proc/self/fd/{COMPILER_EXECUTION_POLICY_CHILD_FD_V1}"
            ))
            .arg(&path);
        capability.inherit_for_child(&mut command).unwrap();
        drop(capability);
        assert!(command.status().unwrap().success());
        fs::remove_file(path).unwrap();
    }
}
