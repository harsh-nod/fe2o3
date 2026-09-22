//! Nominal descriptor integrity in the existing detached-section artifact format.
//!
//! V3 wire is retained, never converted to a V1 table. The caller pays descriptor
//! traversal scratch and work; the inherited ELF/AMDHSA inspector has its own
//! bounded allocations. These APIs do not claim whole-process resource metering.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_compiler_ffi::COMPILER_DESCRIPTOR_SECTION_NAME_V3;
use fe2o3_hsaco::{InspectedKernelBindings, MAX_HSACO_BYTES, inspect_and_bind_kernel_descriptors};
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3, CANONICAL_CODE_OBJECT_DOMAIN_V1,
    CanonicalCodeObjectDigest, DESCRIPTOR_QUERY_STORAGE_V3, DESCRIPTOR_READER_SCRATCH_STORAGE_V3,
    DESCRIPTOR_TABLE_VIEW_STORAGE_V3, DescriptorWireErrorV3, DeviceDescriptorTableV3,
    decode_device_descriptor_table_v3,
};
use sha2::{Digest, Sha256};

use crate::{
    DescriptorPlacementV1, DescriptorSectionLocation, FinalizationError,
    locate_versioned_descriptor_section, nominal_descriptor_physical_v3::cross_check,
};

/// Simultaneous descriptor view, reader/query results and hash scratch. Does not
/// include caller bytes, callback state, the returned owner, or ELF/AMDHSA allocations.
/// This number is a storage declaration, not an authenticated reservation.
pub const NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3: usize = DESCRIPTOR_TABLE_VIEW_STORAGE_V3
    + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
    + DESCRIPTOR_QUERY_STORAGE_V3
    + size_of::<Sha256>()
    + size_of::<NominalDescriptorInspectionV3<'static>>()
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
    pub fn location(&self) -> DescriptorSectionLocation {
        self.location
    }
    pub fn digest(&self) -> CanonicalCodeObjectDigest {
        self.table.canonical_code_object_digest()
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
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
    if prepaid_scratch < NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3 {
        return Err(NominalFinalizationErrorV3::Scratch {
            required: NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
            prepaid: prepaid_scratch,
        });
    }
    if bytes.len() > MAX_HSACO_BYTES {
        return Err(FinalizationError::InputTooLarge.into());
    }
    charge(1).map_err(NominalFinalizationErrorV3::Work)?;
    let bindings = inspect_and_bind_kernel_descriptors(bytes).map_err(FinalizationError::from)?;
    let section = locate_versioned_descriptor_section(
        bytes,
        DescriptorPlacementV1::Detached,
        COMPILER_DESCRIPTOR_SECTION_NAME_V3,
    )?;
    let table = decode_device_descriptor_table_v3(&bytes[section.range.clone()], charge)?;
    cross_check(&bindings, &table, charge)?;
    let location = DescriptorSectionLocation {
        offset: section.range.start,
        size: section.range.len(),
        digest_offset: section.range.start + CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3,
    };
    let declared = table.canonical_code_object_digest();
    charge(32).map_err(NominalFinalizationErrorV3::Work)?;
    if finalized {
        if declared.as_bytes() == &[0; 32] {
            return Err(FinalizationError::ExpectedFinalizedDigest.into());
        }
        let calculated = normalized_digest(bytes, location, charge)?;
        if declared != calculated {
            return Err(FinalizationError::CanonicalDigestMismatch {
                declared,
                calculated,
            }
            .into());
        }
    } else if declared.as_bytes() != &[0; 32] {
        return Err(FinalizationError::ExpectedZeroDigest.into());
    }
    Ok(NominalDescriptorInspectionV3 {
        bindings,
        table,
        location,
    })
}

/// Inspect exactly one detached `.fe2o3.kd.v3`; reject V1, mixed and duplicate sections.
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

fn normalized_digest<E>(
    bytes: &[u8],
    location: DescriptorSectionLocation,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<CanonicalCodeObjectDigest, E> {
    charge(bytes.len() + CANONICAL_CODE_OBJECT_DOMAIN_V1.len() + 8 + 128)
        .map_err(NominalFinalizationErrorV3::Work)?;
    let mut hash = Sha256::new();
    hash.update(CANONICAL_CODE_OBJECT_DOMAIN_V1);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(&bytes[..location.digest_offset]);
    hash.update([0; 32]);
    hash.update(&bytes[location.digest_offset + 32..]);
    Ok(CanonicalCodeObjectDigest::from_bytes(
        hash.finalize().into(),
    ))
}

fn copy_bytes<E>(
    bytes: &[u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<Vec<u8>, E> {
    charge(bytes.len()).map_err(NominalFinalizationErrorV3::Work)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| FinalizationError::AllocationFailed)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

/// Require exact zero-digest source bytes, patch only the digest, then independently
/// inspect the result. Public source bytes are an integrity claim, not authentication.
pub fn finalize_unfinalized_nominal_hsaco_v3<E>(
    bytes: &[u8],
    expected_descriptor_source: &[u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<FinalizedNominalHsacoV3, E> {
    let location = {
        let raw = inspect_unfinalized_nominal_hsaco_v3(bytes, prepaid_scratch, charge)?;
        charge(raw.table.canonical_bytes().len()).map_err(NominalFinalizationErrorV3::Work)?;
        if raw.table.canonical_bytes() != expected_descriptor_source {
            return Err(NominalFinalizationErrorV3::DescriptorSourceMismatch);
        }
        raw.location
    };
    let digest = normalized_digest(bytes, location, charge)?;
    let mut output = copy_bytes(bytes, charge)?;
    output[location.digest_offset..location.digest_offset + 32].copy_from_slice(digest.as_bytes());
    charge(bytes.len()).map_err(NominalFinalizationErrorV3::Work)?;
    if output[..location.digest_offset] != bytes[..location.digest_offset]
        || output[location.digest_offset + 32..] != bytes[location.digest_offset + 32..]
    {
        return Err(
            FinalizationError::OutputVerification("bytes outside nominal digest changed").into(),
        );
    }
    let verified = inspect_finalized_nominal_hsaco_v3(&output, prepaid_scratch, charge)?;
    if verified.location != location || verified.digest() != digest {
        return Err(
            FinalizationError::OutputVerification("nominal digest or location changed").into(),
        );
    }
    let bindings = verified.bindings;
    Ok(FinalizedNominalHsacoV3 {
        bytes: output,
        bindings,
        location,
        digest,
    })
}

/// Verify, clear only the digest, and independently validate the reconstructed raw artifact.
pub fn derive_unfinalized_nominal_hsaco_v3<E>(
    bytes: &[u8],
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<Vec<u8>, E> {
    let (location, digest) = {
        let verified = inspect_finalized_nominal_hsaco_v3(bytes, prepaid_scratch, charge)?;
        (verified.location, verified.digest())
    };
    let mut raw = copy_bytes(bytes, charge)?;
    raw[location.digest_offset..location.digest_offset + 32].fill(0);
    let verified = inspect_unfinalized_nominal_hsaco_v3(&raw, prepaid_scratch, charge)?;
    if verified.location != location || normalized_digest(&raw, location, charge)? != digest {
        return Err(
            FinalizationError::OutputVerification("nominal raw reconstruction changed").into(),
        );
    }
    Ok(raw)
}
