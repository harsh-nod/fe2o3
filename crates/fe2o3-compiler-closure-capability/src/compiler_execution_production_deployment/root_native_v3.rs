//! Root-side fixed native installation custody, separate from application admission.
use super::{
    InstalledFile, InstalledTree, identity, open_directory, open_namespace, require_filesystem,
    require_full_identity_map_bounded,
};
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
const NAMESPACES: [&str; 8] = [
    "thread-self/ns/mnt",
    "thread-self/ns/user",
    "thread-self/ns/pid",
    "thread-self/ns/cgroup",
    "thread-self/ns/ipc",
    "thread-self/ns/net",
    "thread-self/ns/uts",
    "thread-self/ns/time",
];
type Result<T> = std::result::Result<T, RootProductionCompilerExecutionDeploymentErrorV3>;

#[derive(Debug)]
pub enum RootProductionCompilerExecutionDeploymentErrorV3 {
    Resource(Resource),
    Filesystem(String),
    Profile(CompilerExecutionClientProfileErrorV3),
    Supervisor(CompilerExecutionSupervisorDeploymentErrorV3),
    Anchor(CompilerExecutionExternalAnchorDeploymentErrorV3),
    Binding,
}
impl From<Resource> for RootProductionCompilerExecutionDeploymentErrorV3 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for RootProductionCompilerExecutionDeploymentErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root native compiler deployment rejected: {self:?}")
    }
}
impl Error for RootProductionCompilerExecutionDeploymentErrorV3 {}

/// Full, unreserved retained owner charge on the original account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootProductionCompilerExecutionDeploymentStorageV3(usize);
impl RootProductionCompilerExecutionDeploymentStorageV3 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Root-admitted fixed V3 installation, not a caller-created sealed capability.
///
/// Requires stable real/effective/saved/filesystem UID and GID zero, unremapped
/// UID/GID mappings and the original procfs, namespaces, root and installed tree.
/// The root/kernel are trusted; this is not protection against malicious root,
/// rollback or a service-liveness/currentness lease. No service, child, signing,
/// proof, compiler or GPU authority follows from configuration admission alone.
/// Each read has a finite syscall schedule; interruption/short read is rejection.
/// Kernel latency, allocation overhead and generated stack/RSS are not bounded.
///
/// The caller reserves the returned charge. Admission and revalidation retain
/// the same Work lifetime and optional storage account; failed operations retain
/// charges. There is no environment override, alternate public path or fallback.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Root;
/// fn copy(value: Root<'_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Root;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Root<'_>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::{RootProductionCompilerExecutionDeploymentV3 as Root,
///     ProductionCompilerExecutionDeploymentV3 as Application};
/// fn promote<'w>(value: Application<'w>) -> Root<'w> { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Root;
/// use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn escape() -> Root<'static> {
///     let mut work = Work::new(usize::MAX);
///     let mut budget = Budget::new(&mut work, 1_000_000);
///     Root::open(&mut budget).unwrap().0
/// }
/// ```
pub struct RootProductionCompilerExecutionDeploymentV3<'work> {
    profile: Profile,
    supervisor: Supervisor,
    anchor: Anchor,
    tree: InstalledTree,
    namespace: RootNamespaces,
    ledger: Ledger,
    storage_account: Option<StorageAccount>,
    work: PhantomData<&'work Work>,
}
impl fmt::Debug for RootProductionCompilerExecutionDeploymentV3<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootProductionCompilerExecutionDeploymentV3")
            .field("policy", &self.policy().identity())
            .finish_non_exhaustive()
    }
}
impl<'work> RootProductionCompilerExecutionDeploymentV3<'work> {
    pub const IO_WORK: usize = 128 * 1024;
    pub const IO_STORAGE: usize = 128 * 1024;

    pub fn open(
        budget: &mut Budget<'work>,
    ) -> Result<(Self, RootProductionCompilerExecutionDeploymentStorageV3)> {
        Self::open_tree(Path::new("/"), budget)
    }

