//! Independently pinned native V3 deployment; no legacy policy promotion.
use super::{InstalledFile, InstalledTree, NamespaceCustody};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3, CompilerExecutionClientProfileErrorV3,
    CompilerExecutionClientProfileV3 as Profile, CompilerExecutionExternalAnchorDeploymentErrorV3,
    CompilerExecutionExternalAnchorDeploymentV3 as Anchor,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionSupervisorDeploymentErrorV3,
    CompilerExecutionSupervisorDeploymentV3 as Supervisor,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as StorageAccount,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use std::{error::Error, fmt, fs::File, marker::PhantomData, mem::size_of, path::Path};

const FILES: [(&str, usize); 3] = [
    (
        "client-profile-v3",
        COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3,
    ),
    (
        "supervisor-deployment-v3",
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3,
    ),
    (
        "anchor-deployment-v3",
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3,
    ),
];
type Result<T> = std::result::Result<T, ProductionCompilerExecutionDeploymentErrorV3>;

#[derive(Debug)]
pub enum ProductionCompilerExecutionDeploymentErrorV3 {
    Resource(Resource),
    Filesystem(String),
    Profile(CompilerExecutionClientProfileErrorV3),
    Supervisor(CompilerExecutionSupervisorDeploymentErrorV3),
    Anchor(CompilerExecutionExternalAnchorDeploymentErrorV3),
    Binding,
}
impl From<Resource> for ProductionCompilerExecutionDeploymentErrorV3 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for ProductionCompilerExecutionDeploymentErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "native production compiler deployment rejected: {self:?}"
        )
    }
}
impl Error for ProductionCompilerExecutionDeploymentErrorV3 {}

/// Unreserved full owner charge on the original account, including retained paths/files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCompilerExecutionDeploymentStorageV3(usize);
impl ProductionCompilerExecutionDeploymentStorageV3 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Original fixed-path native profile, supervisor, anchor and namespace custody.
///
/// Only `open` admits production provenance. Root/kernel and unremapped compatible
/// procfs are trusted, exactly as for V1. This is configuration provenance, not a
/// service-liveness lease, current-record response, proof, load or launch permit.
/// No environment override, caller capability, alternate path or V1/V2 fallback.
///
/// Admission and revalidation use the original cumulative resource account. On
/// success only local scratch is released; callers reserve the returned full
/// owner charge. Failure is terminal for that operation and retains its charges.
/// Logical quotas do not claim allocator, kernel, stack or total RSS accounting.
/// Native file and namespace-map reads are single-attempt, bounded preads with
/// an EOF check; interrupted or partial reads fail closed without hidden retries.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ProductionCompilerExecutionDeploymentV3 as Native;
/// fn copy(value: Native) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ProductionCompilerExecutionDeploymentV3 as Native;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Native>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::{ProductionCompilerExecutionDeploymentV1 as Old,
///     ProductionCompilerExecutionDeploymentV3 as Native};
/// fn promote<'work>(value: Old) -> Native<'work> { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ProductionCompilerExecutionDeploymentV3 as Native;
/// use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn escape() -> Native<'static> {
///     let mut work = Work::new(usize::MAX);
///     let mut budget = Budget::new(&mut work, 1_000_000);
///     Native::open(&mut budget).unwrap().0
/// }
/// ```
pub struct ProductionCompilerExecutionDeploymentV3<'work> {
    profile: Profile,
    supervisor: Supervisor,
    anchor: Anchor,
    tree: InstalledTree,
    namespace: NamespaceCustody,
    ledger: Ledger,
    storage_account: Option<StorageAccount>,
    work: PhantomData<&'work Work>,
}
impl fmt::Debug for ProductionCompilerExecutionDeploymentV3<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionCompilerExecutionDeploymentV3")
            .field("policy", &self.policy().identity())
            .finish_non_exhaustive()
    }
}
impl<'work> ProductionCompilerExecutionDeploymentV3<'work> {
    /// Fixed local I/O allowance; nested native decoding/matching is charged separately.
    pub const IO_WORK: usize = 64 * 1024;
    /// Bounds the fixed tree, namespace-map reads, configuration bytes and local owners.
    pub const IO_STORAGE: usize = 128 * 1024;

    pub fn open(
        budget: &mut Budget<'work>,
    ) -> Result<(Self, ProductionCompilerExecutionDeploymentStorageV3)> {
        Self::open_tree(Path::new("/"), 0, 0, budget)
    }

