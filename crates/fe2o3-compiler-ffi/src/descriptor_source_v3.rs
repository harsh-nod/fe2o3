//! Owned nominal descriptor bytes. Structural validity is not compiler authority.
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3, DescriptorWireErrorV3,
    DeviceDescriptorTableV3, decode_device_descriptor_table_v3,
};
use sha2::Sha256;
use std::{error::Error, fmt, mem::size_of};

/// Distinct ELF section for nominal V3 descriptors; V1 is not reinterpreted.
pub const COMPILER_DESCRIPTOR_SECTION_NAME_V3: &str = ".fe2o3.kd.v3";
/// SHA-256 preimage is domain, little-endian u64 byte length, then exact bytes.
pub const COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V3: &[u8] = b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V3\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerDescriptorSourceIdentityV3 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl CompilerDescriptorSourceIdentityV3 {
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
pub struct CompilerDescriptorSourceStorageV3(usize);
impl CompilerDescriptorSourceStorageV3 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only zero-digest V3 descriptor source, with no self-borrowed table.
///
/// Public construction proves canonical structure and content identity only.
/// It does not authenticate rustc, nominal source correspondence, executable
/// semantics, signatures, publication or device execution.
///
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV3;
/// fn duplicate(source: CompilerDescriptorSourceV3) { let _ = source.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV3;
/// fn forge() -> CompilerDescriptorSourceV3 { CompilerDescriptorSourceV3::default() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV3;
/// fn mutate(source: CompilerDescriptorSourceV3) { source.canonical_bytes()[0] = 0; }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV3;
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
/// fn escape(source: CompilerDescriptorSourceV3) -> DeviceDescriptorTableV3<'static> {
///     source.table(usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::CompilerDescriptorSourceV3;
/// fn move_while_borrowed(source: CompilerDescriptorSourceV3) {
///     let table = source.table(usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap();
///     drop(source);
///     let _ = table.kernel_count();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{CompilerDescriptorSourceV1, CompilerDescriptorSourceV3};
/// fn relabel(source: CompilerDescriptorSourceV3) -> CompilerDescriptorSourceV1 { source.into() }
/// ```
pub struct CompilerDescriptorSourceV3 {
    canonical_bytes: Vec<u8>,
    identity: CompilerDescriptorSourceIdentityV3,
    storage: CompilerDescriptorSourceStorageV3,
}
impl fmt::Debug for CompilerDescriptorSourceV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerDescriptorSourceV3")
            .field("identity", &self.identity)
            .field("storage", &self.storage)
            .finish_non_exhaustive()
    }
}

pub const COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3: usize =
    size_of::<CompilerDescriptorSourceV3>();
pub const COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V3: usize = size_of::<Sha256>()
    + size_of::<CompilerDescriptorSourceIdentityV3>()
    + size_of::<[u8; 8]>()
    + size_of::<[u8; 32]>()
    + 128;
/// Includes the returned VIEW, which stays paid after reader scratch is released.
pub const COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3: usize =
    DESCRIPTOR_TABLE_VIEW_STORAGE_V3 + DESCRIPTOR_READER_SCRATCH_STORAGE_V3;
/// Conservative simultaneous typed extent, excluding the separately paid owner.
pub const COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3: usize =
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3 + COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V3;

/// Checked full-owner extent; call before transferring an allocated Vec.
pub fn compiler_descriptor_source_retained_storage_v3(
    capacity: usize,
) -> Option<CompilerDescriptorSourceStorageV3> {
    COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3
        .checked_add(capacity)
        .map(CompilerDescriptorSourceStorageV3)
}
/// Checked minimum for construction or revalidation, including actual capacity.
pub fn compiler_descriptor_source_validation_storage_v3(capacity: usize) -> Option<usize> {
    compiler_descriptor_source_retained_storage_v3(capacity)?
        .0
        .checked_add(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)
}
/// Checked minimum for returning a fresh borrowed table.
pub fn compiler_descriptor_source_table_storage_v3(capacity: usize) -> Option<usize> {
    compiler_descriptor_source_retained_storage_v3(capacity)?
        .0
        .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
}

