//! Ordered, inert bindings for one expanded final subject, never a U-only route.
use crate::expanded_history_receipt_v4::{Reader, allocate, storage, work};
use crate::{
    EXPANDED_PUBLICATION_HASH_STORAGE_V4, ExpandedContentIdentityV4, ExpandedPublicationErrorV4,
    MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3, MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3,
    NativeOutputTransitionRootV1, expanded_content_identity_v4,
};
use std::{mem::size_of, ops::Range};

/// Complete association identity domain, separate from every old output transition.
pub const EXPANDED_OUTPUT_ASSOCIATION_DOMAIN_V1: &[u8] = b"FE2O3/EXPANDED-OUTPUT-ASSOCIATION/V1\0";
/// Domain of the complete original signed-source packet, interpreted later by its checker.
pub const EXPANDED_ORIGINAL_SOURCE_DOMAIN_V1: &[u8] =
    b"FE2O3/EXPANDED-ORIGINAL-SOURCE-CONTENT/V1\0";
/// Domain of the actual final contract-catalog bytes.
pub const EXPANDED_FINAL_CATALOG_DOMAIN_V1: &[u8] = b"FE2O3/EXPANDED-FINAL-CATALOG-CONTENT/V1\0";
/// Number of required, positionally typed identity axes.
pub const EXPANDED_OUTPUT_AXIS_COUNT_V1: usize = 19;
const HEADER: usize = 40;
const AXES_BYTES: usize = 40 * EXPANDED_OUTPUT_AXIS_COUNT_V1;

/// A closed source branch, not an assertion that source was authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ExpandedSourceKindV1 {
    /// Genuine direct source branch.
    Direct = 1,
    /// Genuine erased source branch.
    Erased = 2,
}
/// Required binding coordinates in normative wire order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum ExpandedOutputAxisV1 {
    /// Complete original signed-source packet.
    OriginalSource = 0,
    /// Actual original semantic MIR receipt.
    SemanticMir,
    /// Original MIR-to-neutral correspondence receipt.
    Correspondence,
    /// Original neutral graph identity.
    OriginalNeutral,
    /// Actual pre-bind N or E graph identity.
    PreBind,
    /// Actual target-bound B graph identity.
    BoundInput,
    /// Complete required expanded-history receipt.
    History,
    /// Full canonical final KIR receipt, never a compact substitute.
    FinalGraph,
    /// Complete final catalog content.
    FinalCatalog,
    /// Original guarded formal-memory roster.
    OriginalFormal,
    /// Fresh final guarded formal-memory receipt.
    FinalFormal,
    /// Exact zero-code-object-digest nominal descriptor V3 source.
    Descriptor,
    /// Exact native symbol manifest.
    SymbolManifest,
    /// Actual final LLVM module identity.
    NativeModule,
    /// Compact final ModuleV2 commitment receipt.
    FinalCommitment,
    /// Actual target-binding receipt.
    TargetBinding,
    /// Actual target data-layout receipt.
    DataLayout,
    /// Fresh actual final AMDGPU lowering receipt.
    AmdgpuLowering,
    /// Exact final FFI envelope identity.
    FfiEnvelope,
}

/// Borrowed producer inputs. Original source and catalog remain caller-owned/prepaid.
pub struct ExpandedOutputAssociationInputsV1<'a> {
    /// Closed actual source branch.
    pub source_kind: ExpandedSourceKindV1,
    /// Exact canonical production target spelling, including feature suffixes.
    pub target: &'a str,
    /// Required identity axes in the enum's fixed order.
    pub axes: [ExpandedContentIdentityV4; EXPANDED_OUTPUT_AXIS_COUNT_V1],
    /// Complete semantic-root-ordered original/final coordinate roster.
    pub roots: &'a [NativeOutputTransitionRootV1],
    /// Complete original signed-source packet, not just its digest.
    pub original_source: &'a [u8],
    /// Complete final contract catalog.
    pub final_catalog: &'a [u8],
}
/// Borrowed canonical association with fixed-size ranges, not allocated root rows.
pub struct ExpandedOutputAssociationRefV1<'a> {
    bytes: &'a [u8],
    target: &'a str,
    kind: ExpandedSourceKindV1,
    axes: [ExpandedContentIdentityV4; EXPANDED_OUTPUT_AXIS_COUNT_V1],
    roots: Range<usize>,
    source: Range<usize>,
    catalog: Range<usize>,
}
/// Full view, fixed bounded root-permutation scratch and sequential hash scratch.
pub const EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1: usize = size_of::<
    ExpandedOutputAssociationRefV1<'static>,
>() + size_of::<Reader<'static>>()
    + 3 * MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
    + size_of::<NativeOutputTransitionRootV1>()
    + EXPANDED_PUBLICATION_HASH_STORAGE_V4;

