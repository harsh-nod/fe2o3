//! Root-approved code inventory custody, not an observed compiler runtime guard.
use crate::{
    ApprovedCompilerPolicyV1, CompilerApprovalErrorV1,
    CompilerExecutionCapabilityErrorV2 as CapabilityError, trusted_profile_tree as tree,
};
use fe2o3_build_authority::{
    COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1 as MAX_BYTES,
    COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 as MAX_ENTRIES,
    COMPILER_RUNTIME_MANIFEST_STORAGE_V1 as CODEC_STORAGE, CompilerApprovalPolicyV1,
    CompilerClosureV2, CompilerRuntimeEntryV1, CompilerRuntimeManifestErrorV1,
    CompilerRuntimeManifestV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use rustix::fs::{FileType, IFlags, Mode, OFlags};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::File,
    mem::size_of,
    os::unix::fs::MetadataExt,
    path::{Component, Path},
};

/// The only manifest origin admitted by the production loader.
pub const COMPILER_RUNTIME_MANIFEST_PATH_V1: &str =
    "/etc/fe2o3/build-authority/compiler-runtime-manifest-v1";
/// The only code tree admitted by the production loader; no symlink traversal.
pub const COMPILER_RUNTIME_ROOT_V1: &str = "/opt/fe2o3/compiler-runtime-v1";
const IO_WORK: usize = 256 * 1024;
// Covers two stable file inspections and a maximum-depth descriptor walk.
const ENTRY_IO_WORK: usize = 256 * 1024;
const CHUNK: usize = 64 * 1024;
const FRAME: usize = 2 * CODEC_STORAGE + 2 * size_of::<Inventory>() + 2 * CHUNK + 16 * 1024;
type Result<T> = std::result::Result<T, RetainedCompilerRuntimeErrorV1>;
type ImmutableCheck = fn(&File) -> Result<()>;

/// Move-only custody of independently approved, immutable compiler code files.
///
/// The sole public positive loader consumes fixed-root compiler approval. No
/// caller-supplied bytes, FD, alternate path, PID, successful exit or callback
/// can manufacture this owner. No descriptors are exported for substitution.
/// This is inventory custody only: no ELF dependency-resolution, process launch,
/// mapping history, protected sibling association or completed-guard claim.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RetainedCompilerRuntimeV1;
/// fn duplicate(v: RetainedCompilerRuntimeV1) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RetainedCompilerRuntimeV1;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<RetainedCompilerRuntimeV1>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RetainedCompilerRuntimeV1;
/// let _ = RetainedCompilerRuntimeV1::from_bytes;
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RetainedCompilerRuntimeV1;
/// let _ = RetainedCompilerRuntimeV1::from_file;
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::RetainedCompilerRuntimeV1;
/// let _ = RetainedCompilerRuntimeV1::from_path;
/// ```
pub struct RetainedCompilerRuntimeV1 {
    approval: ApprovedCompilerPolicyV1,
    inventory: Inventory,
}

/// Unreserved increment over the consumed approval's already reserved storage.
/// On success keep that approval reservation and reserve `additional_storage()`.
/// On failure the approval is consumed/dropped, but its caller-owned reservation
/// is unchanged and must be released by the caller. Work is never refunded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedCompilerRuntimeStorageV1 {
    additional: usize,
    retained: usize,
}
impl RetainedCompilerRuntimeStorageV1 {
    /// Newly retained inventory, origin descriptors and complete file backing.
    pub const fn additional_storage(self) -> usize {
        self.additional
    }
    /// Total live owner charge, including the original compiler approval.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Bounded origin, framing, approval or resource refusal; never execution evidence.
#[derive(Debug)]
pub enum RetainedCompilerRuntimeErrorV1 {
    /// Original resource account refused the operation.
    Resource(Resource),
    /// Existing policy/profile origin revalidation refused.
    Approval(CompilerApprovalErrorV1),
    /// Existing trusted-tree descriptor checks refused.
    Capability(CapabilityError),
    /// Inert inventory framing or semantics refused.
    Codec(CompilerRuntimeManifestErrorV1<Resource>),
    /// Origin, content, protection or complete closure mismatch.
    Mismatch(&'static str),
}
impl From<Resource> for RetainedCompilerRuntimeErrorV1 {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl From<CompilerApprovalErrorV1> for RetainedCompilerRuntimeErrorV1 {
    fn from(v: CompilerApprovalErrorV1) -> Self {
        Self::Approval(v)
    }
}
impl From<tree::TrustedProfileError> for RetainedCompilerRuntimeErrorV1 {
    fn from(v: tree::TrustedProfileError) -> Self {
        Self::Capability(v.into())
    }
}
impl fmt::Display for RetainedCompilerRuntimeErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "retained compiler inventory: {self:?}")
    }
}
impl std::error::Error for RetainedCompilerRuntimeErrorV1 {}

