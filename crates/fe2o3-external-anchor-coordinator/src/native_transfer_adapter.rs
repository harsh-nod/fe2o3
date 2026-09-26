//! Closed native supervisor transfers from retained managed custody only.

/// Full unreserved charge for a separately owned native supervisor transfer.
/// Unlike launch storage this is NOT growth above a consumed preparation owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorSupervisorTransferStorageV2(pub(crate) usize);
impl ExternalAnchorSupervisorTransferStorageV2 {
    /// Reserve immediately before retention; retire only after drop or transfer.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Complete logical work and additional peak above all borrowed input charges.
/// Includes nested native continuity and exact-object checks, not elapsed time,
/// generated stack, socket queues, kernel allocations or process RSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorSupervisorTransferQuotaV2 {
    pub(crate) work: usize,
    pub(crate) scratch: usize,
}
impl ExternalAnchorSupervisorTransferQuotaV2 {
    /// Complete successful-path work on the original ledger.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Conservative additional peak, including simultaneously live output owners.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

macro_rules! transfer {
    ($Managed:ident, $Transfer:ident, $version:literal, $other:literal) => {
        use crate::native_transfer_adapter::{
            ExternalAnchorSupervisorTransferQuotaV2 as TransferQuota,
            ExternalAnchorSupervisorTransferStorageV2 as TransferStorage,
        };
        use std::os::fd::{BorrowedFd, OwnedFd};

        // Fixed scalar/identity moves and up to two close-only error cleanups.
        // All continuity, cloning and final-pair syscalls are prepaid separately.
        const TRANSFER_LOCAL_WORK: usize = ENTRY_WORK + 8 * 1024 + 256;
        const TRANSFER_FRAME: usize = 4 * size_of::<($Transfer, TransferStorage)>()
            + 8 * size_of::<LaunchError>() + 1024;

        fn supervisor_transfer_scope<T>(owner: usize, supervisor: &SupervisorCap,
            policy: &PolicyCap, pair: usize, b: &mut Budget<'_>,
            operation: impl FnOnce(&mut Budget<'_>) -> LaunchResult<T>) -> LaunchResult<T> {
            b.charge_work(ENTRY_WORK)?;
            let floor = launch::sum(&[owner, supervisor.retained_storage(),
                policy.retained_storage(), pair])?;
            b.with_prepaid_scope(floor, 0, TRANSFER_LOCAL_WORK - ENTRY_WORK,
                TRANSFER_FRAME, operation)
        }

        fn supervisor_transfer_quota_for(continuity: LaunchQuota, clone_pair: bool)
            -> LaunchResult<TransferQuota> {
            let validation = TransferQuota {
                work: launch::sum(&[TRANSFER_LOCAL_WORK, continuity.work(),
                    Admission::VALIDATE_TRANSFER_WORK])?,
                scratch: launch::sum(&[TRANSFER_FRAME,
                    maximum(&[continuity.scratch(), Admission::IO_STORAGE])])?,
            };
            if !clone_pair { return Ok(validation); }
            Ok(TransferQuota {
                work: launch::sum(&[TRANSFER_LOCAL_WORK, continuity.work(),
                    Admission::CLONE_TRANSFER_WORK, validation.work()])?,
                scratch: launch::sum(&[TRANSFER_FRAME, maximum(&[
                    continuity.scratch(), Admission::IO_STORAGE,
                    launch::sum(&[$Transfer::STORAGE, validation.scratch()])?])])?,
            })
        }

        impl $Managed {
            /// Complete clone, continuity and final-pair validation allowance.
            /// Prepay this owner's full charge AND both actual context capabilities.
            pub fn supervisor_transfer_quota(&self) -> LaunchResult<TransferQuota> {
                supervisor_transfer_quota_for(self.continuity_quota()?, true)
            }

            /// Complete final-pair validation allowance. In addition to this owner
            /// and both contexts, prepay the candidate pair's full PAIR_STORAGE.
            pub fn supervisor_transfer_validation_quota(&self) -> LaunchResult<TransferQuota> {
                supervisor_transfer_quota_for(self.continuity_quota()?, false)
            }

            /// Clones a supervisor transfer from this live, context-bound occurrence.
            /// Both actual same-family capabilities are mandatory. The result is a
            /// point-in-time transfer, not an independently admitted pair or perpetual
            /// liveness. Retain this Managed owner and its actual contexts through final
            /// staged validation and the receiving supervisor's lifetime.
            ///
            /// Keep all borrowed-owner reservations live. Entry storage is restored
            /// on every exit; work, peak and first denials remain on the original ledger.
            /// Reserve the returned FULL charge immediately before retaining the result.
            ///
            /// ```
            #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Managed), " as Managed, ", stringify!($Transfer), " as Transfer, ExternalAnchorLaunchErrorV2 as Error, ExternalAnchorSupervisorTransferStorageV2 as Storage};")]
            #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Supervisor};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn transfer(m: &Managed, s: &Supervisor, p: &Policy, b: &mut Budget<'_>)
            ///     -> Result<(Transfer, Storage), Error> { m.try_clone_for_supervisor(s, p, b) }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
            #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $other, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Supervisor};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn wrong(m: &Managed, s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
            ///     let _ = m.try_clone_for_supervisor(s, p, b);
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
            #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $other, " as Supervisor};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn wrong(m: &Managed, s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
            ///     let _ = m.try_clone_for_supervisor(s, p, b);
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// fn missing(m: &Managed, b: &mut Budget<'_>) { let _ = m.try_clone_for_supervisor(b); }
            /// ```
            pub fn try_clone_for_supervisor(&self, supervisor: &SupervisorCap,
                policy: &PolicyCap, b: &mut Budget<'_>) -> LaunchResult<($Transfer, TransferStorage)> {
                supervisor_transfer_scope(self.retained, supervisor, policy, 0, b, |b| {
                    self.validate_continuity(supervisor, policy, b)?;
                    let (endpoint, pidfd, charge) = self.admission.try_clone_for_transfer(b)?;
                    b.reserve_storage(charge.additional_storage())?;
                    if charge.additional_storage() != $Transfer::PAIR_STORAGE {
                        return Err(Resource::Accounting.into());
                    }
                    b.reserve_storage($Transfer::STORAGE - $Transfer::PAIR_STORAGE)?;
                    self.validate_supervisor_transfer(endpoint.as_fd(), pidfd.as_fd(),
                        supervisor, policy, b)?;
                    Ok(($Transfer {
                        endpoint, pidfd,
                        service: self.admission.service_identity(),
                        deployment: self.prepared.deployment.deployment().identity(),
                        supervisor: supervisor.deployment().identity(),
                        policy: policy.policy().identity(),
                    }, TransferStorage($Transfer::STORAGE)))
                })
            }

            /// Validates the exact final staged pair against this retained Admission
            /// AND the actual same-family supervisor/policy/preparation and live child.
            /// Requires full Managed + both context + candidate PAIR_STORAGE charges.
            /// Does not independently admit descriptors or accept identity-only context.
            /// Retain exclusive transfer custody; no concurrent flag/content mutation.
            ///
            /// ```
            #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Managed), " as Managed, ExternalAnchorLaunchErrorV2 as Error};")]
            #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Supervisor};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// use std::{fs::File, os::fd::AsFd};
            /// fn check(m: &Managed, peer: &File, pidfd: &File, s: &Supervisor,
            ///     p: &Policy, b: &mut Budget<'_>) -> Result<(), Error> {
            ///     m.validate_supervisor_transfer(peer.as_fd(), pidfd.as_fd(), s, p, b)
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// use std::os::fd::BorrowedFd;
            /// fn missing(m: &Managed, fd: BorrowedFd<'_>, b: &mut Budget<'_>) {
            ///     let _ = m.validate_supervisor_transfer(fd, fd, b);
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
            #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyIdentityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentIdentityV", $version, " as Supervisor};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// use std::os::fd::BorrowedFd;
            /// fn identities(m: &Managed, fd: BorrowedFd<'_>, s: &Supervisor,
            ///     p: &Policy, b: &mut Budget<'_>) {
            ///     let _ = m.validate_supervisor_transfer(fd, fd, s, p, b);
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Managed), " as Managed;")]
            #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSupervisorDeploymentCapabilityV", $other, " as Supervisor};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// use std::os::fd::BorrowedFd;
            /// fn wrong(m: &Managed, fd: BorrowedFd<'_>, s: &Supervisor,
            ///     p: &Policy, b: &mut Budget<'_>) {
            ///     let _ = m.validate_supervisor_transfer(fd, fd, s, p, b);
            /// }
            /// ```
            pub fn validate_supervisor_transfer(&self, endpoint: BorrowedFd<'_>,
                pidfd: BorrowedFd<'_>, supervisor: &SupervisorCap, policy: &PolicyCap,
                b: &mut Budget<'_>) -> LaunchResult<()> {
                supervisor_transfer_scope(self.retained, supervisor, policy,
                    $Transfer::PAIR_STORAGE, b, |b| {
                    self.validate_continuity(supervisor, policy, b)?;
                    Ok(self.admission.validate_transfer(endpoint, pidfd, b)?)
                })
            }
        }

