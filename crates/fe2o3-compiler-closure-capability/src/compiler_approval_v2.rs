//! Fixed-root approval of compiler policy, distinct from execution of that policy.
use crate::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionClientProfileCapabilityV3 as ProfileCapability, trusted_profile_tree as tree,
};
use fe2o3_build_authority::{
    COMPILER_APPROVAL_POLICY_BYTES_V2 as POLICY_BYTES, CompilerApprovalPolicyErrorV2,
    CompilerApprovalPolicyV2, CompilerClosureV2,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3 as PROFILE_BYTES,
    CompilerExecutionClientProfileV3 as Profile,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use rustix::fs::{IFlags, Mode};
use std::{fmt, fs::File, mem::size_of};

const POLICY_PATH: [&str; 4] = ["etc", "fe2o3", "build-authority", "policy-v2"];
const PROFILE_PATH: [&str; 4] = ["etc", "fe2o3", "compiler-execution", "client-profile-v3"];
// At most 256 fixed descriptor operations, including traversal, xattrs, stable
// reads, origin rechecks and closes. Nested codecs/capabilities charge separately.
const IO_WORK: usize = 256 * 1024;
const FRAME: usize = 16 * 1024;
// Exact profile digest plus fixed helper/service credential reads and comparisons.
const BIND_WORK: usize = 64;
type Result<T> = std::result::Result<T, CompilerApprovalErrorV2>;
type ImmutableCheck = fn(&File) -> Result<()>;

/// Approval read exclusively from the fixed root-owned policy and V3 profile.
///
/// This owner approves a configuration, not an observed compiler execution.
/// A complete runtime guard, exact invocation and current execution receipt are
/// still required before trusting compiler-nominated proof keys. No public byte,
/// file, inherited-descriptor or caller-selected path constructor exists.
/// Helper credentials must be separate from both V3 profile services. This does
/// not establish host-root deployment provenance or outside cleanup custody.
/// Retain the returned charge on the original budget and work ledger.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ApprovedCompilerPolicyV2;
/// fn duplicate(value: ApprovedCompilerPolicyV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ApprovedCompilerPolicyV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ApprovedCompilerPolicyV2>();
/// ```
pub struct ApprovedCompilerPolicyV2 {
    policy: CompilerApprovalPolicyV2,
    profile: ProfileCapability,
    policy_file: File,
    profile_file: File,
    policy_snapshot: tree::TrustedFileSnapshot,
    profile_snapshot: tree::TrustedFileSnapshot,
    ledger: Ledger,
    address: usize,
}

/// Complete unreserved charge, including the retained profile and origin files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerApprovalStorageV2(usize);
impl CompilerApprovalStorageV2 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Bounded policy-origin, framing or accounting failure; never execution evidence.
#[derive(Debug)]
pub enum CompilerApprovalErrorV2 {
    Resource(Resource),
    Capability(CapabilityError),
    Codec(CompilerApprovalPolicyErrorV2<Resource>),
    Mismatch(&'static str),
}
impl From<Resource> for CompilerApprovalErrorV2 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<CapabilityError> for CompilerApprovalErrorV2 {
    fn from(error: CapabilityError) -> Self {
        Self::Capability(error)
    }
}
impl From<tree::TrustedProfileError> for CompilerApprovalErrorV2 {
    fn from(error: tree::TrustedProfileError) -> Self {
        Self::Capability(error.into())
    }
}
impl fmt::Display for CompilerApprovalErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fixed compiler-policy approval: {self:?}")
    }
}
impl std::error::Error for CompilerApprovalErrorV2 {}

impl ApprovedCompilerPolicyV2 {
    const HEADER: usize = size_of::<Self>() - size_of::<ProfileCapability>()
        + size_of::<CompilerApprovalStorageV2>()
        + POLICY_BYTES
        + PROFILE_BYTES;

    /// Reads only `/etc/fe2o3/build-authority/policy-v2` and the fixed V3 profile.
    /// Root-owned directories forbid special/group/other-write modes and xattrs.
    /// Files are exact-length, single-link, mode0444, without ACLs/capabilities;
    /// the approval policy additionally requires FS_IMMUTABLE, as its launcher does.
    /// This is a bounded observation, not exclusion of privileged policy writers.
    pub fn from_production_policy(
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CompilerApprovalStorageV2)> {
        Self::load_using(open_root, 0, 0, require_immutable, budget)
    }

    pub const fn policy(&self) -> &CompilerApprovalPolicyV2 {
        &self.policy
    }