impl RetainedCompilerRuntimeV1 {
    /// Open only the two fixed origins, after revalidating the consumed policy.
    /// All file bytes must match the independently approved manifest, with root
    /// ownership, a single link, exact closed role mode, no ACL/capability xattrs,
    /// and FS_IMMUTABLE. Root administrators/kernel remain trusted. This does not
    /// exclude process-memory writers or claim that approved files ever execute.
    pub fn from_production_runtime(
        approval: ApprovedCompilerPolicyV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, RetainedCompilerRuntimeStorageV1)> {
        approval.revalidate(budget)?;
        let inventory = Inventory::load_using(
            approval.policy(),
            approval.required_retained_storage(),
            open_root,
            0,
            0,
            require_immutable,
            budget,
        )?;
        let storage = RetainedCompilerRuntimeStorageV1 {
            additional: inventory.additional_storage(),
            retained: inventory.required_storage(),
        };
        // Keep newly retained backing charged during the final approval recheck.
        budget.with_prepaid_scope(
            approval.required_retained_storage(),
            8,
            8,
            storage.additional,
            |b| {
                approval.revalidate(b)?;
                Ok((
                    Self {
                        approval,
                        inventory,
                    },
                    storage,
                ))
            },
        )
    }
    /// Borrow inert content only, not a transportable approval or execution token.
    pub const fn manifest(&self) -> &CompilerRuntimeManifestV1 {
        &self.inventory.manifest
    }
    /// Borrow the retained profile transport without detaching its root approval.
    /// Inert profile bytes do not replace inventory revalidation or execution.
    pub const fn profile(&self) -> &crate::CompilerExecutionClientProfileCapabilityV3 {
        self.approval.profile()
    }
    /// Full retained charge including consumed approval and all code-file backing.
    pub fn required_retained_storage(&self) -> usize {
        self.inventory.required_storage()
    }
    /// Recheck the original account, approval, current fixed paths, retained inode
    /// identities, protection and all exact file bytes. No execution is observed.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.inventory.check_account(budget)?;
        self.approval.revalidate(budget)?;
        self.inventory.revalidate_using(
            self.approval.policy(),
            open_root,
            0,
            0,
            require_immutable,
            budget,
        )?;
        self.approval.revalidate(budget)?;
        Ok(())
    }
    /// Preserve the existing *full* compiler-closure comparison and fixed-root
    /// approval checks; a matching rustc/backend pair alone is insufficient.
    pub fn require_compiler(
        &self,
        closure: CompilerClosureV2,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.inventory.check_account(budget)?;
        self.approval.require_compiler(closure, budget)?;
        self.inventory.revalidate_using(
            self.approval.policy(),
            open_root,
            0,
            0,
            require_immutable,
            budget,
        )?;
        self.approval.require_compiler(closure, budget)?;
        Ok(())
    }
}