#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerDescriptorSourceErrorV3<E> {
    Wire(DescriptorWireErrorV3<E>),
    Work(E),
    Storage { required: usize, prepaid: usize },
    Arithmetic,
    FinalizedDigest,
    IdentityMismatch,
}
type ResultV3<T, E> = Result<T, CompilerDescriptorSourceErrorV3<E>>;
impl<E: fmt::Display> fmt::Display for CompilerDescriptorSourceErrorV3<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "invalid V3 compiler descriptor source: {error}"),
            Self::Work(error) => write!(f, "V3 compiler descriptor source work refused: {error}"),
            Self::Storage { required, prepaid } => write!(
                f,
                "V3 compiler descriptor source requires {required} prepaid bytes, got {prepaid}"
            ),
            Self::Arithmetic => f.write_str("V3 compiler descriptor source extent overflow"),
            Self::FinalizedDigest => {
                f.write_str("V3 compiler descriptor source digest must be zero")
            }
            Self::IdentityMismatch => f.write_str("V3 compiler descriptor source identity changed"),
        }
    }
}
impl<E: Error + 'static> Error for CompilerDescriptorSourceErrorV3<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Wire(error) => Some(error),
            Self::Work(error) => Some(error),
            _ => None,
        }
    }
}

fn prepaid<E>(required: Option<usize>, actual: usize) -> ResultV3<(), E> {
    let required = required.ok_or(CompilerDescriptorSourceErrorV3::Arithmetic)?;
    if actual < required {
        return Err(CompilerDescriptorSourceErrorV3::Storage {
            required,
            prepaid: actual,
        });
    }
    Ok(())
}
fn decode_zero<'a, E>(
    bytes: &'a [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<DeviceDescriptorTableV3<'a>, E> {
    charge(1).map_err(CompilerDescriptorSourceErrorV3::Work)?;
    let table = decode_device_descriptor_table_v3(bytes, charge)
        .map_err(CompilerDescriptorSourceErrorV3::Wire)?;
    charge(32).map_err(CompilerDescriptorSourceErrorV3::Work)?;
    if table.canonical_code_object_digest().as_bytes() != &[0; 32] {
        return Err(CompilerDescriptorSourceErrorV3::FinalizedDigest);
    }
    Ok(table)
}
fn validate<E>(
    bytes: &[u8],
    capacity: usize,
    prepaid_storage: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<CompilerDescriptorSourceIdentityV3, E> {
    prepaid(
        compiler_descriptor_source_validation_storage_v3(capacity),
        prepaid_storage,
    )?;
    {
        let _table = decode_zero(bytes, charge)?;
    }
    let (sha256, byte_len) = crate::descriptor_source_common::identity(
        COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V3,
        bytes,
        charge,
    )
    .map_err(|e| match e {
        crate::descriptor_source_common::HashError::Arithmetic => {
            CompilerDescriptorSourceErrorV3::Arithmetic
        }
        crate::descriptor_source_common::HashError::Work(e) => {
            CompilerDescriptorSourceErrorV3::Work(e)
        }
    })?;
    Ok(CompilerDescriptorSourceIdentityV3 { sha256, byte_len })
}

impl CompilerDescriptorSourceV3 {
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
    ) -> ResultV3<Self, E> {
        let storage = compiler_descriptor_source_retained_storage_v3(bytes.capacity())
            .ok_or(CompilerDescriptorSourceErrorV3::Arithmetic)?;
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
    ) -> ResultV3<DeviceDescriptorTableV3<'_>, E> {
        prepaid(
            compiler_descriptor_source_table_storage_v3(self.canonical_bytes.capacity()),
            prepaid_storage,
        )?;
        decode_zero(&self.canonical_bytes, charge)
    }
    /// Fresh complete decode/hash, not cached semantic or compiler authority.
    /// Requires the same full prepaid extent as construction; allocates nothing.
    pub fn revalidate<E>(
        &self,
        prepaid_storage: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<(), E> {
        let identity = validate(
            &self.canonical_bytes,
            self.canonical_bytes.capacity(),
            prepaid_storage,
            charge,
        )?;
        charge(size_of::<CompilerDescriptorSourceIdentityV3>() + 1)
            .map_err(CompilerDescriptorSourceErrorV3::Work)?;
        if identity != self.identity {
            return Err(CompilerDescriptorSourceErrorV3::IdentityMismatch);
        }
        Ok(())
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Cached identity, an O(1) copy with no new hash or source authentication.
    pub const fn identity(&self) -> CompilerDescriptorSourceIdentityV3 {
        self.identity
    }
    pub const fn storage(&self) -> CompilerDescriptorSourceStorageV3 {
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
#[path = "descriptor_source_v3_tests.rs"]
mod tests;