    /// Borrowed inert transport configuration; copying it does not copy approval.
    pub const fn profile(&self) -> &ProfileCapability {
        &self.profile
    }

    pub const fn required_retained_storage(&self) -> usize {
        Self::HEADER + self.profile.retained_storage()
    }

    /// Reobserves the fixed paths and original objects, rejecting replacement,
    /// content/metadata drift, profile rotation, missing immutability or bad seals.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.revalidate_using(open_root, 0, 0, require_immutable, budget)
    }

    /// Matches every approved compiler pin after fresh policy-origin validation.
    /// Equality here does not establish that these images executed.
    pub fn require_compiler(
        &self,
        closure: CompilerClosureV2,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.revalidate(budget)?;
        self.match_compiler(closure, budget)
    }

    fn match_compiler(&self, closure: CompilerClosureV2, budget: &mut Budget<'_>) -> Result<()> {
        self.check_account(budget)?;
        budget.charge_work(512)?;
        if self.policy.compiler_closure() != closure {
            return Err(CompilerApprovalErrorV2::Mismatch(
                "compiler differs from root-approved closure",
            ));
        }
        Ok(())
    }

    fn check_account(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(8)?;
        if budget.work_ledger_identity_v1() != self.ledger
            || budget as *const Budget<'_> as usize != self.address
            || budget.storage() < self.required_retained_storage()
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    // Only the fixed production entry points and private unit fixtures choose
    // these dependencies. No alternate root or immutability probe is exported.
    fn load_using(
        root: impl FnOnce() -> Result<File>,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CompilerApprovalStorageV2)> {
        budget.with_prepaid_scope(0, 8, IO_WORK, FRAME, |b| {
            let root = root()?;
            let (policy_bytes, policy_file, policy_snapshot) =
                read_fixed::<POLICY_BYTES>(&root, &POLICY_PATH, uid, gid, Some(immutable))?;
            let (profile_bytes, profile_file, profile_snapshot) =
                read_fixed::<PROFILE_BYTES>(&root, &PROFILE_PATH, uid, gid, None)?;
            let policy = CompilerApprovalPolicyV2::decode(&policy_bytes, |w| b.charge_work(w))
                .map_err(CompilerApprovalErrorV2::Codec)?;
            let (profile, storage) =
                Profile::decode(&profile_bytes, b).map_err(CapabilityError::from)?;
            b.reserve_storage(storage.additional_storage())?;
            let (profile, storage) = ProfileCapability::create(profile, b)?;
            b.reserve_storage(storage.additional_storage())?;
            b.charge_work(BIND_WORK)?;
            require_profile_binding(&policy, profile.profile())?;
            check_current(
                &root,
                &POLICY_PATH,
                uid,
                gid,
                POLICY_BYTES,
                policy_snapshot,
                Some(immutable),
            )?;
            check_current(
                &root,
                &PROFILE_PATH,
                uid,
                gid,
                PROFILE_BYTES,
                profile_snapshot,
                None,
            )?;
            b.reserve_storage(Self::HEADER)?;
            let owner = Self {
                policy,
                profile,
                policy_file,
                profile_file,
                policy_snapshot,
                profile_snapshot,
                ledger: b.work_ledger_identity_v1(),
                address: b as *const Budget<'_> as usize,
            };
            let storage = CompilerApprovalStorageV2(owner.required_retained_storage());
            Ok((owner, storage))
        })
    }

    fn revalidate_using(
        &self,
        root: impl FnOnce() -> Result<File>,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.check_account(budget)?;
        budget.with_prepaid_scope(self.required_retained_storage(), 8, IO_WORK, FRAME, |b| {
            self.profile.revalidate(b)?;
            let policy = tree::validate_file(&self.policy_file, uid, gid, POLICY_BYTES)?;
            immutable(&self.policy_file)?;
            let profile = tree::validate_file(&self.profile_file, uid, gid, PROFILE_BYTES)?;
            if policy != self.policy_snapshot || profile != self.profile_snapshot {
                return Err(CompilerApprovalErrorV2::Mismatch(
                    "retained policy origin changed",
                ));
            }
            let root = root()?;
            let (policy_bytes, _, policy) =
                read_fixed::<POLICY_BYTES>(&root, &POLICY_PATH, uid, gid, Some(immutable))?;
            let (profile_bytes, _, profile) =
                read_fixed::<PROFILE_BYTES>(&root, &PROFILE_PATH, uid, gid, None)?;
            b.charge_work(POLICY_BYTES + PROFILE_BYTES)?;
            if policy != self.policy_snapshot
                || profile != self.profile_snapshot
                || &policy_bytes != self.policy.canonical_bytes()
                || &profile_bytes != self.profile.profile().canonical_bytes()
            {
                return Err(CompilerApprovalErrorV2::Mismatch(
                    "fixed policy or profile path changed",
                ));
            }
            Ok(())
        })
    }
}

