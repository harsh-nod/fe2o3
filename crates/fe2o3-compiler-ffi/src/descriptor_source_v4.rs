//! Owned nominal descriptor bytes. Structural validity is not compiler authority.
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_READER_SCRATCH_STORAGE_V4, DESCRIPTOR_TABLE_VIEW_STORAGE_V4, DescriptorWireErrorV4,
    DeviceDescriptorTableV4,
};
use sha2::Sha256;
use std::{error::Error, fmt, mem::size_of};

/// Distinct ELF section for nominal V4 descriptors; V1 is not reinterpreted.
pub const COMPILER_DESCRIPTOR_SECTION_NAME_V4: &str = ".fe2o3.kd.v4";
/// SHA-256 preimage is domain, little-endian u64 byte length, then exact bytes.
pub const COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V4: &[u8] = b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V4\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerDescriptorSourceIdentityV4 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl CompilerDescriptorSourceIdentityV4 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Complete owner header plus its actual transferred Vec capacity, not an
/// allocation request or an additional-storage credit. This number is inert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerDescriptorSourceStorageV4(usize);
impl CompilerDescriptorSourceStorageV4 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only zero-digest mandatory-conditional V4 source, with no self-borrowed table.
///
/// Public construction proves canonical structure and content identity only.
/// It does not authenticate rustc, nominal source correspondence, executable
/// semantics, signatures, publication or device execution.
///
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV4;
/// fn duplicate(source: CompilerDescriptorSourceV4) { let _ = source.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV4;
/// fn forge() -> CompilerDescriptorSourceV4 { CompilerDescriptorSourceV4::default() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV4;
/// fn mutate(source: CompilerDescriptorSourceV4) { source.canonical_bytes()[0] = 0; }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV4;
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV4;
/// fn escape(source: CompilerDescriptorSourceV4) -> DeviceDescriptorTableV4<'static> {
///     source.table(usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV4;
/// fn move_while_borrowed(source: CompilerDescriptorSourceV4) {
///     let table = source.table(usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap();
///     drop(source);
///     let _ = table.kernel_count();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{CompilerDescriptorSourceV1, CompilerDescriptorSourceV4};
/// fn relabel(source: CompilerDescriptorSourceV4) -> CompilerDescriptorSourceV1 { source.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{CompilerDescriptorSourceV3, CompilerDescriptorSourceV4};
/// fn downgrade(source: CompilerDescriptorSourceV4) -> CompilerDescriptorSourceV3 { source.into() }
/// ```
pub struct CompilerDescriptorSourceV4 {
    canonical_bytes: Vec<u8>,
    identity: CompilerDescriptorSourceIdentityV4,
    storage: CompilerDescriptorSourceStorageV4,
}
impl fmt::Debug for CompilerDescriptorSourceV4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerDescriptorSourceV4")
            .field("identity", &self.identity)
            .field("storage", &self.storage)
            .finish_non_exhaustive()
    }
}

pub const COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V4: usize =
    size_of::<CompilerDescriptorSourceV4>();
pub const COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V4: usize = size_of::<Sha256>()
    + size_of::<CompilerDescriptorSourceIdentityV4>()
    + size_of::<[u8; 8]>()
    + size_of::<[u8; 32]>()
    + 128;
/// Includes the returned VIEW, which stays paid after reader scratch is released.
pub const COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V4: usize =
    DESCRIPTOR_TABLE_VIEW_STORAGE_V4 + DESCRIPTOR_READER_SCRATCH_STORAGE_V4;
/// Conservative simultaneous typed extent, excluding the separately paid owner.
pub const COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V4: usize =
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V4 + COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V4;

/// Checked full-owner extent; call before transferring an allocated Vec.
pub fn compiler_descriptor_source_retained_storage_v4(
    capacity: usize,
) -> Option<CompilerDescriptorSourceStorageV4> {
    COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V4
        .checked_add(capacity)
        .map(CompilerDescriptorSourceStorageV4)
}
/// Checked minimum for construction or revalidation, including actual capacity.
pub fn compiler_descriptor_source_validation_storage_v4(capacity: usize) -> Option<usize> {
    compiler_descriptor_source_retained_storage_v4(capacity)?
        .0
        .checked_add(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V4)
}
/// Checked minimum for returning a fresh borrowed table.
pub fn compiler_descriptor_source_table_storage_v4(capacity: usize) -> Option<usize> {
    compiler_descriptor_source_retained_storage_v4(capacity)?
        .0
        .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V4)
}

