//! Inert conditional V5 artifact integrity, with mandatory contracts retained.
//! This does not join owned source proof, machine evidence or Worker custody.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V5;
use fe2o3_hsaco::InspectedKernelBindings;
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V5, CanonicalCodeObjectDigest, DESCRIPTOR_QUERY_STORAGE_V5,
    DESCRIPTOR_READER_SCRATCH_STORAGE_V5, DESCRIPTOR_TABLE_VIEW_STORAGE_V5, DescriptorWireErrorV3,
    DescriptorWireErrorV5, DeviceDescriptorTableV5, decode_device_descriptor_table_v5,
};
use sha2::Sha256;

use crate::{
    DescriptorSectionLocation, FinalizationError,
    nominal_descriptor_common::{self as common, Failure, Format, Inspection},
    nominal_descriptor_physical::cross_check,
};

/// Simultaneous descriptor view, mandatory-contract reader/query and hash scratch.
/// Excludes caller bytes, callback state, returned owner and ELF/AMDHSA allocations.
/// This is an inert storage declaration, not an authenticated reservation.
/// Shared inspection/format headers and physical projection/launch copies are
/// included as logical typed extents, not compiler stack or whole-process usage.
pub const NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V5: usize = DESCRIPTOR_TABLE_VIEW_STORAGE_V5
    + DESCRIPTOR_READER_SCRATCH_STORAGE_V5
    + DESCRIPTOR_QUERY_STORAGE_V5
    + size_of::<Sha256>()
    + size_of::<NominalDescriptorInspectionV5<'static>>()
    + common::COMMON_STORAGE
    + crate::nominal_descriptor_physical::PHYSICAL_PROJECTION_STORAGE
    + 256;

#[derive(Debug)]
pub enum NominalFinalizationErrorV5<E> {
    Artifact(FinalizationError),
    Wire(DescriptorWireErrorV5<E>),
    Work(E),
    Scratch { required: usize, prepaid: usize },
    DescriptorSourceMismatch,
}
impl<E: fmt::Display> fmt::Display for NominalFinalizationErrorV5<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Artifact(FinalizationError::MissingDescriptorSection) => {
                f.write_str("ELF section .fe2o3.kd.v5 is missing")
            }
            Self::Artifact(FinalizationError::DuplicateDescriptorSection) => {
                f.write_str("ELF contains multiple .fe2o3.kd.v5 sections")
            }
            Self::Artifact(e) => write!(f, "conditional nominal HSACO inspection failed: {e}"),
            Self::Wire(e) => write!(f, "conditional nominal descriptor rejected: {e}"),
            Self::Work(e) => write!(f, "conditional nominal descriptor work refused: {e}"),
            Self::Scratch { required, prepaid } => write!(
                f, "conditional nominal descriptor needs {required} scratch bytes, got {prepaid}"
            ),
            Self::DescriptorSourceMismatch => f.write_str(
                "embedded conditional nominal descriptor differs from exact zero-digest compiler source",
            ),
        }
    }
}
impl<E: Error + 'static> Error for NominalFinalizationErrorV5<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Artifact(e) => Some(e),
            Self::Wire(e) => Some(e),
            Self::Work(e) => Some(e),
            _ => None,
        }
    }
}
impl<E> From<FinalizationError> for NominalFinalizationErrorV5<E> {
    fn from(e: FinalizationError) -> Self {
        Self::Artifact(e)
    }
}
impl<E> From<DescriptorWireErrorV5<E>> for NominalFinalizationErrorV5<E> {
    fn from(e: DescriptorWireErrorV5<E>) -> Self {
        Self::Wire(e)
    }
}
impl<E> From<DescriptorWireErrorV3<E>> for NominalFinalizationErrorV5<E> {
    fn from(e: DescriptorWireErrorV3<E>) -> Self {
        Self::Wire(e.into())
    }
}
impl<E> Failure<E> for NominalFinalizationErrorV5<E> {
    fn work(error: E) -> Self {
        Self::Work(error)
    }
    fn scratch(required: usize, prepaid: usize) -> Self {
        Self::Scratch { required, prepaid }
    }
    fn source_mismatch() -> Self {
        Self::DescriptorSourceMismatch
    }
}
type ResultV5<T, E> = Result<T, NominalFinalizationErrorV5<E>>;

/// Exact V5 wire, mandatory contracts and independently inspected physical bindings.
/// Matching public identities do not authenticate source, proof or compiler origin.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::NominalDescriptorInspectionV5;
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV4;
/// fn downgrade<'a>(view: &'a NominalDescriptorInspectionV5<'a>) -> &'a DeviceDescriptorTableV4<'a> {
///     view.descriptor_table()
/// }
/// ```
pub struct NominalDescriptorInspectionV5<'a> {
    bindings: InspectedKernelBindings,
    table: DeviceDescriptorTableV5<'a>,
    location: DescriptorSectionLocation,
}
impl fmt::Debug for NominalDescriptorInspectionV5<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NominalDescriptorInspectionV5")
            .field("location", &self.location)
            .field("bindings", &self.bindings)
            .finish_non_exhaustive()
    }
}
impl<'a> NominalDescriptorInspectionV5<'a> {
    pub fn descriptor_table(&self) -> &DeviceDescriptorTableV5<'a> {
        &self.table
    }
    pub fn kernel_bindings(&self) -> &InspectedKernelBindings {
        &self.bindings
    }
    /// Transfers physical metadata only; the returned metadata remains inert.
    pub fn into_kernel_bindings(self) -> InspectedKernelBindings {
        self.bindings
    }
    pub fn location(&self) -> DescriptorSectionLocation {
        self.location
    }
    pub fn digest(&self) -> CanonicalCodeObjectDigest {
        self.table.canonical_code_object_digest()
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    fn into_inspection(self) -> Inspection {
        Inspection {
            digest: self.digest(),
            bindings: self.bindings,
            location: self.location,
        }
    }
}

