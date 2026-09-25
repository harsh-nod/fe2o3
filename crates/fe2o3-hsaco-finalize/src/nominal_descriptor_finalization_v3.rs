//! Nominal descriptor integrity in the existing detached-section artifact format.
//!
//! V3 wire is retained, never converted to a V1 table. The caller pays descriptor
//! traversal scratch and work; the inherited ELF/AMDHSA inspector has its own
//! bounded allocations. These APIs do not claim whole-process resource metering.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V3;
use fe2o3_hsaco::InspectedKernelBindings;
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3, CanonicalCodeObjectDigest, DESCRIPTOR_QUERY_STORAGE_V3,
    DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3, DescriptorWireErrorV3,
    DeviceDescriptorTableV3, decode_device_descriptor_table_v3,
};
use sha2::Sha256;

use crate::{
    DescriptorSectionLocation, FinalizationError,
    nominal_descriptor_common::{self as common, Failure, Format, Inspection},
    nominal_descriptor_physical::cross_check,
};

/// Simultaneous descriptor view, reader/query results and hash scratch. Does not
/// include caller bytes, callback state, the returned owner, or ELF/AMDHSA allocations.
/// This number is a storage declaration, not an authenticated reservation.
/// Includes the shared inspection/format headers and physical projection/launch
/// copies. It is a logical typed extent, not compiler stack or whole-process usage.
pub const NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3: usize = DESCRIPTOR_TABLE_VIEW_STORAGE_V3
    + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
    + DESCRIPTOR_QUERY_STORAGE_V3
    + size_of::<Sha256>()
    + size_of::<NominalDescriptorInspectionV3<'static>>()
    + common::COMMON_STORAGE
    + crate::nominal_descriptor_physical::PHYSICAL_PROJECTION_STORAGE
    + 256;

#[derive(Debug)]
pub enum NominalFinalizationErrorV3<E> {
    Artifact(FinalizationError),
    Wire(DescriptorWireErrorV3<E>),
    Work(E),
    Scratch { required: usize, prepaid: usize },
    DescriptorSourceMismatch,
}
impl<E: fmt::Display> fmt::Display for NominalFinalizationErrorV3<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Artifact(FinalizationError::MissingDescriptorSection) => {
                f.write_str("ELF section .fe2o3.kd.v3 is missing")
            }
            Self::Artifact(FinalizationError::DuplicateDescriptorSection) => {
                f.write_str("ELF contains multiple .fe2o3.kd.v3 sections")
            }
            Self::Artifact(e) => write!(f, "nominal HSACO inspection failed: {e}"),
            Self::Wire(e) => write!(f, "nominal descriptor rejected: {e}"),
            Self::Work(e) => write!(f, "nominal descriptor work refused: {e}"),
            Self::Scratch { required, prepaid } => write!(
                f,
                "nominal descriptor needs {required} scratch bytes, got {prepaid}"
            ),
            Self::DescriptorSourceMismatch => f.write_str(
                "embedded nominal descriptor differs from exact zero-digest compiler source",
            ),
        }
    }
}
impl<E: Error + 'static> Error for NominalFinalizationErrorV3<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Artifact(e) => Some(e),
            Self::Wire(e) => Some(e),
            Self::Work(e) => Some(e),
            _ => None,
        }
    }
}
impl<E> From<FinalizationError> for NominalFinalizationErrorV3<E> {
    fn from(e: FinalizationError) -> Self {
        Self::Artifact(e)
    }
}
impl<E> From<DescriptorWireErrorV3<E>> for NominalFinalizationErrorV3<E> {
    fn from(e: DescriptorWireErrorV3<E>) -> Self {
        Self::Wire(e)
    }
}
impl<E> Failure<E> for NominalFinalizationErrorV3<E> {
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
pub(crate) type ResultV3<T, E> = Result<T, NominalFinalizationErrorV3<E>>;

/// Borrowed exact nominal wire plus independently inspected physical bindings.
/// Neither this view nor a matching digest authenticates compiler origin.
pub struct NominalDescriptorInspectionV3<'a> {
    bindings: InspectedKernelBindings,
    table: DeviceDescriptorTableV3<'a>,
    location: DescriptorSectionLocation,
}
impl fmt::Debug for NominalDescriptorInspectionV3<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NominalDescriptorInspectionV3")
            .field("location", &self.location)
            .field("bindings", &self.bindings)
            .finish_non_exhaustive()
    }
}
impl<'a> NominalDescriptorInspectionV3<'a> {
    pub fn descriptor_table(&self) -> &DeviceDescriptorTableV3<'a> {
        &self.table
    }
    pub fn kernel_bindings(&self) -> &InspectedKernelBindings {
        &self.bindings
    }
    /// Transfers the independently inspected physical metadata without cloning it.
    /// The returned metadata remains inert, as it was while borrowed here.
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

