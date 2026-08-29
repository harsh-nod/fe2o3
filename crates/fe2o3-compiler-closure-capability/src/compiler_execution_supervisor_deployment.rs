use std::fs::File;
use std::os::fd::RawFd;

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1, CompilerExecutionSupervisorDeploymentV1,
};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution supervisor deployment capability",
    memfd_name: "fe2o3-compiler-execution-supervisor-deployment-v1",
};
const LENGTH: ImageLength = ImageLength::Exact(COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1);

/// Reserved descriptor carrying trusted deployment configuration into the protected supervisor.
///
/// This lies above the static launcher's complete reserved `198..=215` descriptor range.
pub const COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1: RawFd = 220;

/// Move-only immutable descriptor capability for one protected-supervisor deployment manifest.
///
/// This value carries inert trust configuration. It exposes no descriptor, signing operation,
/// compiler authority, publication authority, process launch, load, or GPU authority.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSupervisorDeploymentCapabilityV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionSupervisorDeploymentCapabilityV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_closure_capability::CompilerExecutionSupervisorDeploymentCapabilityV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionSupervisorDeploymentCapabilityV1>();
/// ```
pub struct CompilerExecutionSupervisorDeploymentCapabilityV1 {
    deployment: CompilerExecutionSupervisorDeploymentV1,
    image: SealedCapabilityImage,
}

impl CompilerExecutionSupervisorDeploymentCapabilityV1 {
    /// Creates and seals one exact canonical deployment image.
    pub fn create(deployment: CompilerExecutionSupervisorDeploymentV1) -> Result<Self, String> {
        let image = SealedCapabilityImage::create(deployment.canonical_bytes(), ROLE, LENGTH)?;
        let admitted = Self { deployment, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits one already transferred immutable deployment image.
    pub fn from_file(image: File) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        let deployment = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { deployment, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits the exact deployment image inherited at the canonical supervisor descriptor.
    pub fn from_inherited() -> Result<Self, String> {
        Self::from_inherited_at(COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1)
    }

    /// Retains a private close-on-exec duplicate of one inherited deployment image.
    pub fn from_inherited_at(descriptor: RawFd) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_inherited_at(descriptor, ROLE, LENGTH)?;
        let deployment = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { deployment, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Borrows the exact inert deployment manifest.
    pub const fn deployment(&self) -> &CompilerExecutionSupervisorDeploymentV1 {
        &self.deployment
    }

    /// Revalidates descriptor identity, mode, seals, bytes, and canonical manifest equality.
    pub fn revalidate(&self) -> Result<(), String> {
        if decode(&self.image.read_exact_bytes()?)? != self.deployment {
            return Err("compiler-execution supervisor deployment capability changed".to_owned());
        }
        Ok(())
    }

    /// Clones the same sealed descriptor for one trusted provisioning transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }
}

fn decode(bytes: &[u8]) -> Result<CompilerExecutionSupervisorDeploymentV1, String> {
    CompilerExecutionSupervisorDeploymentV1::decode(bytes).map_err(|error| {
        format!("compiler-execution supervisor deployment capability is not canonical: {error}")
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1,
    };

    use super::*;

    fn deployment() -> CompilerExecutionSupervisorDeploymentV1 {
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
        CompilerExecutionSupervisorDeploymentV1::new(
            1001,
            1002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(1003, 1004).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 16384).unwrap(),
            &policy,
        )
        .unwrap()
    }

    #[test]
    fn exact_deployment_is_sealed_transferred_and_revalidated() {
        let expected = deployment();
        let capability =
            CompilerExecutionSupervisorDeploymentCapabilityV1::create(expected.clone()).unwrap();
        assert_eq!(capability.deployment(), &expected);
        let transferred = capability.try_clone_for_transfer().unwrap();
        let recovered =
            CompilerExecutionSupervisorDeploymentCapabilityV1::from_file(transferred).unwrap();
        assert_eq!(recovered.deployment(), &expected);
        recovered.revalidate().unwrap();
    }

    #[test]
    fn mutable_ordinary_file_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-supervisor-deployment-capability-hostile-{}",
            std::process::id()
        ));
        fs::write(&path, deployment().canonical_bytes()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionSupervisorDeploymentCapabilityV1::from_file(
                File::open(&path).unwrap()
            )
            .is_err()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn canonical_inherited_deployment_is_retained_privately() {
        let _guard = crate::FIXED_DESCRIPTOR_TEST_LOCK.lock().unwrap();
        let expected = deployment();
        let capability =
            CompilerExecutionSupervisorDeploymentCapabilityV1::create(expected.clone()).unwrap();
        let installed = rustix::io::fcntl_dupfd_cloexec(
            capability.try_clone_for_transfer().unwrap(),
            COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1,
        )
        .unwrap();
        assert_eq!(
            installed.as_raw_fd(),
            COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1
        );
        rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty()).unwrap();
        let retained = CompilerExecutionSupervisorDeploymentCapabilityV1::from_inherited().unwrap();
        drop(installed);
        assert_eq!(retained.deployment(), &expected);
        retained.revalidate().unwrap();
    }
}