struct RetainedEntry {
    file: File,
    snapshot: Snapshot,
}
struct Inventory {
    manifest: CompilerRuntimeManifestV1,
    manifest_file: File,
    manifest_snapshot: tree::TrustedFileSnapshot,
    directory: File,
    directory_snapshot: Snapshot,
    files: [Option<RetainedEntry>; MAX_ENTRIES],
    ledger: Ledger,
    address: usize,
    approval_identity: [u8; 32],
    approval_storage: usize,
}
impl Inventory {
    fn additional_storage(&self) -> usize {
        size_of::<RetainedCompilerRuntimeV1>() - size_of::<ApprovedCompilerPolicyV1>()
            + size_of::<RetainedCompilerRuntimeStorageV1>()
            + self.manifest.canonical_bytes().len()
            + self.manifest.total_file_bytes() as usize
    }
    fn required_storage(&self) -> usize {
        self.approval_storage + self.additional_storage()
    }
    fn check_account(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(8)?;
        if budget.work_ledger_identity_v1() != self.ledger
            || budget as *const Budget<'_> as usize != self.address
            || budget.storage() < self.required_storage()
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
    // Private intake mechanics only. Tests call these on a synthetic tree and an
    // inert policy; they never construct RetainedCompilerRuntimeV1 or approval.
    fn load_using(
        policy: &CompilerApprovalPolicyV1,
        approval_storage: usize,
        root: impl FnOnce() -> Result<File>,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.with_prepaid_scope(approval_storage, 8, IO_WORK, FRAME, |b| {
            let root = root()?;
            let (manifest, manifest_file, manifest_snapshot) =
                read_manifest(&root, uid, gid, immutable, b)?;
            match_policy(&manifest, policy, b)?;
            let directory = open_fixed(&root, COMPILER_RUNTIME_ROOT_V1, true, uid, gid)?;
            let directory_snapshot = Snapshot::read(&directory)?;
            let mut inventory = Self {
                manifest,
                manifest_file,
                manifest_snapshot,
                directory,
                directory_snapshot,
                files: std::array::from_fn(|_| None),
                ledger: b.work_ledger_identity_v1(),
                address: b as *const Budget<'_> as usize,
                approval_identity: *policy.identity(),
                approval_storage,
            };
            b.reserve_storage(inventory.additional_storage())?;
            for (entry, slot) in inventory.manifest.entries().zip(&mut inventory.files) {
                charge_entry(entry, b)?;
                let file = open_relative(&inventory.directory, entry.path, false, uid, gid)?;
                let snapshot = check_code(&file, entry, uid, gid, immutable)?;
                hash_code(&file, entry)?;
                if check_code(&file, entry, uid, gid, immutable)? != snapshot {
                    return Err(mismatch("code changed during read"));
                }
                *slot = Some(RetainedEntry { file, snapshot });
            }
            inventory.check_origins(&root, uid, gid, immutable, b)?;
            Ok(inventory)
        })
    }
    fn revalidate_using(
        &self,
        policy: &CompilerApprovalPolicyV1,
        root: impl FnOnce() -> Result<File>,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.check_account(budget)?;
        budget.with_prepaid_scope(self.required_storage(), 8, IO_WORK, FRAME, |b| {
            match_policy(&self.manifest, policy, b)?;
            if policy.identity() != &self.approval_identity {
                return Err(mismatch("approved policy rotated"));
            }
            for (entry, retained) in self.manifest.entries().zip(&self.files) {
                charge_entry(entry, b)?;
                let retained = retained.as_ref().expect("retained entry");
                if check_code(&retained.file, entry, uid, gid, immutable)? != retained.snapshot {
                    return Err(mismatch("retained code origin changed"));
                }
                hash_code(&retained.file, entry)?;
                if check_code(&retained.file, entry, uid, gid, immutable)? != retained.snapshot {
                    return Err(mismatch("code changed during revalidation"));
                }
            }
            let root = root()?;
            self.check_origins(&root, uid, gid, immutable, b)
        })
    }
    fn check_origins(
        &self,
        root: &File,
        uid: u32,
        gid: u32,
        immutable: ImmutableCheck,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let len = self.manifest.canonical_bytes().len();
        if tree::validate_file(&self.manifest_file, uid, gid, len)? != self.manifest_snapshot {
            return Err(mismatch("retained manifest origin changed"));
        }
        immutable(&self.manifest_file)?;
        let (current, _, snapshot) = read_manifest(root, uid, gid, immutable, b)?;
        b.charge_work(MAX_BYTES)?;
        if snapshot != self.manifest_snapshot || current != self.manifest {
            return Err(mismatch("fixed manifest origin changed"));
        }
        directory(&self.directory, uid, gid)?;
        let current = open_fixed(root, COMPILER_RUNTIME_ROOT_V1, true, uid, gid)?;
        if Snapshot::read(&self.directory)? != self.directory_snapshot
            || Snapshot::read(&current)? != self.directory_snapshot
        {
            return Err(mismatch("runtime root changed"));
        }
        for (entry, retained) in self.manifest.entries().zip(&self.files) {
            b.charge_work(ENTRY_IO_WORK)?;
            let file = open_relative(&current, entry.path, false, uid, gid)?;
            if check_code(&file, entry, uid, gid, immutable)?
                != retained.as_ref().expect("retained entry").snapshot
            {
                return Err(mismatch("fixed code path changed"));
            }
        }
        Ok(())
    }
}
fn match_policy(
    manifest: &CompilerRuntimeManifestV1,
    policy: &CompilerApprovalPolicyV1,
    b: &mut Budget<'_>,
) -> Result<()> {
    b.charge_work(512)?;
    if manifest.identity() != policy.runtime_manifest_identity()
        || manifest.compiler_closure() != policy.compiler_closure()
    {
        return Err(mismatch(
            "inventory differs from root-approved digest or full compiler closure",
        ));
    }
    Ok(())
}
fn mismatch(message: &'static str) -> RetainedCompilerRuntimeErrorV1 {
    RetainedCompilerRuntimeErrorV1::Mismatch(message)
}
fn io(operation: &'static str, e: rustix::io::Errno) -> RetainedCompilerRuntimeErrorV1 {
    RetainedCompilerRuntimeErrorV1::Capability(CapabilityError::io(operation, e))
}
fn open_root() -> Result<File> {
    rustix::fs::open("/", tree::DIRECTORY_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|e| io("open compiler inventory root", e))
}
fn require_immutable(file: &File) -> Result<()> {
    if !rustix::fs::ioctl_getflags(file)
        .map_err(|e| io("inspect code immutability", e))?
        .contains(IFlags::IMMUTABLE)
    {
        return Err(mismatch("compiler inventory backing is not immutable"));
    }
    Ok(())
}
fn directory(file: &File, uid: u32, gid: u32) -> Result<()> {
    tree::validate_directory(file, uid, gid)?;
    let stat = rustix::fs::fstat(file).map_err(|e| io("inspect runtime directory", e))?;
    if stat.st_nlink < 2 || stat.st_mode & 0o7022 != 0 || stat.st_mode & 0o500 != 0o500 {
        return Err(mismatch(
            "compiler inventory directory violates launcher policy",
        ));
    }
    Ok(())
}
fn open_fixed(root: &File, path: &str, is_directory: bool, uid: u32, gid: u32) -> Result<File> {
    open_relative(
        root,
        path.strip_prefix('/').expect("fixed absolute path"),
        is_directory,
        uid,
        gid,
    )
}
fn open_relative(root: &File, path: &str, is_directory: bool, uid: u32, gid: u32) -> Result<File> {
    directory(root, uid, gid)?;
    let mut parent = rustix::io::fcntl_dupfd_cloexec(root, 0)
        .map(File::from)
        .map_err(|e| io("retain inventory directory", e))?;
    let mut components = Path::new(path).components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(mismatch("invalid code path component"));
        };
        let dir = components.peek().is_some() || is_directory;
        parent = rustix::fs::openat(
            &parent,
            name,
            if dir {
                tree::DIRECTORY_FLAGS
            } else {
                tree::FILE_FLAGS
            },
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|e| io("open fixed compiler inventory component", e))?;
        if dir {
            directory(&parent, uid, gid)?;
        }
    }
    Ok(parent)
}
fn read_manifest(
    root: &File,
    uid: u32,
    gid: u32,
    immutable: ImmutableCheck,
    budget: &mut Budget<'_>,
) -> Result<(CompilerRuntimeManifestV1, File, tree::TrustedFileSnapshot)> {
    let file = open_fixed(root, COMPILER_RUNTIME_MANIFEST_PATH_V1, false, uid, gid)?;
    let len = rustix::fs::fstat(&file)
        .map_err(|e| io("inspect runtime manifest length", e))?
        .st_size;
    if len <= 0 || len as u64 > MAX_BYTES as u64 {
        return Err(mismatch("runtime manifest exceeds bound"));
    }
    let len = len as usize;
    let snapshot = tree::validate_file(&file, uid, gid, len)?;
    immutable(&file)?;
    let mut bytes = [0; MAX_BYTES];
    read_exact_at(&file, &mut bytes[..len], 0)?;
    let manifest = CompilerRuntimeManifestV1::decode(&bytes[..len], |w| budget.charge_work(w))
        .map_err(RetainedCompilerRuntimeErrorV1::Codec)?;
    if tree::validate_file(&file, uid, gid, len)? != snapshot {
        return Err(mismatch("manifest changed while reading"));
    }
    immutable(&file)?;
    Ok((manifest, file, snapshot))
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}
impl Snapshot {
    fn read(file: &File) -> Result<Self> {
        let m = file.metadata().map_err(|e| {
            io(
                "inspect retained code",
                rustix::io::Errno::from_raw_os_error(e.raw_os_error().unwrap_or(libc::EIO)),
            )
        })?;
        Ok(Self {
            dev: m.dev(),
            ino: m.ino(),
            mode: m.mode(),
            uid: m.uid(),
            gid: m.gid(),
            links: m.nlink(),
            len: m.len(),
            mtime: (m.mtime(), m.mtime_nsec()),
            ctime: (m.ctime(), m.ctime_nsec()),
        })
    }
}
fn check_code(
    file: &File,
    entry: CompilerRuntimeEntryV1<'_>,
    uid: u32,
    gid: u32,
    immutable: ImmutableCheck,
) -> Result<Snapshot> {
    let flags =
        rustix::io::fcntl_getfd(file).map_err(|e| io("inspect code descriptor flags", e))?;
    let status =
        rustix::fs::fcntl_getfl(file).map_err(|e| io("inspect code descriptor status", e))?;
    let s = Snapshot::read(file)?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(s.mode) != FileType::RegularFile
        || s.uid != uid
        || s.gid != gid
        || s.links != 1
        || s.len != entry.length
        || s.mode & 0o7777 != entry.role.protected_mode()
    {
        return Err(mismatch("invalid protected code file"));
    }
    // The shared profile predicate fixes mode0444; executable roles need0555.
    // Keep its closed xattr policy, without accepting arbitrary capability data.
    for name in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = [0];
        match rustix::fs::fgetxattr(file, name, &mut byte) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(mismatch("code file has a forbidden xattr"));
            }
            Err(e) => return Err(io("inspect code file xattrs", e)),
        }
    }
    immutable(file)?;
    Ok(s)
}
fn charge_entry(entry: CompilerRuntimeEntryV1<'_>, b: &mut Budget<'_>) -> Result<()> {
    b.charge_work(ENTRY_IO_WORK + entry.length as usize * 8)?;
    Ok(())
}
fn read_exact_at(file: &File, bytes: &mut [u8], offset: u64) -> Result<()> {
    // A short positional read refuses instead of creating an attacker-controlled
    // unbounded retry loop. Regular immutable file reads normally fill the slice.
    if rustix::io::pread(file, &mut *bytes, offset)
        .map_err(|e| io("read immutable code bytes", e))?
        != bytes.len()
    {
        return Err(mismatch("short immutable code read"));
    }
    Ok(())
}
fn hash_code(file: &File, entry: CompilerRuntimeEntryV1<'_>) -> Result<()> {
    let mut bytes = [0; CHUNK];
    let mut hash = Sha256::new();
    let mut offset = 0;
    while offset < entry.length {
        let len = (entry.length - offset).min(CHUNK as u64) as usize;
        read_exact_at(file, &mut bytes[..len], offset)?;
        hash.update(&bytes[..len]);
        offset += len as u64;
    }
    if <[u8; 32]>::from(hash.finalize()) != entry.sha256 {
        return Err(mismatch("code bytes differ from approved digest"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "retained_compiler_runtime_v1_tests.rs"]
mod tests;