/// Move-only structural artifact with exact mandatory-conditional V5 bytes.
/// The byte copy and parsed metadata are a separate bounded allocation domain.
/// This owner is not Worker finalization, proof, admission or launch authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::FinalizedNominalHsacoV5;
/// fn clone(value: FinalizedNominalHsacoV5) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::FinalizedNominalHsacoV5;
/// fn mutate(value: FinalizedNominalHsacoV5) { value.as_bytes()[0] = 1; }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{FinalizedNominalHsacoV4, FinalizedNominalHsacoV5};
/// fn downgrade(value: FinalizedNominalHsacoV5) -> FinalizedNominalHsacoV4 { value.into() }
/// ```
#[derive(Debug)]
pub struct FinalizedNominalHsacoV5 {
    bytes: Vec<u8>,
    bindings: InspectedKernelBindings,
    location: DescriptorSectionLocation,
    digest: CanonicalCodeObjectDigest,
}
impl FinalizedNominalHsacoV5 {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Actual byte-buffer capacity only; not a storage receipt or allocation credit.
    pub fn artifact_byte_capacity(&self) -> usize {
        self.bytes.capacity()
    }
    pub fn kernel_bindings(&self) -> &InspectedKernelBindings {
        &self.bindings
    }
    pub fn location(&self) -> DescriptorSectionLocation {
        self.location
    }
    pub fn digest(&self) -> CanonicalCodeObjectDigest {
        self.digest
    }
    pub fn descriptor_bytes(&self) -> &[u8] {
        &self.bytes[self.location.offset..self.location.offset + self.location.size]
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn inspect<'a, E>(
    bytes: &'a [u8],
    finalized: bool,
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<NominalDescriptorInspectionV5<'a>, E> {
    let (table, inspected) = common::inspect(
        bytes,
        finalized,
        prepaid_scratch,
        Format {
            section_name: COMPILER_DESCRIPTOR_SECTION_NAME_V5,
            digest_offset: CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V5,
            scratch: NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V5,
        },
        charge,
        |wire, bindings, charge| {
            let table = decode_device_descriptor_table_v5(wire, charge)?;
            cross_check(bindings, &table, charge)?;
            let digest = table.canonical_code_object_digest();
            Ok::<_, NominalFinalizationErrorV5<E>>((table, digest))
        },
    )?;
    Ok(NominalDescriptorInspectionV5 {
        bindings: inspected.bindings,
        table,
        location: inspected.location,
    })
}

/// Inspect exactly one detached `.fe2o3.kd.v5`, with all mandatory contracts.
/// Reject duplicates and every other schema in `.fe2o3.kd.*`, including future
/// names, without fallback or contract erasure.
/// COV6 requires a declared explicit-size + 256 ABI; physical metadata and hardware
/// must agree on explicit-only or complete hidden-tail form, as in nominal V3.
pub fn inspect_unfinalized_nominal_hsaco_v5<'a, E>(
    bytes: &'a [u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<NominalDescriptorInspectionV5<'a>, E> {
    inspect(bytes, false, prepaid_scratch, charge)
}

/// Independently verify physical metadata, mandatory contracts and artifact digest.
pub fn inspect_finalized_nominal_hsaco_v5<'a, E>(
    bytes: &'a [u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<NominalDescriptorInspectionV5<'a>, E> {
    inspect(bytes, true, prepaid_scratch, charge)
}

/// Require exact zero-digest V5 source bytes, patch only the digest, and reinspect.
/// Caller-supplied source bytes and their hashes cannot supply owned proof or machine evidence.
pub fn finalize_unfinalized_nominal_hsaco_v5<E>(
    bytes: &[u8],
    expected_descriptor_source: &[u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<FinalizedNominalHsacoV5, E> {
    let (output, verified) = common::finalize(
        bytes,
        expected_descriptor_source,
        charge,
        |bytes, finalized, charge| {
            Ok::<_, NominalFinalizationErrorV5<E>>(
                inspect(bytes, finalized, prepaid_scratch, charge)?.into_inspection(),
            )
        },
    )?;
    Ok(FinalizedNominalHsacoV5 {
        bytes: output,
        bindings: verified.bindings,
        location: verified.location,
        digest: verified.digest,
    })
}

/// Verify, clear only the digest, and independently validate the raw V5 artifact.
pub fn derive_unfinalized_nominal_hsaco_v5<E>(
    bytes: &[u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<Vec<u8>, E> {
    common::derive_unfinalized(bytes, charge, |bytes, finalized, charge| {
        Ok(inspect(bytes, finalized, prepaid_scratch, charge)?.into_inspection())
    })
}
