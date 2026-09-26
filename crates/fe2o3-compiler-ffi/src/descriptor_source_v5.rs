//! Owned nominal descriptor bytes. Structural validity is not compiler authority.
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_READER_SCRATCH_STORAGE_V5, DESCRIPTOR_TABLE_VIEW_STORAGE_V5, DescriptorWireErrorV5,
    DeviceDescriptorTableV5,
};
use sha2::Sha256;
use std::{error::Error, fmt, mem::size_of};

/// Distinct ELF section for nominal V5 descriptors; V1 is not reinterpreted.
pub const COMPILER_DESCRIPTOR_SECTION_NAME_V5: &str = ".fe2o3.kd.v5";
/// SHA-256 preimage is domain, little-endian u64 byte length, then exact bytes.
pub const COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V5: &[u8] = b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V5\0";

/// V5 domain-separated exact source content identity, not authenticated origin.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerDescriptorSourceIdentityV5 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl CompilerDescriptorSourceIdentityV5 {
    /// Borrow the exact source digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Return the full canonical source byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Complete owner header plus its actual transferred Vec capacity, not an
/// allocation request or an additional-storage credit. This number is inert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerDescriptorSourceStorageV5(usize);
impl CompilerDescriptorSourceStorageV5 {
    /// Return the owner header plus actual transferred capacity.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only zero-digest mandatory-conditional V5 source, with no self-borrowed table.
///
/// Public construction proves canonical structure and content identity only.
/// It does not authenticate rustc, nominal source correspondence, executable
/// semantics, signatures, publication or device execution.
///
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV5;
/// fn duplicate(source: CompilerDescriptorSourceV5) { let _ = source.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV5;
/// fn forge() -> CompilerDescriptorSourceV5 { CompilerDescriptorSourceV5::default() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV5;
/// fn mutate(source: CompilerDescriptorSourceV5) { source.canonical_bytes()[0] = 0; }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV5;
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV5;
/// fn escape(source: CompilerDescriptorSourceV5) -> DeviceDescriptorTableV5<'static> {
///     source.table(usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV5;
/// fn move_while_borrowed(source: CompilerDescriptorSourceV5) {
///     let table = source.table(usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap();
///     drop(source);
///     let _ = table.kernel_count();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{CompilerDescriptorSourceV1, CompilerDescriptorSourceV5};
/// fn relabel(source: CompilerDescriptorSourceV5) -> CompilerDescriptorSourceV1 { source.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{CompilerDescriptorSourceV3, CompilerDescriptorSourceV5};
/// fn downgrade(source: CompilerDescriptorSourceV5) -> CompilerDescriptorSourceV3 { source.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{CompilerDescriptorSourceV4, CompilerDescriptorSourceV5};
/// fn downgrade(source: CompilerDescriptorSourceV5) -> CompilerDescriptorSourceV4 { source.into() }
/// ```
pub struct CompilerDescriptorSourceV5 {
    canonical_bytes: Vec<u8>,
    identity: CompilerDescriptorSourceIdentityV5,
    storage: CompilerDescriptorSourceStorageV5,
}
impl fmt::Debug for CompilerDescriptorSourceV5 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerDescriptorSourceV5")
            .field("identity", &self.identity)
            .field("storage", &self.storage)
            .finish_non_exhaustive()
    }
}

/// Complete move-only source owner header.
pub const COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V5: usize =
    size_of::<CompilerDescriptorSourceV5>();
/// Simultaneous source identity hashing scratch.
pub const COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V5: usize = size_of::<Sha256>()
    + size_of::<CompilerDescriptorSourceIdentityV5>()
    + size_of::<[u8; 8]>()
    + size_of::<[u8; 32]>()
    + 128;
/// Includes the returned VIEW, which stays paid after reader scratch is released.
pub const COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V5: usize =
    DESCRIPTOR_TABLE_VIEW_STORAGE_V5 + DESCRIPTOR_READER_SCRATCH_STORAGE_V5;
/// Conservative simultaneous typed extent, excluding the separately paid owner.
pub const COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V5: usize =
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V5 + COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V5;

/// Checked full-owner extent; call before transferring an allocated Vec.
pub fn compiler_descriptor_source_retained_storage_v5(
    capacity: usize,
) -> Option<CompilerDescriptorSourceStorageV5> {
    COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V5
        .checked_add(capacity)
        .map(CompilerDescriptorSourceStorageV5)
}
/// Checked minimum for construction or revalidation, including actual capacity.
pub fn compiler_descriptor_source_validation_storage_v5(capacity: usize) -> Option<usize> {
    compiler_descriptor_source_retained_storage_v5(capacity)?
        .0
        .checked_add(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V5)
}
/// Checked minimum for returning a fresh borrowed table.
pub fn compiler_descriptor_source_table_storage_v5(capacity: usize) -> Option<usize> {
    compiler_descriptor_source_retained_storage_v5(capacity)?
        .0
        .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V5)
}