#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerDescriptorSourceErrorV4<E> {
    Wire(DescriptorWireErrorV4<E>),
    Work(E),
    Storage { required: usize, prepaid: usize },
    Arithmetic,
    FinalizedDigest,
    IdentityMismatch,
}
type ResultV4<T, E> = Result<T, CompilerDescriptorSourceErrorV4<E>>;
impl<E: fmt::Display> fmt::Display for CompilerDescriptorSourceErrorV4<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "invalid V4 compiler descriptor source: {error}"),
            Self::Work(error) => write!(f, "V4 compiler descriptor source work refused: {error}"),
            Self::Storage { required, prepaid } => write!(
                f,
                "V4 compiler descriptor source requires {required} prepaid bytes, got {prepaid}"
            ),
            Self::Arithmetic => f.write_str("V4 compiler descriptor source extent overflow"),
            Self::FinalizedDigest => {
                f.write_str("V4 compiler descriptor source digest must be zero")
            }
            Self::IdentityMismatch => f.write_str("V4 compiler descriptor source identity changed"),
        }
    }
}
impl<E: Error + 'static> Error for CompilerDescriptorSourceErrorV4<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Wire(error) => Some(error),
            Self::Work(error) => Some(error),
            _ => None,
        }
    }
}

fn error<E>(
    e: crate::descriptor_source_common::SourceError<DescriptorWireErrorV4<E>, E>,
) -> CompilerDescriptorSourceErrorV4<E> {
    use crate::descriptor_source_common::SourceError;
    match e {
        SourceError::Wire(e) => CompilerDescriptorSourceErrorV4::Wire(e),
        SourceError::Work(e) => CompilerDescriptorSourceErrorV4::Work(e),
        SourceError::Storage { required, prepaid } => {
            CompilerDescriptorSourceErrorV4::Storage { required, prepaid }
        }
        SourceError::Arithmetic => CompilerDescriptorSourceErrorV4::Arithmetic,
        SourceError::FinalizedDigest => CompilerDescriptorSourceErrorV4::FinalizedDigest,
        SourceError::IdentityMismatch => CompilerDescriptorSourceErrorV4::IdentityMismatch,
    }
}
fn validate<E>(
    bytes: &[u8],
    capacity: usize,
    prepaid_storage: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<CompilerDescriptorSourceIdentityV4, E> {
    let (sha256, byte_len) =
        crate::descriptor_source_common::validate::<DeviceDescriptorTableV4<'_>, E>(
            bytes,
            compiler_descriptor_source_validation_storage_v4(capacity),
            prepaid_storage,
            charge,
        )
        .map_err(error)?;
    Ok(CompilerDescriptorSourceIdentityV4 { sha256, byte_len })
}

impl CompilerDescriptorSourceV4 {
    /// Consumes caller-allocated backing without allocating, copying or shrinking.
    /// Call the checked validation-storage helper before moving the Vec; prepay
    /// the full owner header, actual capacity and validation scratch. If the Vec
    /// header was already paid, transfer that credit rather than counting it twice.
    /// Failure drops the transferred Vec; the caller's enclosing scope owns refunds.
    ///
    /// `prepaid_storage` is only an inert numeric declaration, not authenticated
    /// reservations. Callers retain and pay all additional owners/siblings and
    /// callback state separately. The unchanged wire bound constrains byte length;
    /// outer owning protocols must impose their aggregate storage/capacity bounds.
    pub fn from_owned_canonical_bytes<E>(
        bytes: Vec<u8>,
        prepaid_storage: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<Self, E> {
        let storage = compiler_descriptor_source_retained_storage_v4(bytes.capacity())
            .ok_or(CompilerDescriptorSourceErrorV4::Arithmetic)?;
        let identity = validate(&bytes, bytes.capacity(), prepaid_storage, charge)?;
        Ok(Self {
            canonical_bytes: bytes,
            identity,
            storage,
        })
    }
    /// Fresh paid structural/zero-digest decode borrowing this exact owner.
    /// Prepay full owner + VIEW + READER. After return, keep VIEW paid until the
    /// table drops; every query also needs QUERY plus all retained cursors/results.
    /// Concurrent traversals require separate scratch. No view can outlive self.
    pub fn table<E>(
        &self,
        prepaid_storage: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<DeviceDescriptorTableV4<'_>, E> {
        crate::descriptor_source_common::table::<DeviceDescriptorTableV4<'_>, E>(
            &self.canonical_bytes,
            compiler_descriptor_source_table_storage_v4(self.canonical_bytes.capacity()),
            prepaid_storage,
            charge,
        )
        .map_err(error)
    }
    /// Fresh complete decode/hash, not cached semantic or compiler authority.
    /// Requires the same full prepaid extent as construction; allocates nothing.
    pub fn revalidate<E>(
        &self,
        prepaid_storage: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<(), E> {
        crate::descriptor_source_common::revalidate::<DeviceDescriptorTableV4<'_>, E>(
            &self.canonical_bytes,
            compiler_descriptor_source_validation_storage_v4(self.canonical_bytes.capacity()),
            prepaid_storage,
            (&self.identity.sha256, self.identity.byte_len),
            size_of::<CompilerDescriptorSourceIdentityV4>() + 1,
            charge,
        )
        .map_err(error)
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Cached identity, an O(1) copy with no new hash or source authentication.
    pub const fn identity(&self) -> CompilerDescriptorSourceIdentityV4 {
        self.identity
    }
    pub const fn storage(&self) -> CompilerDescriptorSourceStorageV4 {
        self.storage
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_link_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "descriptor_source_v4_tests.rs"]
mod tests;
