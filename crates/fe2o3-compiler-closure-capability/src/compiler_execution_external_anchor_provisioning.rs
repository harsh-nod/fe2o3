use std::fs::File;
use std::os::fd::RawFd;

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
    CompilerExecutionExternalAnchorProvisioningV1,
};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution external-anchor provisioning capability",
    memfd_name: "fe2o3-compiler-execution-external-anchor-provisioning-v1",
};
const LENGTH: ImageLength =
    ImageLength::Exact(COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1);

/// Reserved descriptor carrying trusted provisioning configuration into the anchor helper.
pub const COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1: RawFd = 223;

/// Move-only immutable descriptor capability for one external-anchor provisioning manifest.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorProvisioningCapabilityV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionExternalAnchorProvisioningCapabilityV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorProvisioningCapabilityV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionExternalAnchorProvisioningCapabilityV1>();
/// ```
pub struct CompilerExecutionExternalAnchorProvisioningCapabilityV1 {
    provisioning: CompilerExecutionExternalAnchorProvisioningV1,
    image: SealedCapabilityImage,
}

impl CompilerExecutionExternalAnchorProvisioningCapabilityV1 {
    /// Creates a sealed canonical provisioning capability.
    pub fn create(
        provisioning: CompilerExecutionExternalAnchorProvisioningV1,
    ) -> Result<Self, String> {
        let image = SealedCapabilityImage::create(provisioning.canonical_bytes(), ROLE, LENGTH)?;
        let admitted = Self {
            provisioning,
            image,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an already transferred immutable provisioning image.
    pub fn from_file(image: File) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        let provisioning = decode(&image.read_exact_bytes()?)?;
        let admitted = Self {
            provisioning,
            image,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits and privately retains the capability inherited at FD 223.
    pub fn from_inherited() -> Result<Self, String> {
        Self::from_inherited_at(COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1)
    }

    /// Admits and privately retains a capability at an explicit fixed descriptor.
    pub fn from_inherited_at(descriptor: RawFd) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_inherited_at(descriptor, ROLE, LENGTH)?;
        let provisioning = decode(&image.read_exact_bytes()?)?;
        let admitted = Self {
            provisioning,
            image,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Returns the inert canonical provisioning manifest.
    pub const fn provisioning(&self) -> &CompilerExecutionExternalAnchorProvisioningV1 {
        &self.provisioning
    }

    /// Revalidates exact object identity, seals, bytes, and canonical equality.
    pub fn revalidate(&self) -> Result<(), String> {
        if decode(&self.image.read_exact_bytes()?)? != self.provisioning {
            return Err(
                "compiler-execution external-anchor provisioning capability changed".into(),
            );
        }
        Ok(())
    }

    /// Clones the same immutable descriptor for one protected transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }
}

fn decode(bytes: &[u8]) -> Result<CompilerExecutionExternalAnchorProvisioningV1, String> {
    CompilerExecutionExternalAnchorProvisioningV1::decode(bytes).map_err(|error| {
        format!("compiler-execution external-anchor provisioning is not canonical: {error}")
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorDeploymentV1,
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentV1,
    };

    use super::*;

    fn provisioning() -> CompilerExecutionExternalAnchorProvisioningV1 {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x11; 32], 4096).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x22; 32], 8192).unwrap(),
            SigningKey::from_bytes(&[0x33; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[0x44; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            1001,
            1002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(1003, 1004).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 16384).unwrap(),
            &policy,
        )
        .unwrap();
        let deployment = CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy,
            CompilerExecutionIssuerMeasurementV1::new([0x66; 32], 32768).unwrap(),
        )
        .unwrap();
        CompilerExecutionExternalAnchorProvisioningV1::new(
            &deployment,
            CompilerExecutionIssuerMeasurementV1::new([0x77; 32], 65536).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn exact_provisioning_is_sealed_transferred_and_revalidated() {
        let expected = provisioning();
        let capability =
            CompilerExecutionExternalAnchorProvisioningCapabilityV1::create(expected.clone())
                .unwrap();
        let recovered = CompilerExecutionExternalAnchorProvisioningCapabilityV1::from_file(
            capability.try_clone_for_transfer().unwrap(),
        )
        .unwrap();
        assert_eq!(recovered.provisioning(), &expected);
        recovered.revalidate().unwrap();
    }

    #[test]
    fn mutable_ordinary_file_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-external-anchor-provisioning-capability-hostile-{}",
            std::process::id()
        ));
        fs::write(&path, provisioning().canonical_bytes()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionExternalAnchorProvisioningCapabilityV1::from_file(
                File::open(&path).unwrap()
            )
            .is_err()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn inherited_provisioning_is_retained_privately() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let expected = provisioning();
        let capability =
            CompilerExecutionExternalAnchorProvisioningCapabilityV1::create(expected.clone())
                .unwrap();
        let installed = rustix::io::fcntl_dupfd_cloexec(
            capability.try_clone_for_transfer().unwrap(),
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1,
        )
        .unwrap();
        assert_eq!(
            installed.as_raw_fd(),
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1
        );
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();
        let retained =
            CompilerExecutionExternalAnchorProvisioningCapabilityV1::from_inherited().unwrap();
        drop(installed);
        assert_eq!(retained.provisioning(), &expected);
        retained.revalidate().unwrap();
    }
}