        /// Move-only native endpoint/pidfd transfer, created only by Managed custody.
        /// Identities are inert routing metadata, not a substitute for final contextual
        /// validation or protected receiver admission. No independent pair constructor,
        /// V1 conversion, raw accessor, Clone or AsFd is provided. Drop only closes FDs.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Transfer), " as Transfer;")]
        /// fn clone<T: Clone>() {} clone::<Transfer>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Transfer), " as Transfer;")]
        /// fn fd<T: std::os::fd::AsFd>() {} fd::<Transfer>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Transfer), " as Transfer;")]
        /// fn raw<T: std::os::fd::FromRawFd>() {} raw::<Transfer>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Transfer), " as Transfer, ExternalAnchorSupervisorTransferV1 as Old};")]
        /// fn upgrade(old: Old) -> Transfer { old.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Transfer), " as Transfer, ExternalAnchorSupervisorTransferV", $other, " as Other};")]
        /// fn mix(value: Transfer) -> Other { value.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Transfer), " as Transfer;")]
        /// fn escape(value: Transfer) { let _ = value.endpoint; }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Transfer), " as Transfer;")]
        /// use std::os::fd::OwnedFd;
        /// fn forge(pair: (OwnedFd, OwnedFd)) -> Transfer { pair.into() }
        /// ```
        pub struct $Transfer {
            endpoint: OwnedFd,
            pidfd: OwnedFd,
            service: fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorServiceIdentityV1,
            deployment: TransferDeploymentIdentity,
            supervisor: TransferSupervisorIdentity,
            policy: TransferPolicyIdentity,
        }
        impl $Transfer {
            /// Full charge including both descriptors, nominal metadata and receipt.
            pub const STORAGE: usize = Self::PAIR_STORAGE
                + size_of::<(Self, TransferStorage)>() - 2 * size_of::<OwnedFd>();
            /// Full charge retained by the two ordered output descriptors.
            pub const PAIR_STORAGE: usize = Admission::PAIR_STORAGE;
            /// Fixed extraction work, including two close-only failure cleanups.
            pub const INTO_DESCRIPTORS_WORK: usize = ENTRY_WORK + 2 * 1024 + 256;
            /// Fixed extraction frame above this owner's full prepaid STORAGE.
            pub const INTO_DESCRIPTORS_SCRATCH: usize = TRANSFER_FRAME;

