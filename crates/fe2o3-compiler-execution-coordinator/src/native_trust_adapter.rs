//! Closed family binding of genuine native configuration and signing-key owners.
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as CapabilityError;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::{error::Error as StdError, fmt};

pub(crate) const ENTRY_WORK: usize = 8;
// Fixed binding/metadata controls and at most three close-only refusal cleanups.
// Native transport, policy matching and key checks charge their own full quotas.
pub(crate) const LOCAL_WORK: usize = ENTRY_WORK + 3 * 1024 + 256;
pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionSupervisorTrustErrorV2>;

/// Bounded native trust refusal, without allocated diagnostics or authority claims.
#[derive(Debug)]
pub enum CompilerExecutionSupervisorTrustErrorV2 {
    /// Original work/storage ledger or checked retained-storage arithmetic refused.
    Resource(Resource),
    /// Native capability or policy-bound key validation refused.
    Capability(CapabilityError),
    /// The actual deployment does not name the complete actual native policy.
    ContextMismatch,
}
use CompilerExecutionSupervisorTrustErrorV2 as Error;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<CapabilityError> for Error {
    fn from(error: CapabilityError) -> Self {
        Self::Capability(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Capability(error) => error.fmt(f),
            Self::ContextMismatch => {
                f.write_str("supervisor deployment names another native policy")
            }
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Capability(error) => Some(error),
            Self::ContextMismatch => None,
        }
    }
}

