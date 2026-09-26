//! Budgeted custody of policy-neutral listener/root inputs, not compiler authority.
use crate::{
    IssuerServiceCredentialProfileV1 as Credentials,
    listener::{ListenerFilesystemPolicyV1, ProvisionedProtectedIssuerSocketV1, SocketError},
    root_checks::{self, RootCheckError, RootSnapshot},
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    fs::File,
    io,
    mem::size_of,
    os::fd::{BorrowedFd, OwnedFd},
    path::Path,
};

type Inputs = ProvisionedProtectedIssuerServiceInputsV2;
type Failure = ProtectedIssuerServiceProvisioningErrorV2;
use ProtectedIssuerServiceProvisioningStorageV2 as Storage;
type Result<T> = std::result::Result<T, Failure>;
const ENTRY: usize = 8;
const MAX_PATH_BYTES: usize = 108;

/// Native custody of the sole production listener and exact service-owned root.
///
/// This policy-neutral owner is shared by both native policy families. It grants
/// no compiler authority, provisioning provenance, or launch permission. There
/// is no V1 conversion and no public pathname selector. Admission checks target
/// ownership; the coordinator separately checks its own root confinement.
///
/// Operations restore entry storage on success, error and unwind without refunding
/// work. Returned deltas are unreserved: reserve before retaining a returned owner
/// and retire charges only after dropping it. Consuming failure closes both inputs
/// before the caller releases their reservations.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2;
/// fn cloned<T: Clone>() {}
/// cloned::<ProvisionedProtectedIssuerServiceInputsV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ProvisionedProtectedIssuerServiceInputsV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProvisionedProtectedIssuerServiceInputsV1, ProvisionedProtectedIssuerServiceInputsV2};
/// fn upgrade(old: ProvisionedProtectedIssuerServiceInputsV1) -> ProvisionedProtectedIssuerServiceInputsV2 { old.into() }
/// ```
pub struct ProvisionedProtectedIssuerServiceInputsV2 {
    credentials: Credentials,
    listener: ProvisionedProtectedIssuerSocketV1,
    root: File,
    root_snapshot: RootSnapshot,
}

impl Inputs {
    /// Logical charge of the ordered input or transferred descriptor pair.
    pub const PAIR_STORAGE: usize = size_of::<(OwnedFd, Storage)>() + size_of::<(File, Storage)>();
    /// Logical growth above the consumed pair, including the bounded owned path.
    pub const OWNER_GROWTH: usize =
        size_of::<(Self, Storage)>() + MAX_PATH_BYTES - Self::PAIR_STORAGE;
    /// Fixed charge for fewer than 256 syscalls and bounded path/snapshot work.
    /// Each kernel call is attempted once; this is not a wall-clock guarantee.
    pub const WORK: usize = ENTRY + 256 * 1024;
    /// Logical temporary frame allowance, excluding retained owner/pair growth.
    pub const SCRATCH: usize = 8 * size_of::<Self>() + 4096;

    /// Admits fresh descriptors using the fixed production pathname and policy.
    pub fn admit(
        listener: OwnedFd,
        root: File,
        credentials: Credentials,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_at(
            listener,
            root,
            credentials,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
            ListenerFilesystemPolicyV1::production(credentials),
            budget,
        )
    }