            /// Full charge to retain until drop or explicit descriptor transfer.
            pub const fn retained_storage(&self) -> usize { Self::STORAGE }
            /// Deployment-pinned credentials; not receiver or process admission.
            pub const fn service(&self)
                -> fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorServiceIdentityV1 {
                self.service
            }
            /// Native deployment identity recorded from the retained actual owner.
            pub const fn deployment_identity(&self) -> TransferDeploymentIdentity { self.deployment }
            /// Native supervisor identity recorded from the actual borrowed capability.
            pub const fn supervisor_identity(&self) -> TransferSupervisorIdentity { self.supervisor }
            /// Native policy identity recorded from the actual borrowed capability.
            pub const fn policy_identity(&self) -> TransferPolicyIdentity { self.policy }

            /// Consumes into ordered CLOEXEC (endpoint, pidfd) owners, with no I/O or
            /// fresh admission. Final staged aliases MUST still be checked using the
            /// retained Managed owner and both actual contexts before supervisor exec.
            /// Keep those owners charged throughout staging. Extraction checks only
            /// this transfer's floor; the protected receiver needs native admission.
            ///
            /// Prepay STORAGE. This restores entry storage on success/error/unwind;
            /// it NEVER automatically retires the consumed reservation. On success
            /// retire STORAGE - PAIR_STORAGE, keeping PAIR_STORAGE live until the FDs
            /// close or transfer into another charged owner. On failure both FDs close:
            /// retire the consumed full charge yourself. Work/peak/denials persist.
            ///
            /// ```
            #[doc = concat!("use fe2o3_external_anchor_coordinator::{", stringify!($Transfer), " as Transfer, ExternalAnchorLaunchErrorV2 as Error};")]
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
            /// use std::os::fd::OwnedFd;
            /// fn take(value: Transfer, b: &mut Budget<'_>) -> Result<(OwnedFd, OwnedFd), Error> {
            ///     // This example assumes the full input charge was already reserved.
            ///     let pair = match value.into_ordered_descriptors(b) {
            ///         Ok(pair) => pair,
            ///         Err(error) => {
            ///             b.release_storage(Transfer::STORAGE)?;
            ///             return Err(error);
            ///         }
            ///     };
            ///     b.release_storage(Transfer::STORAGE - Transfer::PAIR_STORAGE)?;
            ///     Ok(pair)
            /// }
            /// ```
            /// ```compile_fail
            #[doc = concat!("use fe2o3_external_anchor_coordinator::", stringify!($Transfer), " as Transfer;")]
            /// fn unmetered(value: Transfer) { let _ = value.into_ordered_descriptors(); }
            /// ```
            pub fn into_ordered_descriptors(self, b: &mut Budget<'_>)
                -> LaunchResult<(OwnedFd, OwnedFd)> {
                b.with_prepaid_scope(Self::STORAGE, ENTRY_WORK, Self::INTO_DESCRIPTORS_WORK,
                    Self::INTO_DESCRIPTORS_SCRATCH, |_| Ok((self.endpoint, self.pidfd)))
            }
        }
        impl fmt::Debug for $Transfer {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Transfer)).field("authority", &"descriptor-transfer-only")
                    .field("service", &self.service).field("deployment", &self.deployment)
                    .field("supervisor", &self.supervisor).field("policy", &self.policy)
                    .finish_non_exhaustive()
            }
        }
        const _: () = {
            assert!($Transfer::STORAGE >= $Transfer::PAIR_STORAGE);
            assert!($Transfer::PAIR_STORAGE >= size_of::<(OwnedFd, OwnedFd, TransferStorage)>());
        };
    };
}
pub(crate) use transfer;
