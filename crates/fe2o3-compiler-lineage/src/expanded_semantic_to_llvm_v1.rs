//! Compact exact-content association; deliberately not an LLVM refinement proof.
use crate::expanded_history_receipt_v4::{Reader, allocate, storage, work};
use crate::{
    EXPANDED_OUTPUT_ASSOCIATION_DOMAIN_V1, EXPANDED_PUBLICATION_HASH_STORAGE_V4,
    ExpandedContentIdentityV4, ExpandedOutputAssociationRefV1, ExpandedOutputAxisV1,
    ExpandedPublicationErrorV4, expanded_content_identity_v4,
};
use std::mem::size_of;

/// Distinct final-expanded semantic/native content-association domain.
pub const EXPANDED_SEMANTIC_TO_LLVM_DOMAIN_V1: &[u8] = b"FE2O3/EXPANDED-SEMANTIC-TO-LLVM/V1\0";
/// Exact compact record size; no LLVM or graph payload is duplicated here.
pub const EXPANDED_SEMANTIC_TO_LLVM_BYTES_V1: usize = 16 + 5 * 40;
/// Borrowed result and reader plus its sequential association-hash scratch.
pub const EXPANDED_SEMANTIC_TO_LLVM_READ_STORAGE_V1: usize = size_of::<
    ExpandedSemanticToLlvmRefV1<'static>,
>() + size_of::<Reader<'static>>()
    + EXPANDED_PUBLICATION_HASH_STORAGE_V4;

/// Five inert exact identities in a fixed schema, with no producer authority.
pub struct ExpandedSemanticToLlvmRefV1<'a> {
    bytes: &'a [u8],
    axes: [ExpandedContentIdentityV4; 5],
}
impl<'a> ExpandedSemanticToLlvmRefV1<'a> {
    /// Reads only this explicit schema; legacy semantic/native receipts are rejected.
    /// Input backing is prepaid. The returned view remains part of the supplied extent.
    pub fn read<E>(
        bytes: &'a [u8],
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedPublicationErrorV4<E>> {
        storage(EXPANDED_SEMANTIC_TO_LLVM_READ_STORAGE_V1, available)?;
        if bytes.len() != EXPANDED_SEMANTIC_TO_LLVM_BYTES_V1 {
            return Err(ExpandedPublicationErrorV4::Format(
                "expanded derivation length",
            ));
        }
        work(bytes.len(), charge)?;
        let mut r = Reader::new(bytes);
        if r.take::<E>(8)? != b"F2EXLD1\0"
            || r.u16::<E>()? != 1
            || r.u16::<E>()? != 1
            || r.u32::<E>()? as usize != bytes.len()
        {
            return Err(ExpandedPublicationErrorV4::Format(
                "expanded derivation header",
            ));
        }
        let first = r.identity()?;
        let mut axes = [first; 5];
        for axis in &mut axes[1..] {
            *axis = r.identity()?;
        }
        r.finish()?;
        Ok(Self { bytes, axes })
    }
    /// Checks every coordinate against the complete association, not just a graph hash.
    /// Both views and backing are prepaid; `available` covers only fresh hash scratch.
    pub fn check_association<E>(
        &self,
        association: &ExpandedOutputAssociationRefV1<'_>,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), ExpandedPublicationErrorV4<E>> {
        let id = expanded_content_identity_v4(
            EXPANDED_OUTPUT_ASSOCIATION_DOMAIN_V1,
            association.canonical_bytes(),
            available,
            charge,
        )?;
        work(5 * 40, charge)?;
        if self.axes[0] != id
            || self.axes[1] != association.axis(ExpandedOutputAxisV1::History)
            || self.axes[2] != association.axis(ExpandedOutputAxisV1::FinalGraph)
            || self.axes[3] != association.axis(ExpandedOutputAxisV1::NativeModule)
            || self.axes[4] != association.axis(ExpandedOutputAxisV1::FinalCommitment)
        {
            return Err(ExpandedPublicationErrorV4::Identity(
                "expanded derivation association",
            ));
        }
        Ok(())
    }
    /// Exact canonical bytes of the compact record.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Exact native LLVM content identity, independently checked by ModuleV2 preflight.
    pub const fn native_module(&self) -> ExpandedContentIdentityV4 {
        self.axes[3]
    }
    /// Compact final commitment receipt identity, not an LLVM semantic theorem.
    pub const fn final_commitment(&self) -> ExpandedContentIdentityV4 {
        self.axes[4]
    }
}

/// Output Vec capacity/header plus simultaneous fixed record/hash work domain.
pub fn expanded_semantic_to_llvm_encode_storage_v1(capacity: usize) -> Option<usize> {
    size_of::<Vec<u8>>()
        .checked_add(capacity)?
        .checked_add(EXPANDED_SEMANTIC_TO_LLVM_READ_STORAGE_V1)?
        .checked_add(5 * size_of::<ExpandedContentIdentityV4>())
}
/// Frames the checked association's exact coordinates without authenticating it.
/// Input association/backing is prepaid; output capacity remains charged on return.
pub fn encode_expanded_semantic_to_llvm_v1<E>(
    association: &ExpandedOutputAssociationRefV1<'_>,
    available: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<u8>, ExpandedPublicationErrorV4<E>> {
    let n = EXPANDED_SEMANTIC_TO_LLVM_BYTES_V1;
    storage(
        expanded_semantic_to_llvm_encode_storage_v1(n)
            .ok_or(ExpandedPublicationErrorV4::Overflow)?,
        available,
    )?;
    let identity = expanded_content_identity_v4(
        EXPANDED_OUTPUT_ASSOCIATION_DOMAIN_V1,
        association.canonical_bytes(),
        EXPANDED_PUBLICATION_HASH_STORAGE_V4,
        charge,
    )?;
    let axes = [
        identity,
        association.axis(ExpandedOutputAxisV1::History),
        association.axis(ExpandedOutputAxisV1::FinalGraph),
        association.axis(ExpandedOutputAxisV1::NativeModule),
        association.axis(ExpandedOutputAxisV1::FinalCommitment),
    ];
    work(n, charge)?;
    let mut out = allocate(n)?;
    storage(
        expanded_semantic_to_llvm_encode_storage_v1(out.capacity())
            .ok_or(ExpandedPublicationErrorV4::Overflow)?,
        available,
    )?;
    out.extend_from_slice(b"F2EXLD1\0");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(n as u32).to_le_bytes());
    for axis in axes {
        axis.write(&mut out);
    }
    Ok(out)
}