impl<'a> ExpandedOutputAssociationRefV1<'a> {
    /// Strictly validates bytes, complete ordered axes and embedded-content identities.
    /// Input backing is prepaid; the returned view stays paid within `available`.
    pub fn read<E>(
        bytes: &'a [u8],
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedPublicationErrorV4<E>> {
        storage(EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1, available)?;
        if bytes.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(ExpandedPublicationErrorV4::Format("association cap"));
        }
        work(HEADER + AXES_BYTES, charge)?;
        let mut r = Reader::new(bytes);
        if r.take::<E>(8)? != b"F2EXOA1\0" || r.u16::<E>()? != 1 {
            return Err(ExpandedPublicationErrorV4::Format("association version"));
        }
        let kind = match r.u16::<E>()? {
            1 => ExpandedSourceKindV1::Direct,
            2 => ExpandedSourceKindV1::Erased,
            _ => return Err(ExpandedPublicationErrorV4::Format("source kind")),
        };
        if r.u64::<E>()? != bytes.len() as u64 {
            return Err(ExpandedPublicationErrorV4::Format("association length"));
        }
        let count = r.u32::<E>()? as usize;
        let target_len = r.u16::<E>()? as usize;
        if r.u16::<E>()? != 6 {
            return Err(ExpandedPublicationErrorV4::Format("code-object version"));
        }
        let source_len = r.u32::<E>()? as usize;
        let catalog_len = r.u32::<E>()? as usize;
        if r.u32::<E>()? != 0
            || !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&count)
            || !(1..=128).contains(&target_len)
            || source_len == 0
            || catalog_len == 0
        {
            return Err(ExpandedPublicationErrorV4::Format("association counts"));
        }
        let first = r.identity::<E>()?;
        let mut axes = [first; EXPANDED_OUTPUT_AXIS_COUNT_V1];
        for axis in &mut axes[1..] {
            *axis = r.identity()?;
        }
        work(target_len, charge)?;
        let target = std::str::from_utf8(r.take(target_len)?)
            .map_err(|_| ExpandedPublicationErrorV4::Format("target text"))?;
        // The enclosing capsule performs the canonical DeviceTarget parser and invocation join.
        if !target.is_ascii() || target.bytes().any(|b| b == 0) {
            return Err(ExpandedPublicationErrorV4::Format("target text"));
        }
        let roots_start = r.at;
        r.take::<E>(
            count
                .checked_mul(16)
                .ok_or(ExpandedPublicationErrorV4::Overflow)?,
        )?;
        let roots = roots_start..r.at;
        let source_start = r.at;
        r.take::<E>(source_len)?;
        let source = source_start..r.at;
        let catalog_start = r.at;
        r.take::<E>(catalog_len)?;
        let catalog = catalog_start..r.at;
        r.finish()?;
        let view = Self {
            bytes,
            target,
            kind,
            axes,
            roots,
            source,
            catalog,
        };
        work(3 * MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3, charge)?;
        let mut seen = [[false; MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3]; 3];
        let mut previous = None;
        for index in 0..count {
            let row = view.root(index, charge)?;
            if previous.is_some_and(|id| id >= row.semantic_root()) {
                return Err(ExpandedPublicationErrorV4::Format("semantic root order"));
            }
            previous = Some(row.semantic_root());
            for (flags, ordinal) in seen.iter_mut().zip([
                row.descriptor_ordinal(),
                row.input_kernel_ordinal(),
                row.output_kernel_ordinal(),
            ]) {
                let ordinal = ordinal as usize;
                if ordinal >= count || flags[ordinal] {
                    return Err(ExpandedPublicationErrorV4::Format("root permutation"));
                }
                flags[ordinal] = true;
            }
        }
        let source_id = expanded_content_identity_v4(
            EXPANDED_ORIGINAL_SOURCE_DOMAIN_V1,
            view.original_source(),
            EXPANDED_PUBLICATION_HASH_STORAGE_V4,
            charge,
        )?;
        let catalog_id = expanded_content_identity_v4(
            EXPANDED_FINAL_CATALOG_DOMAIN_V1,
            view.final_catalog(),
            EXPANDED_PUBLICATION_HASH_STORAGE_V4,
            charge,
        )?;
        if source_id != view.axis(ExpandedOutputAxisV1::OriginalSource)
            || catalog_id != view.axis(ExpandedOutputAxisV1::FinalCatalog)
        {
            return Err(ExpandedPublicationErrorV4::Identity(
                "embedded association content",
            ));
        }
        if kind == ExpandedSourceKindV1::Direct
            && view.axis(ExpandedOutputAxisV1::OriginalNeutral)
                != view.axis(ExpandedOutputAxisV1::PreBind)
        {
            return Err(ExpandedPublicationErrorV4::Identity(
                "direct pre-bind input",
            ));
        }
        Ok(view)
    }
    /// Returns one fixed, inert identity axis.
    pub const fn axis(&self, axis: ExpandedOutputAxisV1) -> ExpandedContentIdentityV4 {
        self.axes[axis as usize]
    }
    /// Exact target text, independently checked by capsule admission.
    pub const fn target(&self) -> &'a str {
        self.target
    }
    /// Closed branch tag, not producer authentication.
    pub const fn source_kind(&self) -> ExpandedSourceKindV1 {
        self.kind
    }
    /// Complete number of ordered root rows.
    pub fn root_count(&self) -> usize {
        self.roots.len() / 16
    }
    /// Reads one original row in place and charges its exact bytes.
    pub fn root<E>(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<NativeOutputTransitionRootV1, ExpandedPublicationErrorV4<E>> {
        if index >= self.root_count() {
            return Err(ExpandedPublicationErrorV4::Format("root index"));
        }
        work(16, charge)?;
        let at = self.roots.start + index * 16;
        let mut r = Reader::new(&self.bytes[at..at + 16]);
        Ok(NativeOutputTransitionRootV1::new(
            r.u32()?,
            r.u32()?,
            r.u32()?,
            r.u32()?,
        ))
    }
    /// Complete original packet, still requiring its genuine source-proof checker.
    pub fn original_source(&self) -> &'a [u8] {
        &self.bytes[self.source.clone()]
    }
    /// Complete actual final catalog content, not a catalog-validity claim.
    pub fn final_catalog(&self) -> &'a [u8] {
        &self.bytes[self.catalog.clone()]
    }
    /// Complete exact association bytes.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// Exact encoded length, including complete source/catalog payloads and all roots.
