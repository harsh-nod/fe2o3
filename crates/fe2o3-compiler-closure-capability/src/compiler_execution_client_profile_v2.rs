use crate::{
    native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK, NativeCapability, Record, Result,
        Storage,
    },
    sealed_image::CapabilityRole,
    trusted_profile_tree as tree,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2 as BYTES,
    CompilerExecutionClientProfileV2 as Profile,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::Mode;
use std::{fs::File, mem::size_of};

type Capability = NativeCapability<Profile, BYTES>;
const PROFILE_NAME: &str = "client-profile-v2";
impl Record<BYTES> for Profile {
    const ROLE: CapabilityRole = CapabilityRole {
        name: "native compiler-execution client-profile capability",
        memfd_name: "fe2o3-compiler-execution-client-profile-v2",
    };
    fn bytes(&self) -> &[u8; BYTES] {
        self.canonical_bytes()
    }
    fn retained_storage(&self) -> usize {
        self.retained_storage()
    }
    fn decode_retained(bytes: &[u8; BYTES], budget: &mut Budget<'_>) -> Result<Self> {
        let (profile, storage) = Self::decode(bytes, budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        Ok(profile)
    }
}

/// Sealed native trust configuration, not compiler or GPU launch authority.
/// Uses the same ownership/resource contract as PolicyCapabilityV2. Production
/// admission has one fixed root-owned path and never falls back to V1.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV2;
/// fn duplicate(value: CompilerExecutionClientProfileCapabilityV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionClientProfileCapabilityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV2;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(profile: CompilerExecutionClientProfileV1, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = CompilerExecutionClientProfileCapabilityV2::create(profile, budget);
/// }
/// ```
pub struct CompilerExecutionClientProfileCapabilityV2(Capability);
impl CompilerExecutionClientProfileCapabilityV2 {
    pub const IO_WORK: usize = Capability::IO_WORK;
    pub const IO_STORAGE: usize = Capability::IO_STORAGE;
    pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;
    /// Additional tree allowance: at most 64 descriptor syscalls at 1024 units
    /// each, including opens, metadata, ordered xattr probes, read, and closes.
    /// Actual native profile decoding additionally charges this same ledger.
    pub const PRODUCTION_WORK: usize = Capability::IO_WORK + 64 * 1024;
    pub const PRODUCTION_STORAGE: usize = Capability::IO_STORAGE
        + BYTES
        + 4 * size_of::<File>()
        + 4 * size_of::<tree::TrustedFileSnapshot>()
        + 4096;

    pub fn create(profile: Profile, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::create(profile, budget).map(|(value, storage)| (Self(value), storage))
    }
    /// Consumes a File prepaid at FILE_STORAGE and returns only owner growth.
    pub fn from_file(image: File, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::from_file(image, budget).map(|(value, storage)| (Self(value), storage))
    }
    pub const fn profile(&self) -> &Profile {
        &self.0.record
    }
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.0.revalidate(budget)
    }
    /// Returns a separately charged CLOEXEC File, preserving the source owner.
    pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
        self.0.try_clone_for_transfer(budget)
    }
    pub const fn retained_storage(&self) -> usize {
        Capability::RETAINED
    }

    /// Admits `/etc/fe2o3/compiler-execution/client-profile-v2` exclusively.
    /// Each root-owned component is opened descriptor-relatively, without
    /// symlinks; the final file is single-link, regular, exact mode0444/length.
    /// ACLs/capabilities are forbidden and the final metadata snapshot must be
    /// stable across its single positional read. This pins opened objects,
    /// not perpetual pathname currentness or privileged-writer exclusion.
    /// Returns the FULL new capability charge, with no consumed input owner.
    pub fn from_production_profile(budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        budget.with_prepaid_scope(
            0,
            ENTRY_WORK,
            Self::PRODUCTION_WORK,
            Self::PRODUCTION_STORAGE,
            |budget| {
                let root = rustix::fs::open("/", tree::DIRECTORY_FLAGS, Mode::empty())
                    .map(File::from)
                    .map_err(|e| Error::io("open production profile root", e))?;
                Ok((
                    Self(from_trusted_tree(root, 0, 0, budget)?),
                    Storage(Capability::RETAINED),
                ))
            },
        )
    }
}

// The public boundary chooses '/', root ownership, and this exact fixed path.
// Only private tests may supply a synthetic tree and fixture uid/gid.
fn from_trusted_tree(
    mut directory: File,
    uid: u32,
    gid: u32,
    budget: &mut Budget<'_>,
) -> Result<Capability> {
    tree::validate_directory(&directory, uid, gid)?;
    for component in tree::COMPONENTS {
        let next = rustix::fs::openat(&directory, component, tree::DIRECTORY_FLAGS, Mode::empty())
            .map(File::from)
            .map_err(|e| Error::io("open trusted profile directory", e))?;
        tree::validate_directory(&next, uid, gid)?;
        directory = next;
    }
    let file = rustix::fs::openat(&directory, PROFILE_NAME, tree::FILE_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|e| Error::io("open trusted native profile", e))?;
    let bytes = read_profile(&file, uid, gid, |_| {})?;
    let profile = Profile::decode_retained(&bytes, budget)?;
    Capability::create_inner(profile)
}

// Only private tests supply a nontrivial hook, to exercise read-boundary races
// deterministically. Production admits no caller callback or alternate path.
fn read_profile(
    file: &File,
    uid: u32,
    gid: u32,
    after_read: impl FnOnce(&File),
) -> Result<[u8; BYTES]> {
    let before = tree::validate_file(file, uid, gid, BYTES)?;
    let mut bytes = [0; BYTES];
    let read = rustix::io::pread(file, bytes.as_mut_slice(), 0)
        .map_err(|e| Error::io("read trusted native profile", e))?;
    if read != BYTES {
        return Err(Error::Rejected("short trusted native profile read"));
    }
    after_read(file);
    let after = tree::validate_file(file, uid, gid, BYTES)?;
    if before != after {
        return Err(Error::Rejected(
            "trusted native profile changed while reading",
        ));
    }
    Ok(bytes)
}

const _: () = {
    Capability::assert_layout::<CompilerExecutionClientProfileCapabilityV2>();
};

#[cfg(test)]
#[path = "native_profile_tests.rs"]
mod tests;