    fn open_tree(
        root: &Path,
        uid: u32,
        gid: u32,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, ProductionCompilerExecutionDeploymentStorageV3)> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let storage_account = budget.storage_account_identity_v1();
        budget.charge_work(Self::IO_WORK)?;
        budget.reserve_storage(Self::IO_STORAGE)?;
        if root.as_os_str().len() > 4096 {
            return Err(ProductionCompilerExecutionDeploymentErrorV3::Binding);
        }
        let namespace = NamespaceCustody::open_with_read_mode(true)
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Filesystem)?;
        let (tree, bytes) = InstalledTree::open_with_read_mode(root, uid, gid, FILES, true)
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Filesystem)?;
        let (profile, storage) = Profile::decode(&bytes[0], budget)
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Profile)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (supervisor, storage) = Supervisor::decode(&bytes[1], profile.policy(), budget)
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Supervisor)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (anchor, storage) = Anchor::decode(&bytes[2], &supervisor, profile.policy(), budget)
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Anchor)?;
        budget.reserve_storage(storage.additional_storage())?;
        let value = Self {
            profile,
            supervisor,
            anchor,
            tree,
            namespace,
            ledger,
            storage_account,
            work: PhantomData,
        };
        value.revalidate_inner(budget)?;
        let retained = value.retained_storage()?;
        if retained > Self::IO_STORAGE
            || ledger != budget.work_ledger_identity_v1()
            || storage_account != budget.storage_account_identity_v1()
        {
            return Err(Resource::Accounting.into());
        }
        drop(bytes);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        Ok((
            value,
            ProductionCompilerExecutionDeploymentStorageV3(retained),
        ))
    }

    pub const fn policy(&self) -> &Policy {
        self.profile.policy()
    }
    pub const fn profile(&self) -> &Profile {
        &self.profile
    }
    pub const fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }
    pub const fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    pub fn retained_storage(&self) -> Result<usize> {
        let mut bytes =
            size_of::<Self>() + size_of::<ProductionCompilerExecutionDeploymentStorageV3>();
        for addition in [
            self.profile
                .retained_storage()
                .checked_sub(size_of::<Profile>()),
            self.supervisor
                .retained_storage()
                .checked_sub(size_of::<Supervisor>()),
            self.anchor
                .retained_storage()
                .checked_sub(size_of::<Anchor>()),
            self.tree
                .directories
                .capacity()
                .checked_mul(size_of::<File>()),
            self.tree
                .files
                .capacity()
                .checked_mul(size_of::<InstalledFile>()),
            Some(self.tree.root_path.capacity()),
        ] {
            bytes = bytes
                .checked_add(addition.ok_or(Resource::Arithmetic)?)
                .ok_or(Resource::Arithmetic)?;
        }
        Ok(bytes)
    }

    pub fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        budget.charge_work(Self::IO_WORK)?;
        if self.ledger != budget.work_ledger_identity_v1()
            || self.storage_account != budget.storage_account_identity_v1()
            || budget.storage() < self.retained_storage()?
        {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(Self::IO_STORAGE)?;
        self.revalidate_inner(budget)?;
        budget.release_storage(Self::IO_STORAGE)?;
        Ok(())
    }

    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> Result<()> {
        self.namespace
            .revalidate()
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Filesystem)?;
        let p = &self.profile;
        let s = &self.supervisor;
        let a = &self.anchor;
        if p.supervisor_uid() != s.service_uid()
            || p.supervisor_gid() != s.service_gid()
            || p.external_anchor_service() != s.external_anchor_service()
            || !s
                .matches_policy(p.policy(), budget)
                .map_err(ProductionCompilerExecutionDeploymentErrorV3::Supervisor)?
            || !a
                .matches_supervisor_and_policy(s, p.policy(), budget)
                .map_err(ProductionCompilerExecutionDeploymentErrorV3::Anchor)?
            || self.namespace.credentials.uid == s.service_uid()
            || self.namespace.credentials.uid == a.service().uid()
        {
            return Err(ProductionCompilerExecutionDeploymentErrorV3::Binding);
        }
        self.tree
            .revalidate(&[
                p.canonical_bytes().as_slice(),
                s.canonical_bytes().as_slice(),
                a.canonical_bytes().as_slice(),
            ])
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Filesystem)?;
        self.namespace
            .revalidate()
            .map_err(ProductionCompilerExecutionDeploymentErrorV3::Filesystem)
    }
}

#[cfg(test)]
mod tests;
