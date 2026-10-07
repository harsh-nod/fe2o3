//! Budgeted custody of policy-neutral listener/root inputs, not compiler authority.
use crate::{
    IssuerServiceCredentialProfileV1 as Credentials,
    listener::{ListenerFilesystemPolicyV1, ProvisionedProtectedIssuerSocketV1, SocketError},
    root_checks::{self, RootCheckError, RootSnapshot},
};
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV2 as Lease, LifecycleLeaseErrorV2,
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
    /// Logical charge of one service-root directory alias (never a listener).
    pub const ROOT_STORAGE: usize = size_of::<(File, Storage)>();
    /// Logical charge of one untrusted accepted transport endpoint.
    pub const CONNECTION_STORAGE: usize = size_of::<(OwnedFd, Storage)>();
    /// Logical growth above the consumed pair, including the bounded owned path.
    pub const OWNER_GROWTH: usize =
        size_of::<(Self, Storage)>() + MAX_PATH_BYTES - Self::PAIR_STORAGE;
    /// Fixed charge for fewer than 256 syscalls and bounded path/snapshot work.
    /// Each kernel call is attempted once; this is not a wall-clock guarantee.
    pub const WORK: usize = ENTRY + 256 * 1024;
    /// Logical temporary frame allowance, excluding retained owner/pair growth.
    pub const SCRATCH: usize = 8 * size_of::<Self>() + 4096;
    /// Complete work for root-bound lifecycle validation, including input continuity.
    pub const LIFECYCLE_WORK: usize = Self::WORK + Lease::ROOT_BINDING_WORK;
    /// Complete additional peak while the lease checks the retained root's parent.
    pub const LIFECYCLE_SCRATCH: usize = Self::SCRATCH + Lease::ROOT_BINDING_SCRATCH;

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

    /// Admits policy-neutral inputs at the separate native application pathname.
    /// This observes descriptors and their original filesystem objects only;
    /// it does not authorize an application or select a supervisor execution role.
    pub fn admit_native_application(
        listener: OwnedFd,
        root: File,
        credentials: Credentials,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_at(
            listener,
            root,
            credentials,
            Path::new(
                fe2o3_compiler_execution_protocol::NATIVE_APPLICATION_SUPERVISOR_SOCKET_PATH_V3,
            ),
            ListenerFilesystemPolicyV1::production(credentials),
            budget,
        )
    }

    /// Revalidates original inputs and requires the separate application path.
    /// A compiler listener, including an otherwise valid one, is not accepted.
    pub fn validate_native_application(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.revalidate(budget)?;
        if !self.listener.has_native_application_path() {
            return Err(Failure::Listener("not the native application listener"));
        }
        Ok(())
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

    /// Activates this original listener without exporting a listener alias.
    /// This is transport setup, not invocation or compiler admission. Any prior
    /// ordered-pair export permanently refuses this route, even after Drop. The
    /// existing indirect-service transfer remains bound-only and is unavailable
    /// after activation. Refusal may leave the socket listening; retire this
    /// owner on failure, never retry setup or fall back to another acceptor.
    pub fn activate_original_root_listener(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            Self::WORK,
            Self::SCRATCH,
            |_| {
                self.check()?;
                self.listener.activate_original_root()?;
                self.check()
            },
        )
    }

    /// Attempts one nonblocking accept from the root-activated original socket.
    /// No polling, retry or allocation occurs. Interruption/would-block returns
    /// None. An accepted endpoint is UNTRUSTED transport custody: authenticate
    /// its peer and every record, with separate bounded intake funding, before
    /// interpreting a request. No process or invocation authority is returned.
    /// Reserve the returned full delta before retaining the endpoint. All error
    /// paths close any endpoint accepted by this call, leaving this owner intact.
    pub fn try_accept_original_root(
        &mut self,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(OwnedFd, Storage)>> {
        budget.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            Self::WORK,
            Self::SCRATCH,
            |b| {
                b.reserve_storage(Self::CONNECTION_STORAGE)?;
                self.check()?;
                let accepted = self.listener.try_accept_original_root()?;
                self.check()?;
                Ok(accepted.map(|fd| (fd, Storage(Self::CONNECTION_STORAGE))))
            },
        )
    }

    /// Revalidates this exact root and its canonical shared lifecycle custody.
    /// Both full owners must remain prepaid. No root descriptor is exposed, and
    /// an independently valid lease from another parent cannot satisfy this join.
    ///
    /// ```
    /// use fe2o3_compiler_execution_supervisor::{ProvisionedProtectedIssuerServiceInputsV2 as Inputs,
    ///     ProtectedIssuerServiceProvisioningErrorV2 as Error};
    /// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn check(inputs: &Inputs, lease: &Lease, b: &mut Budget<'_>) -> Result<(), Error> {
    ///     inputs.validate_lifecycle(lease, b)
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_compiler_execution_supervisor::ProvisionedProtectedIssuerServiceInputsV2 as Inputs;
    /// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
    /// fn unmetered(inputs: &Inputs, lease: &Lease) { let _ = inputs.validate_lifecycle(lease); }
    /// ```
    pub fn validate_lifecycle(&self, lease: &Lease, budget: &mut Budget<'_>) -> Result<()> {
        let floor = self
            .retained_storage()
            .checked_add(lease.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            self.check()?;
            lease.revalidate_for_root(&self.root, b)?;
            self.check()
        })
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
                let root = self.clone_root()?;
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

    /// Duplicates only the exact checked directory for direct root-owned staging.
    /// Unlike the ordered pair, this works after listener activation. No listener,
    /// lifecycle obligation or process authority is transferred. Keep this owner
    /// prepaid and validate the final staged alias before releasing a child.
    pub fn try_clone_root_for_spawn(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
        budget.with_prepaid_scope(
            self.retained_storage(),
            ENTRY,
            Self::WORK,
            Self::SCRATCH,
            |b| {
                b.reserve_storage(Self::ROOT_STORAGE)?;
                self.check()?;
                let root = self.clone_root()?;
                self.check()?;
                Ok((root, Storage(Self::ROOT_STORAGE)))
            },
        )
    }

    /// Checks a final staged directory against this original retained owner.
    /// Both reservations must remain on the original account. Listening is
    /// allowed; a changed root, socket or pathname still refuses.
    pub fn validate_root_transfer(&self, root: &File, budget: &mut Budget<'_>) -> Result<()> {
        let floor = self
            .retained_storage()
            .checked_add(Self::ROOT_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, |_| {
            self.check()?;
            self.check_root(root)?;
            self.check()
        })
    }

    fn clone_root(&self) -> Result<File> {
        let root = File::from(
            rustix::io::fcntl_dupfd_cloexec(&self.root, 0).map_err(|errno| Failure::Io {
                operation: "clone protected issuer deployment root",
                source: errno.into(),
            })?,
        );
        self.check_root(&root)?;
        Ok(root)
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
    /// Shared lifecycle custody does not match the retained service root.
    Lifecycle(LifecycleLeaseErrorV2),
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
impl From<LifecycleLeaseErrorV2> for Failure {
    fn from(error: LifecycleLeaseErrorV2) -> Self {
        Self::Lifecycle(error)
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
            Self::Lifecycle(error) => error.fmt(f),
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
            Self::Lifecycle(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "provisioning_v2_tests.rs"]
mod tests;