/// Move-only finalized artifact; the descriptor lives in these bytes, not in an
/// independently mutable or nominally flattened table. This is not a launch token.
/// Its byte copy and parsed metadata are a separate bounded allocation domain,
/// not part of descriptor scratch or a transfer of the caller's input storage.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::FinalizedNominalHsacoV3;
/// fn clone(value: FinalizedNominalHsacoV3) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::FinalizedNominalHsacoV3;
/// fn mutate(value: FinalizedNominalHsacoV3) { value.as_bytes()[0] = 1; }
/// ```
#[derive(Debug)]
pub struct FinalizedNominalHsacoV3 {
    bytes: Vec<u8>,
    bindings: InspectedKernelBindings,
    location: DescriptorSectionLocation,
    digest: CanonicalCodeObjectDigest,
}
impl FinalizedNominalHsacoV3 {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Actual byte-buffer capacity only, excluding owner/header and parsed metadata.
    /// This observation is not a complete storage receipt or an allocation credit.
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
) -> ResultV3<NominalDescriptorInspectionV3<'a>, E> {
    let (table, inspected) = common::inspect(
        bytes,
        finalized,
        prepaid_scratch,
        Format {
            section_name: COMPILER_DESCRIPTOR_SECTION_NAME_V3,
            digest_offset: CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3,
            scratch: NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
        },
        charge,
        |wire, bindings, charge| {
            let table = decode_device_descriptor_table_v3(wire, charge)?;
            cross_check(bindings, &table, charge)?;
            let digest = table.canonical_code_object_digest();
            Ok::<_, NominalFinalizationErrorV3<E>>((table, digest))
        },
    )?;
    Ok(NominalDescriptorInspectionV3 {
        bindings: inspected.bindings,
        table,
        location: inspected.location,
    })
}

/// Inspect exactly one detached `.fe2o3.kd.v3`; reject duplicates and every other
/// schema in the reserved `.fe2o3.kd.*` namespace, including unknown future names.
/// COV6 requires a declared explicit-size + 256 ABI, with physical metadata and
/// hardware descriptors agreeing on either explicit-only or complete hidden-tail form.
pub fn inspect_unfinalized_nominal_hsaco_v3<'a, E>(
    bytes: &'a [u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<NominalDescriptorInspectionV3<'a>, E> {
    inspect(bytes, false, prepaid_scratch, charge)
}

/// Independently verify physical metadata, nominal wire and whole-artifact digest.
pub fn inspect_finalized_nominal_hsaco_v3<'a, E>(
    bytes: &'a [u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<NominalDescriptorInspectionV3<'a>, E> {
    inspect(bytes, true, prepaid_scratch, charge)
}

/// Require exact zero-digest source bytes, patch only the digest, then independently
/// inspect the result. Public source bytes are an integrity claim, not authentication.
pub fn finalize_unfinalized_nominal_hsaco_v3<E>(
    bytes: &[u8],
    expected_descriptor_source: &[u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<FinalizedNominalHsacoV3, E> {
    let (output, verified) = common::finalize(
        bytes,
        expected_descriptor_source,
        charge,
        |bytes, finalized, charge| {
            Ok::<_, NominalFinalizationErrorV3<E>>(
                inspect(bytes, finalized, prepaid_scratch, charge)?.into_inspection(),
            )
        },
    )?;
    Ok(FinalizedNominalHsacoV3 {
        bytes: output,
        bindings: verified.bindings,
        location: verified.location,
        digest: verified.digest,
    })
}

/// Verify, clear only the digest, and independently validate the reconstructed raw artifact.
pub fn derive_unfinalized_nominal_hsaco_v3<E>(
    bytes: &[u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<Vec<u8>, E> {
    common::derive_unfinalized(bytes, charge, |bytes, finalized, charge| {
        Ok(inspect(bytes, finalized, prepaid_scratch, charge)?.into_inspection())
    })
}