pub fn expanded_output_association_length_v1(
    input: &ExpandedOutputAssociationInputsV1<'_>,
) -> Option<usize> {
    if input.roots.is_empty()
        || input.roots.len() > MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
        || input.original_source.is_empty()
        || input.final_catalog.is_empty()
        || input.target.is_empty()
        || input.target.len() > 128
    {
        return None;
    }
    let n = HEADER
        .checked_add(AXES_BYTES)?
        .checked_add(input.target.len())?
        .checked_add(input.roots.len().checked_mul(16)?)?
        .checked_add(input.original_source.len())?
        .checked_add(input.final_catalog.len())?;
    (n <= MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3).then_some(n)
}
/// Simultaneous output Vec header/capacity plus the full validation domain.
pub fn expanded_output_association_encode_storage_v1(capacity: usize) -> Option<usize> {
    size_of::<Vec<u8>>()
        .checked_add(capacity)?
        .checked_add(EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1)
}
/// Encodes all rows without sorting or erasing coordinates. Inputs are prepaid;
/// `available` covers the output capacity/header and validation until return.
pub fn encode_expanded_output_association_v1<E>(
    input: &ExpandedOutputAssociationInputsV1<'_>,
    available: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<u8>, ExpandedPublicationErrorV4<E>> {
    let n = expanded_output_association_length_v1(input)
        .ok_or(ExpandedPublicationErrorV4::Format("association extent"))?;
    storage(
        expanded_output_association_encode_storage_v1(n)
            .ok_or(ExpandedPublicationErrorV4::Overflow)?,
        available,
    )?;
    work(n, charge)?;
    let mut out = allocate(n)?;
    storage(
        expanded_output_association_encode_storage_v1(out.capacity())
            .ok_or(ExpandedPublicationErrorV4::Overflow)?,
        available,
    )?;
    out.extend_from_slice(b"F2EXOA1\0");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(input.source_kind as u16).to_le_bytes());
    out.extend_from_slice(&(n as u64).to_le_bytes());
    out.extend_from_slice(&(input.roots.len() as u32).to_le_bytes());
    out.extend_from_slice(&(input.target.len() as u16).to_le_bytes());
    out.extend_from_slice(&6u16.to_le_bytes());
    out.extend_from_slice(&(input.original_source.len() as u32).to_le_bytes());
    out.extend_from_slice(&(input.final_catalog.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for axis in input.axes {
        axis.write(&mut out);
    }
    out.extend_from_slice(input.target.as_bytes());
    for row in input.roots {
        for word in [
            row.semantic_root(),
            row.descriptor_ordinal(),
            row.input_kernel_ordinal(),
            row.output_kernel_ordinal(),
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
    }
    out.extend_from_slice(input.original_source);
    out.extend_from_slice(input.final_catalog);
    let view = ExpandedOutputAssociationRefV1::read(
        &out,
        EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
        charge,
    )?;
    drop(view);
    Ok(out)
}
