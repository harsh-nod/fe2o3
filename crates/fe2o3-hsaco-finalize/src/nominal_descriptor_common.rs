//! Shared byte-integrity operations. Version-specific decoders retain their tables.

use fe2o3_hsaco::{InspectedKernelBindings, MAX_HSACO_BYTES, inspect_and_bind_kernel_descriptors};
use fe2o3_kernel_descriptor::{CANONICAL_CODE_OBJECT_DOMAIN_V1, CanonicalCodeObjectDigest};
use sha2::{Digest, Sha256};

use crate::{
    DescriptorPlacementV1, DescriptorSectionLocation, FinalizationError,
    locate_versioned_descriptor_section,
};

pub(crate) trait Failure<E>: From<FinalizationError> {
    fn work(error: E) -> Self;
    fn scratch(required: usize, prepaid: usize) -> Self;
    fn source_mismatch() -> Self;
}

pub(crate) struct Format {
    pub section_name: &'static str,
    pub digest_offset: usize,
    pub scratch: usize,
}

pub(crate) struct Inspection {
    pub bindings: InspectedKernelBindings,
    pub location: DescriptorSectionLocation,
    pub digest: CanonicalCodeObjectDigest,
}

// Logical typed extents additional to the versioned codec/query declarations.
// Includes coexisting result/summary headers and copied coordinates during
// wrapping and reinspection. Heap backing and compiler stack layout are excluded.
pub(crate) const COMMON_STORAGE: usize = size_of::<Format>()
    + 2 * size_of::<Inspection>()
    + 2 * size_of::<DescriptorSectionLocation>()
    + 3 * size_of::<CanonicalCodeObjectDigest>()
    + size_of::<Vec<u8>>()
    + size_of::<crate::ElfSection>();

pub(crate) fn inspect<'a, E, F: Failure<E>, T, C: FnMut(usize) -> Result<(), E>>(
    bytes: &'a [u8],
    finalized: bool,
    prepaid_scratch: usize,
    format: Format,
    charge: &mut C,
    decode_and_check: impl FnOnce(
        &'a [u8],
        &InspectedKernelBindings,
        &mut C,
    ) -> Result<(T, CanonicalCodeObjectDigest), F>,
) -> Result<(T, Inspection), F> {
    if prepaid_scratch < format.scratch {
        return Err(F::scratch(format.scratch, prepaid_scratch));
    }
    if bytes.len() > MAX_HSACO_BYTES {
        return Err(FinalizationError::InputTooLarge.into());
    }
    charge(1).map_err(F::work)?;
    let bindings = inspect_and_bind_kernel_descriptors(bytes).map_err(FinalizationError::from)?;
    let section = locate_versioned_descriptor_section(
        bytes,
        DescriptorPlacementV1::Detached,
        format.section_name,
    )?;
    let (table, declared) = decode_and_check(&bytes[section.range.clone()], &bindings, charge)?;
    let location = DescriptorSectionLocation {
        offset: section.range.start,
        size: section.range.len(),
        digest_offset: section.range.start + format.digest_offset,
    };
    charge(32).map_err(F::work)?;
    if finalized {
        if declared.as_bytes() == &[0; 32] {
            return Err(FinalizationError::ExpectedFinalizedDigest.into());
        }
        let calculated = normalized_digest::<E, F>(bytes, location, charge)?;
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
    Ok((
        table,
        Inspection {
            bindings,
            location,
            digest: declared,
        },
    ))
}

fn normalized_digest<E, F: Failure<E>>(
    bytes: &[u8],
    location: DescriptorSectionLocation,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<CanonicalCodeObjectDigest, F> {
    charge(bytes.len() + CANONICAL_CODE_OBJECT_DOMAIN_V1.len() + 8 + 128).map_err(F::work)?;
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

fn copy_bytes<E, F: Failure<E>>(
    bytes: &[u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<u8>, F> {
    charge(bytes.len()).map_err(F::work)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.len())
        .map_err(|_| FinalizationError::AllocationFailed)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

pub(crate) fn finalize<E, F: Failure<E>, C: FnMut(usize) -> Result<(), E>>(
    bytes: &[u8],
    expected_descriptor_source: &[u8],
    charge: &mut C,
    mut inspect: impl FnMut(&[u8], bool, &mut C) -> Result<Inspection, F>,
) -> Result<(Vec<u8>, Inspection), F> {
    let location = {
        let raw = inspect(bytes, false, charge)?;
        charge(raw.location.size).map_err(F::work)?;
        if &bytes[raw.location.offset..raw.location.offset + raw.location.size]
            != expected_descriptor_source
        {
            return Err(F::source_mismatch());
        }
        raw.location
    };
    let digest = normalized_digest::<E, F>(bytes, location, charge)?;
    let mut output = copy_bytes::<E, F>(bytes, charge)?;
    output[location.digest_offset..location.digest_offset + 32].copy_from_slice(digest.as_bytes());
    charge(bytes.len()).map_err(F::work)?;
    if output[..location.digest_offset] != bytes[..location.digest_offset]
        || output[location.digest_offset + 32..] != bytes[location.digest_offset + 32..]
    {
        return Err(
            FinalizationError::OutputVerification("bytes outside nominal digest changed").into(),
        );
    }
    let verified = inspect(&output, true, charge)?;
    if verified.location != location || verified.digest != digest {
        return Err(
            FinalizationError::OutputVerification("nominal digest or location changed").into(),
        );
    }
    Ok((output, verified))
}

pub(crate) fn derive_unfinalized<E, F: Failure<E>, C: FnMut(usize) -> Result<(), E>>(
    bytes: &[u8],
    charge: &mut C,
    mut inspect: impl FnMut(&[u8], bool, &mut C) -> Result<Inspection, F>,
) -> Result<Vec<u8>, F> {
    let (location, digest) = {
        let verified = inspect(bytes, true, charge)?;
        (verified.location, verified.digest)
    };
    let mut raw = copy_bytes::<E, F>(bytes, charge)?;
    raw[location.digest_offset..location.digest_offset + 32].fill(0);
    let verified = inspect(&raw, false, charge)?;
    if verified.location != location || normalized_digest::<E, F>(&raw, location, charge)? != digest
    {
        return Err(
            FinalizationError::OutputVerification("nominal raw reconstruction changed").into(),
        );
    }
    Ok(raw)
}
