//! Root-side admission of the two persistent protected-service inputs.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::Path;

use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1;

use crate::authority::ProtectedIssuerRootV1;
use crate::listener::ProtectedIssuerListenerV1;
use crate::{
    IssuerServiceCredentialProfileV1, ProtectedIssuerServiceErrorV1,
    ProtectedIssuerSupervisorErrorV1,
};

/// Root-retained admission of the exact production listener and service-owned durable root.
///
/// Admission validates the target service ownership without requiring the admitting process to
/// assume that identity. The value is move-only and exposes neither retained descriptor.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ProvisionedProtectedIssuerServiceInputsV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ProvisionedProtectedIssuerServiceInputsV1>();
/// ```
pub struct ProvisionedProtectedIssuerServiceInputsV1 {
    credentials: IssuerServiceCredentialProfileV1,
    listener: ProtectedIssuerListenerV1,
    root: ProtectedIssuerRootV1,
}

impl fmt::Debug for ProvisionedProtectedIssuerServiceInputsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProvisionedProtectedIssuerServiceInputsV1")
            .field("authority", &"deployment-input-custody-only")
            .field("credentials", &self.credentials)
            .field("listener", &COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1)
            .field("root", &"service-owned-0700-directory")
            .finish_non_exhaustive()
    }
}

impl ProvisionedProtectedIssuerServiceInputsV1 {
    /// Admits the listener at the sole production pathname and the exact service-owned root.
    pub fn admit(
        listener: OwnedFd,
        root: File,
        credentials: IssuerServiceCredentialProfileV1,
    ) -> Result<Self, ProtectedIssuerServiceProvisioningErrorV1> {
        Self::admit_at(
            listener,
            root,
            credentials,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
        )
    }

    pub(super) fn admit_at(
        listener: OwnedFd,
        root: File,
        credentials: IssuerServiceCredentialProfileV1,
        expected_path: &Path,
    ) -> Result<Self, ProtectedIssuerServiceProvisioningErrorV1> {
        let root = ProtectedIssuerRootV1::admit(root, credentials)
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Root)?;
        let listener = ProtectedIssuerListenerV1::admit(listener, expected_path)
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Listener)?;
        let admitted = Self {
            credentials,
            listener,
            root,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Revalidates the exact root metadata plus listener descriptor and pathname identities.
    pub fn revalidate(&self) -> Result<(), ProtectedIssuerServiceProvisioningErrorV1> {
        self.root
            .revalidate(self.credentials)
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Root)?;
        self.listener
            .revalidate()
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Listener)
    }

    /// Returns the target dedicated-service credentials without exposing descriptor custody.
    pub const fn credentials(&self) -> IssuerServiceCredentialProfileV1 {
        self.credentials
    }

    /// Consumes admission into root-retained deployment custody.
    pub fn into_deployment_transfer(
        self,
    ) -> Result<ProtectedIssuerServiceDeploymentInputsV1, ProtectedIssuerServiceProvisioningErrorV1>
    {
        self.revalidate()?;
        Ok(ProtectedIssuerServiceDeploymentInputsV1 {
            credentials: self.credentials,
            listener: self.listener,
            root: self.root,
        })
    }
}

/// One move-only ordered listener/root descriptor transfer for the protected-service launcher.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceDeploymentInputsV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ProtectedIssuerServiceDeploymentInputsV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceDeploymentInputsV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ProtectedIssuerServiceDeploymentInputsV1>();
/// ```
pub struct ProtectedIssuerServiceDeploymentInputsV1 {
    credentials: IssuerServiceCredentialProfileV1,
    listener: ProtectedIssuerListenerV1,
    root: ProtectedIssuerRootV1,
}

impl fmt::Debug for ProtectedIssuerServiceDeploymentInputsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedIssuerServiceDeploymentInputsV1")
            .field("authority", &"ordered-deployment-transfer-only")
            .finish_non_exhaustive()
    }
}

impl ProtectedIssuerServiceDeploymentInputsV1 {
    /// Revalidates retained root and listener continuity without exposing either descriptor.
    pub fn revalidate(&self) -> Result<(), ProtectedIssuerServiceProvisioningErrorV1> {
        self.root
            .revalidate(self.credentials)
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Root)?;
        self.listener
            .revalidate()
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Listener)
    }

    /// Returns the target dedicated-service credentials without exposing descriptor custody.
    pub const fn credentials(&self) -> IssuerServiceCredentialProfileV1 {
        self.credentials
    }

    /// Produces exact close-on-exec listener/root clones in fixed deployment order.
    pub fn try_clone_ordered_for_spawn(
        &self,
    ) -> Result<(OwnedFd, File), ProtectedIssuerServiceProvisioningErrorV1> {
        self.revalidate()?;
        let listener = self
            .listener
            .try_clone_for_deployment()
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Listener)?;
        let root = self
            .root
            .try_clone_for_launch(self.credentials)
            .map_err(ProtectedIssuerServiceProvisioningErrorV1::Root)?;
        self.revalidate()?;
        Ok((listener, root))
    }
}

/// Stable root-side persistent-input admission failures.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerServiceProvisioningErrorV1 {
    /// Durable-root admission or continuity failed.
    Root(ProtectedIssuerSupervisorErrorV1),
    /// Production-listener admission or continuity failed.
    Listener(ProtectedIssuerServiceErrorV1),
}

impl fmt::Display for ProtectedIssuerServiceProvisioningErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "protected issuer root failed: {error}"),
            Self::Listener(error) => write!(formatter, "protected issuer listener failed: {error}"),
        }
    }
}

impl Error for ProtectedIssuerServiceProvisioningErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Root(error) => Some(error),
            Self::Listener(error) => Some(error),
        }
    }
}