/// Structural V5 source, resource callback or declared-storage refusal.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerDescriptorSourceErrorV5<E> {
    /// Strict outer V5/invocation V2 validation failed.
    Wire(DescriptorWireErrorV5<E>),
    /// The original caller's work callback refused.
    Work(E),
    /// Full owner/scratch storage was not declared prepaid.
    Storage {
        /// Complete required extent including actual Vec capacity.
        required: usize,
        /// Caller-declared prepaid extent.
        prepaid: usize,
    },
    /// A checked extent overflowed.
    Arithmetic,
    /// Compiler source must contain a zero code-object digest.
    FinalizedDigest,
    /// Independent revalidation disagreed with the retained content identity.
    IdentityMismatch,
}
type ResultV5<T, E> = Result<T, CompilerDescriptorSourceErrorV5<E>>;
impl<E: fmt::Display> fmt::Display for CompilerDescriptorSourceErrorV5<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "invalid V5 compiler descriptor source: {error}"),
            Self::Work(error) => write!(f, "V5 compiler descriptor source work refused: {error}"),
            Self::Storage { required, prepaid } => write!(
                f,
                "V5 compiler descriptor source requires {required} prepaid bytes, got {prepaid}"
            ),
            Self::Arithmetic => f.write_str("V5 compiler descriptor source extent overflow"),
            Self::FinalizedDigest => {
                f.write_str("V5 compiler descriptor source digest must be zero")
            }
            Self::IdentityMismatch => f.write_str("V5 compiler descriptor source identity changed"),
        }
    }
}
impl<E: Error + 'static> Error for CompilerDescriptorSourceErrorV5<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Wire(error) => Some(error),
            Self::Work(error) => Some(error),
            _ => None,
        }
    }
}

fn error<E>(
    e: crate::descriptor_source_common::SourceError<DescriptorWireErrorV5<E>, E>,
) -> CompilerDescriptorSourceErrorV5<E> {
    use crate::descriptor_source_common::SourceError;
    match e {
        SourceError::Wire(e) => CompilerDescriptorSourceErrorV5::Wire(e),
        SourceError::Work(e) => CompilerDescriptorSourceErrorV5::Work(e),
        SourceError::Storage { required, prepaid } => {
            CompilerDescriptorSourceErrorV5::Storage { required, prepaid }
        }
        SourceError::Arithmetic => CompilerDescriptorSourceErrorV5::Arithmetic,
        SourceError::FinalizedDigest => CompilerDescriptorSourceErrorV5::FinalizedDigest,
        SourceError::IdentityMismatch => CompilerDescriptorSourceErrorV5::IdentityMismatch,
    }
}
fn validate<E>(
    bytes: &[u8],
    capacity: usize,
    prepaid_storage: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<CompilerDescriptorSourceIdentityV5, E> {
    let (sha256, byte_len) =
        crate::descriptor_source_common::validate::<DeviceDescriptorTableV5<'_>, E>(
            bytes,
            compiler_descriptor_source_validation_storage_v5(capacity),
            prepaid_storage,
            charge,
        )
        .map_err(error)?;
    Ok(CompilerDescriptorSourceIdentityV5 { sha256, byte_len })
}

impl CompilerDescriptorSourceV5 {
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
    ) -> ResultV5<Self, E> {
        let storage = compiler_descriptor_source_retained_storage_v5(bytes.capacity())
            .ok_or(CompilerDescriptorSourceErrorV5::Arithmetic)?;
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
    ) -> ResultV5<DeviceDescriptorTableV5<'_>, E> {
        crate::descriptor_source_common::table::<DeviceDescriptorTableV5<'_>, E>(
            &self.canonical_bytes,
            compiler_descriptor_source_table_storage_v5(self.canonical_bytes.capacity()),
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
    ) -> ResultV5<(), E> {
        crate::descriptor_source_common::revalidate::<DeviceDescriptorTableV5<'_>, E>(
            &self.canonical_bytes,
            compiler_descriptor_source_validation_storage_v5(self.canonical_bytes.capacity()),
            prepaid_storage,
            (&self.identity.sha256, self.identity.byte_len),
            size_of::<CompilerDescriptorSourceIdentityV5>() + 1,
            charge,
        )
        .map_err(error)
    }
    /// Borrow the exact immutable retained source backing.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Cached identity, an O(1) copy with no new hash or source authentication.
    pub const fn identity(&self) -> CompilerDescriptorSourceIdentityV5 {
        self.identity
    }
    /// Return the full inert owner storage declaration.
    pub const fn storage(&self) -> CompilerDescriptorSourceStorageV5 {
        self.storage
    }
    /// Always false: structural custody grants no production authority.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    /// Always false: structural custody grants no production authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }
    /// Always false: structural custody grants no production authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    /// Always false: structural custody grants no production authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "descriptor_source_v5_tests.rs"]
mod tests;