/// Unreserved metadata growth above the three consumed full native owner charges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionSupervisorTrustStorageV2(pub(crate) usize);
impl CompilerExecutionSupervisorTrustStorageV2 {
    /// Reserve before retaining Trust; keep all three consumed reservations live.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

pub(crate) fn input_storage(deployment: usize, policy: usize, key: usize) -> Result<usize> {
    deployment
        .checked_add(policy)
        .and_then(|n| n.checked_add(key))
        .ok_or(Resource::Arithmetic.into())
}
pub(crate) const fn maximum(values: &[usize]) -> usize {
    let mut result = 0;
    let mut index = 0;
    while index < values.len() {
        if values[index] > result {
            result = values[index];
        }
        index += 1;
    }
    result
}

macro_rules! trust {
    ($Trust:ident, $version:literal, $other:literal) => {
        use crate::native_trust_adapter::{
            CompilerExecutionSupervisorTrustErrorV2 as Error,
            CompilerExecutionSupervisorTrustStorageV2 as Storage,
            ENTRY_WORK, LOCAL_WORK, Result, input_storage, maximum,
        };
        use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as CapabilityError;
        use fe2o3_kernel_ir::{CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource};
        use std::{fmt, mem::size_of};

        /// Move-only binding of genuine native deployment, policy and signing-key custody.
        /// The complete policy identity is checked, including the key's pinned generation.
        /// No provisioning provenance, root-process identity, protected occurrence or launch
        /// authority is established here. The enclosing coordinator must establish those
        /// separately; native key validation requires its actual current owner.
        /// No key/signing/extraction method is public and no V1 owner can be upgraded.
        ///
        /// Keep the full three input-owner charges prepaid on the original Budget.
        /// Operations restore entry storage on success, refusal and unwind, preserving
        /// work, peak and first-denial history. Reserve returned metadata growth before
        /// retaining Trust. Consuming failure closes all inputs but does NOT retire their
        /// reservations. After Drop retire the complete retained_storage(), not just growth.
        ///
        /// ```
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Trust), " as Trust, CompilerExecutionSupervisorTrustErrorV2 as Error, CompilerExecutionSupervisorTrustStorageV2 as Storage};")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Deployment, CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSigningKeyCapabilityV", $version, " as Key};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn bind(d: Deployment, p: Policy, k: Key, b: &mut Budget<'_>)
        ///     -> Result<(Trust, Storage), Error> { Trust::new(d, p, k, b) }
        /// fn borrow(t: &Trust) -> (&Deployment, &Policy) { (t.deployment(), t.policy()) }
        /// fn check(t: &Trust, b: &mut Budget<'_>) -> Result<(), Error> { t.revalidate(b) }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        /// fn clone<T: Clone>() {} clone::<Trust>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        /// fn fd<T: std::os::fd::AsFd>() {} fd::<Trust>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        /// fn raw<T: std::os::fd::FromRawFd>() {} raw::<Trust>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Trust), " as Trust, CompilerExecutionSupervisorTrustV1 as Old};")]
        /// fn upgrade(old: Old) -> Trust { old.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::{", stringify!($Trust), " as Trust, CompilerExecutionSupervisorTrustV", $other, " as Other};")]
        /// fn mix(other: Other) -> Trust { other.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionSupervisorDeploymentCapabilityV", $other, " as Deployment, CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSigningKeyCapabilityV", $version, " as Key};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn wrong(d: Deployment, p: Policy, k: Key, b: &mut Budget<'_>) { let _ = Trust::new(d, p, k, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Deployment, CompilerExecutionPolicyCapabilityV", $other, " as Policy, CompilerExecutionSigningKeyCapabilityV", $version, " as Key};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn wrong(d: Deployment, p: Policy, k: Key, b: &mut Budget<'_>) { let _ = Trust::new(d, p, k, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Deployment, CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSigningKeyCapabilityV", $other, " as Key};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn wrong(d: Deployment, p: Policy, k: Key, b: &mut Budget<'_>) { let _ = Trust::new(d, p, k, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        /// fn key(t: &Trust) { let _ = t.key_template(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        /// fn extract(t: Trust) { let _ = t.key_template; }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_coordinator::", stringify!($Trust), " as Trust;")]
        #[doc = concat!("use fe2o3_compiler_closure_capability::{CompilerExecutionSupervisorDeploymentCapabilityV", $version, " as Deployment, CompilerExecutionPolicyCapabilityV", $version, " as Policy, CompilerExecutionSigningKeyCapabilityV", $version, " as Key};")]
        /// fn unmetered(d: Deployment, p: Policy, k: Key) { let _ = Trust::new(d, p, k); }
        /// ```
        pub struct $Trust {
            deployment: DeploymentCap,
            policy: PolicyCap,
            key_template: Key,
            retained: usize,
        }
        impl $Trust {
            /// Metadata growth above all three consumed full native owner charges.
            pub const GROWTH_STORAGE: usize = size_of::<(Self, Storage)>()
                - size_of::<DeploymentCap>() - size_of::<PolicyCap>() - size_of::<Key>();
            /// Complete successful binding work, including metered matches_policy.
            pub const BIND_WORK: usize = LOCAL_WORK + DeploymentCap::IO_WORK
                + PolicyCap::IO_WORK + DEPLOYMENT_WORK + Key::IO_WORK;
            /// Complete successful revalidation work on the original ledger.
            pub const REVALIDATION_WORK: usize = Self::BIND_WORK;
            const FRAME_STORAGE: usize = 4 * size_of::<(Self, Storage)>()
                + 8 * size_of::<Error>() + 1024;
            // Prepay metadata construction/move staging in BOTH operations, so the
            // same complete scratch quota applies to binding and revalidation.
            const OUTER_STORAGE: usize = Self::FRAME_STORAGE + Self::GROWTH_STORAGE;
            /// Complete additional logical peak above the full input floor, including
            /// metadata growth/staging and the largest sequential native scratch frame.
            /// Not a generated-stack, allocator, kernel-page, time or RSS bound.
            pub const SCRATCH: usize = Self::OUTER_STORAGE + maximum(&[
                DeploymentCap::IO_STORAGE, PolicyCap::IO_STORAGE, DEPLOYMENT_STORAGE, Key::IO_STORAGE]);

            /// Conservative inert owner charge for startup planning. Each component's
            /// I/O envelope includes its full retained owner; no capability is created.
            pub fn maximum_retained_storage() -> Result<usize> {
                input_storage(DeploymentCap::IO_STORAGE, PolicyCap::IO_STORAGE, Key::IO_STORAGE)?
                    .checked_add(Self::GROWTH_STORAGE).ok_or(Resource::Arithmetic.into())
            }

            /// Consumes the three genuine same-family owners after a full prepaid floor.
            /// Returns only metadata growth; failed inputs close without automatic retirement.
            pub fn new(deployment: DeploymentCap, policy: PolicyCap, key_template: Key,
                b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                b.charge_work(ENTRY_WORK)?;
                let floor = input_storage(deployment.retained_storage(), policy.retained_storage(),
                    key_template.retained_storage())?;
                let retained = floor.checked_add(Self::GROWTH_STORAGE).ok_or(Resource::Arithmetic)?;
                b.with_prepaid_scope(floor, 0, LOCAL_WORK - ENTRY_WORK, Self::OUTER_STORAGE, |b| {
                    check(&deployment, &policy, &key_template, b)?;
                    Ok((Self { deployment, policy, key_template, retained }, Storage(Self::GROWTH_STORAGE)))
                })
            }

            /// Revalidates all exact sealed owners and both complete policy bindings.
            /// Requires the full retained owner charge; entry storage and history persist.
            pub fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
                b.with_prepaid_scope(self.retained, ENTRY_WORK, LOCAL_WORK, Self::OUTER_STORAGE,
                    |b| check(self.deployment(), self.policy(), self.key_template(), b))
            }

            /// Borrows the actual native deployment capability, not an identity-only proxy.
            pub const fn deployment(&self) -> &DeploymentCap { &self.deployment }
            /// Borrows the actual native policy capability for contextual admission.
            pub const fn policy(&self) -> &PolicyCap { &self.policy }
            /// Internal coordinator staging only; never a public key or signing accessor.
            pub(crate) const fn key_template(&self) -> &Key { &self.key_template }
            /// Full retained charge, including every consumed owner and metadata growth.
            pub const fn retained_storage(&self) -> usize { self.retained }
        }

        fn check(deployment: &DeploymentCap, policy: &PolicyCap, key: &Key,
            b: &mut Budget<'_>) -> Result<()> {
            deployment.revalidate(b)?;
            policy.revalidate(b)?;
            if !deployment.deployment().matches_policy(policy.policy(), b).map_err(CapabilityError::from)? {
                return Err(Error::ContextMismatch);
            }
            // Native key validation pins the entire policy identity and checks the
            // image/key binding. Do not derive or compare the public key a second time.
            key.revalidate(policy.policy(), b)?;
            Ok(())
        }
        impl fmt::Debug for $Trust {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Trust)).field("authority", &"native-trust-custody-only")
                    .field("deployment", &self.deployment.deployment().identity())
                    .field("policy", &self.policy.policy().identity()).finish_non_exhaustive()
            }
        }
        #[cfg(test)]
        mod tests {
            use super::*;
            type Trust = $Trust;
            include!("native_trust_cases_tests.rs");
        }
    };
}
pub(crate) use trust;