    fn open_tree(
        root: &Path,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, RootProductionCompilerExecutionDeploymentStorageV3)> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let storage_account = budget.storage_account_identity_v1();
        budget.charge_work(Self::IO_WORK)?;
        budget.reserve_storage(Self::IO_STORAGE)?;
        if root.as_os_str().len() > 4096 {
            return Err(Resource::Accounting.into());
        }
        let namespace = RootNamespaces::open().map_err(ErrorV3::Filesystem)?;
        let (tree, bytes) = InstalledTree::open_with_read_mode(root, 0, 0, FILES, true)
            .map_err(ErrorV3::Filesystem)?;
        let (profile, storage) = Profile::decode(&bytes[0], budget).map_err(ErrorV3::Profile)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (supervisor, storage) =
            Supervisor::decode(&bytes[1], profile.policy(), budget).map_err(ErrorV3::Supervisor)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (anchor, storage) = Anchor::decode(&bytes[2], &supervisor, profile.policy(), budget)
            .map_err(ErrorV3::Anchor)?;
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
        if retained > Self::IO_STORAGE {
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
            RootProductionCompilerExecutionDeploymentStorageV3(retained),
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
            size_of::<Self>() + size_of::<RootProductionCompilerExecutionDeploymentStorageV3>();
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
            self.namespace
                .namespaces
                .capacity()
                .checked_mul(size_of::<File>()),
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
        self.namespace.revalidate().map_err(ErrorV3::Filesystem)?;
        let (p, s, a) = (&self.profile, &self.supervisor, &self.anchor);
        if p.supervisor_uid() != s.service_uid()
            || p.supervisor_gid() != s.service_gid()
            || p.external_anchor_service() != s.external_anchor_service()
            || s.service_uid() == 0
            || a.service().uid() == 0
            || !s
                .matches_policy(p.policy(), budget)
                .map_err(ErrorV3::Supervisor)?
            || !a
                .matches_supervisor_and_policy(s, p.policy(), budget)
                .map_err(ErrorV3::Anchor)?
        {
            return Err(ErrorV3::Binding);
        }
        self.tree
            .revalidate(&[
                p.canonical_bytes(),
                s.canonical_bytes(),
                a.canonical_bytes(),
            ])
            .map_err(ErrorV3::Filesystem)?;
        self.namespace.revalidate().map_err(ErrorV3::Filesystem)
    }
}
type ErrorV3 = RootProductionCompilerExecutionDeploymentErrorV3;

struct RootNamespaces {
    procfs: File,
    namespaces: Vec<File>,
}
impl RootNamespaces {
    fn open() -> std::result::Result<Self, String> {
        require_root()?;
        let procfs = open_directory(Path::new("/proc"))?;
        require_filesystem(&procfs, libc::PROC_SUPER_MAGIC as _, "procfs")?;
        let mut namespaces = Vec::with_capacity(NAMESPACES.len());
        for name in NAMESPACES {
            namespaces.push(open_namespace(&procfs, name)?);
        }
        let value = Self { procfs, namespaces };
        value.revalidate()?;
        Ok(value)
    }
    fn revalidate(&self) -> std::result::Result<(), String> {
        require_root()?;
        let current = open_directory(Path::new("/proc"))?;
        require_filesystem(&current, libc::PROC_SUPER_MAGIC as _, "procfs")?;
        if identity(&current)? != identity(&self.procfs)?
            || self.namespaces.len() != NAMESPACES.len()
        {
            return Err("root native deployment procfs changed".into());
        }
        for procfs in [&self.procfs, &current] {
            for (retained, name) in self.namespaces.iter().zip(NAMESPACES) {
                if identity(retained)? != identity(&open_namespace(procfs, name)?)? {
                    return Err("root native deployment namespace changed".into());
                }
            }
            for name in ["thread-self/uid_map", "thread-self/gid_map"] {
                require_full_identity_map_bounded(procfs, name)?;
            }
        }
        require_root()
    }
}

fn root_ids(ids: [u32; 6], fsuid: libc::c_long, fsgid: libc::c_long) -> bool {
    ids == [0; 6] && fsuid == 0 && fsgid == 0
}
fn require_root() -> std::result::Result<(), String> {
    let (mut uid, mut euid, mut suid, mut gid, mut egid, mut sgid) = (0, 0, 0, 0, 0, 0);
    // SAFETY: getres* writes initialized scalar outputs; invalid filesystem IDs only query.
    if unsafe { libc::getresuid(&mut uid, &mut euid, &mut suid) } != 0
        || unsafe { libc::getresgid(&mut gid, &mut egid, &mut sgid) } != 0
        || !root_ids(
            [uid, euid, suid, gid, egid, sgid],
            unsafe { libc::syscall(libc::SYS_setfsuid, u32::MAX) },
            unsafe { libc::syscall(libc::SYS_setfsgid, u32::MAX) },
        )
    {
        return Err("root native deployment requires exact stable root credentials".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