    fn admit_at(
        listener: OwnedFd,
        root: File,
        credentials: Credentials,
        path: &Path,
        filesystem: ListenerFilesystemPolicyV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        budget.with_prepaid_scope(Self::PAIR_STORAGE, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            if path.as_os_str().as_encoded_bytes().len() >= MAX_PATH_BYTES {
                return Err(Failure::Listener(
                    "listener pathname exceeds Unix socket capacity",
                ));
            }
            b.reserve_storage(Self::OWNER_GROWTH)?;
            let root_snapshot = root_checks::inspect(&root, credentials)?;
            let listener =
                ProvisionedProtectedIssuerSocketV1::admit_checked(listener, path, filesystem)?;
            let admitted = Self {
                credentials,
                listener,
                root,
                root_snapshot,
            };
            admitted.check()?;
            Ok((admitted, Storage(Self::OWNER_GROWTH)))
        })
    }

    /// Full logical reservation required while this owner remains live.
    pub const fn retained_storage(&self) -> usize {
        Self::PAIR_STORAGE + Self::OWNER_GROWTH
    }

    /// Target service credentials, without exposing descriptor custody.
    pub const fn credentials(&self) -> Credentials {
        self.credentials
    }

    /// Rechecks the exact root and monotone bound-to-listening socket continuity.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            Self::WORK,
            Self::SCRATCH,
            |_| self.check(),
        )
    }

    /// Produces exact close-on-exec listener/root aliases before activation only.
    /// The returned pair is inert; staging must retain this owner and validate the
    /// final staged files under the original request ledger before release.
    pub fn try_clone_ordered_for_spawn(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<((OwnedFd, File), Storage)> {
        budget.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            Self::WORK,
            Self::SCRATCH,
            |b| {
                b.reserve_storage(Self::PAIR_STORAGE)?;
                self.check()?;
                let listener = self.listener.clone_checked()?;
                let root = File::from(rustix::io::fcntl_dupfd_cloexec(&self.root, 0).map_err(
                    |errno| Failure::Io {
                        operation: "clone protected issuer deployment root",
                        source: errno.into(),
                    },
                )?);
                self.check_root(&root)?;
                self.listener.validate_clone_checked(&listener)?;
                self.check()?;
                Ok(((listener, root), Storage(Self::PAIR_STORAGE)))
            },
        )
    }

    /// Validates the exact final staged pair against this retained owner.
    /// Requires both owner and pair reservations on the original account. A
    /// different same-shaped root or socket is refused, as is activation.
    ///
    /// ```
    /// use fe2o3_compiler_execution_supervisor::{ProvisionedProtectedIssuerServiceInputsV2 as Inputs,
    ///     ProtectedIssuerServiceProvisioningErrorV2 as Error};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use std::{fs::File, os::fd::AsFd};
    /// fn check(inputs: &Inputs, listener: &File, root: &File, b: &mut Budget<'_>) -> Result<(), Error> {
    ///     inputs.validate_transfer(listener.as_fd(), root, b)
    /// }
    /// ```
    pub fn validate_transfer(
        &self,
        listener: BorrowedFd<'_>,
        root: &File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = self
            .retained_storage()
            .checked_add(Self::PAIR_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, |_| {
            self.check()?;
            self.check_root(root)?;
            self.listener.validate_clone_checked(&listener)?;
            self.check()
        })
    }

    fn check_root(&self, root: &File) -> Result<()> {
        if root_checks::inspect(root, self.credentials)? != self.root_snapshot {
            return Err(Failure::Root("root descriptor identity changed"));
        }
        Ok(())
    }

    fn check(&self) -> Result<()> {
        self.check_root(&self.root)?;
        self.listener.revalidate_checked()?;
        Ok(())
    }
}

impl fmt::Debug for Inputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProvisionedProtectedIssuerServiceInputsV2")
            .field("authority", &"deployment-input-custody-only")
            .field("credentials", &self.credentials)
            .finish_non_exhaustive()
    }
}

/// Unreserved logical storage delta, not authority or a descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerServiceProvisioningStorageV2(usize);
impl Storage {
    /// Reserve before retaining the returned custody; retire only after Drop.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Fixed-shape native provisioning failures, without allocated diagnostics.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerServiceProvisioningErrorV2 {
    /// Original request work or storage was insufficient.
    Resource(Resource),
    /// Root shape or identity changed.
    Root(&'static str),
    /// Socket shape, identity, pathname, or activation state was invalid.
    Listener(&'static str),
    /// One bounded kernel operation failed.
    Io {
        /// Fixed operation name.
        operation: &'static str,
        /// Kernel error without custom allocated text.
        source: io::Error,
    },
}
impl From<Resource> for Failure {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<RootCheckError> for Failure {
    fn from(error: RootCheckError) -> Self {
        match error {
            RootCheckError::Invalid(reason) => Self::Root(reason),
            RootCheckError::Io { operation, errno } => Self::Io {
                operation,
                source: errno.into(),
            },
        }
    }
}
impl From<SocketError> for Failure {
    fn from(error: SocketError) -> Self {
        match error {
            SocketError::InvalidListener(reason) => Self::Listener(reason),
            SocketError::Io { operation, source } => Self::Io { operation, source },
        }
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => write!(f, "provisioning resource limit: {error}"),
            Self::Root(reason) => write!(f, "invalid protected issuer root: {reason}"),
            Self::Listener(reason) => write!(f, "invalid protected issuer listener: {reason}"),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
        }
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "provisioning_v2_tests.rs"]
mod tests;