fn require_profile_binding(policy: &CompilerApprovalPolicyV2, profile: &Profile) -> Result<()> {
    if policy.client_profile_identity() != profile.identity().as_bytes() {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "root policy names a different V3 client profile",
        ));
    }
    let anchor = profile.external_anchor_service();
    if policy.proof_helper_uid() == profile.supervisor_uid()
        || policy.proof_helper_uid() == anchor.uid()
    {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "proof helper UID aliases a V3 profile service",
        ));
    }
    if policy.proof_helper_gid() == profile.supervisor_gid()
        || policy.proof_helper_gid() == anchor.gid()
    {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "proof helper GID aliases a V3 profile service",
        ));
    }
    Ok(())
}

fn io(operation: &'static str, error: rustix::io::Errno) -> CompilerApprovalErrorV2 {
    CapabilityError::io(operation, error).into()
}
fn open_root() -> Result<File> {
    rustix::fs::open("/", tree::DIRECTORY_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|e| io("open compiler approval root", e))
}
fn require_immutable(file: &File) -> Result<()> {
    let flags = rustix::fs::ioctl_getflags(file)
        .map_err(|e| io("inspect compiler approval immutability", e))?;
    if !flags.contains(IFlags::IMMUTABLE) {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "compiler approval policy is not immutable",
        ));
    }
    Ok(())
}
fn directory(file: &File, uid: u32, gid: u32) -> Result<()> {
    tree::validate_directory(file, uid, gid)?;
    let stat = rustix::fs::fstat(file).map_err(|e| io("inspect compiler approval directory", e))?;
    if stat.st_nlink < 2 || stat.st_mode & 0o7022 != 0 || stat.st_mode & 0o500 != 0o500 {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "compiler approval directory violates launcher policy",
        ));
    }
    Ok(())
}
fn open_fixed(root: &File, path: &[&str; 4], uid: u32, gid: u32) -> Result<File> {
    directory(root, uid, gid)?;
    let mut parent = rustix::io::fcntl_dupfd_cloexec(root, 0)
        .map(File::from)
        .map_err(|e| io("retain compiler approval root", e))?;
    for component in &path[..3] {
        parent = rustix::fs::openat(&parent, *component, tree::DIRECTORY_FLAGS, Mode::empty())
            .map(File::from)
            .map_err(|e| io("open compiler approval directory", e))?;
        directory(&parent, uid, gid)?;
    }
    rustix::fs::openat(&parent, path[3], tree::FILE_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|e| io("open fixed compiler approval file", e))
}
fn read_fixed<const N: usize>(
    root: &File,
    path: &[&str; 4],
    uid: u32,
    gid: u32,
    immutable: Option<ImmutableCheck>,
) -> Result<([u8; N], File, tree::TrustedFileSnapshot)> {
    let file = open_fixed(root, path, uid, gid)?;
    let before = tree::validate_file(&file, uid, gid, N)?;
    if let Some(check) = immutable {
        check(&file)?;
    }
    let mut bytes = [0; N];
    let read = rustix::io::pread(&file, bytes.as_mut_slice(), 0)
        .map_err(|e| io("read compiler approval file", e))?;
    if read != N {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "short compiler approval read",
        ));
    }
    let after = tree::validate_file(&file, uid, gid, N)?;
    if let Some(check) = immutable {
        check(&file)?;
    }
    if before != after {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "compiler approval file changed during read",
        ));
    }
    Ok((bytes, file, after))
}
fn check_current(
    root: &File,
    path: &[&str; 4],
    uid: u32,
    gid: u32,
    bytes: usize,
    expected: tree::TrustedFileSnapshot,
    immutable: Option<ImmutableCheck>,
) -> Result<()> {
    let file = open_fixed(root, path, uid, gid)?;
    let actual = tree::validate_file(&file, uid, gid, bytes)?;
    if let Some(check) = immutable {
        check(&file)?;
    }
    if actual != expected {
        return Err(CompilerApprovalErrorV2::Mismatch(
            "compiler approval path changed during admission",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_approval_v2_tests.rs"]
mod tests;
