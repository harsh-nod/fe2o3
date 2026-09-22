//! Explicit sixteen-slot expanded capsule. Old V3 framing is unchanged.
pub use crate::capsule_v4_decode::{EXPANDED_CAPSULE_READ_STORAGE_V4, ExpandedCapsuleRefV4};
use crate::expanded_history_receipt_v4::{allocate, storage, work};
use crate::{
    EXPANDED_HISTORY_RECEIPT_DOMAIN_V4, EXPANDED_PUBLICATION_HASH_STORAGE_V4,
    ExpandedContentIdentityV4, ExpandedPublicationErrorV4, MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3,
    MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
    expanded_content_identity_v4,
};
use std::mem::size_of;

/// Distinct expanded capsule schema magic.
pub const INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V4: [u8; 8] = *b"F2O3ISV4";
/// Hash domain for complete canonical capsule bytes, including all sixteen slots.
pub const INERT_PRODUCTION_SEMANTIC_CAPSULE_DOMAIN_V4: &[u8] =
    b"FE2O3/INERT-PRODUCTION-SEMANTIC-CAPSULE/V4\0";
/// Fixed schema-ordered receipt count. History is mandatory, not an optional sidecar.
pub const EXPANDED_CAPSULE_RECEIPT_COUNT_V4: usize = 16;
pub(crate) const HEADER: usize = 60;
pub(crate) const DIRECTORY: usize = 44 * EXPANDED_CAPSULE_RECEIPT_COUNT_V4;

/// Fixed capsule positions; existing payload receipt domains and caps are unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum ExpandedReceiptSlotV4 {
    /// Original rustc identity inventory.
    RustcInventory = 0,
    /// Original protected rustc preflight.
    RustcPreflight,
    /// Actual complete original semantic MIR.
    SemanticMir,
    /// Original middle-end receipt.
    MiddleEnd,
    /// Complete canonical final V12 graph, not a digest-only substitute.
    FinalKernelIr,
    /// Original MIR-to-neutral correspondence.
    Correspondence,
    /// Fresh final formal-memory roster.
    FinalFormal,
    /// Explicit expanded association, with original source proof content.
    ProofBinding,
    /// Exact actual target binding.
    TargetBinding,
    /// Exact actual data layout.
    DataLayout,
    /// Canonical zero-code-object-digest nominal descriptor V3 bytes.
    NominalAbi,
    /// Exact final export manifest.
    ExportManifest,
    /// Actual final AMDGPU lowering receipt.
    AmdgpuLowering,
    /// Explicit expanded semantic/native association.
    SemanticToLlvm,
    /// Compact final ModuleV2 commitment.
    FinalModuleCommitment,
    /// Complete U plus one scalar fixed-point history.
    ExpandedHistory,
}
impl ExpandedReceiptSlotV4 {
    /// Normative complete order, including the required new history slot.
    pub const ALL: [Self; 16] = [
        Self::RustcInventory,
        Self::RustcPreflight,
        Self::SemanticMir,
        Self::MiddleEnd,
        Self::FinalKernelIr,
        Self::Correspondence,
        Self::FinalFormal,
        Self::ProofBinding,
        Self::TargetBinding,
        Self::DataLayout,
        Self::NominalAbi,
        Self::ExportManifest,
        Self::AmdgpuLowering,
        Self::SemanticToLlvm,
        Self::FinalModuleCommitment,
        Self::ExpandedHistory,
    ];
    /// Unchanged old per-slot ceiling, or the new history's aggregate ceiling.
    pub const fn maximum_bytes(self) -> usize {
        match self {
            Self::SemanticMir => MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3,
            Self::ExpandedHistory => MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4,
            _ => MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
        }
    }
    /// Existing content-receipt domain, with a separate domain only for the new slot.
    pub const fn identity_domain(self) -> &'static [u8] {
        match self {
            Self::RustcInventory => b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-IDENTITY-INVENTORY/V3\0",
            Self::RustcPreflight => b"FE2O3/INERT-LINEAGE-CONTENT/RUSTC-PREFLIGHT-PLAN/V3\0",
            Self::SemanticMir => b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-SEMANTIC-MIR/V3\0",
            Self::MiddleEnd => b"FE2O3/INERT-LINEAGE-CONTENT/MIDDLE-END-PASS-CHAIN/V3\0",
            Self::FinalKernelIr => b"FE2O3/INERT-LINEAGE-CONTENT/CANONICAL-KERNEL-IR/V3\0",
            Self::Correspondence => b"FE2O3/INERT-LINEAGE-CONTENT/MIR-TO-KIR-CORRESPONDENCE/V3\0",
            Self::FinalFormal => b"FE2O3/INERT-LINEAGE-CONTENT/FORMAL-MEMORY-OBLIGATIONS/V3\0",
            Self::ProofBinding => b"FE2O3/INERT-LINEAGE-CONTENT/PROOF-BINDING-SET/V3\0",
            Self::TargetBinding => b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-BINDING/V3\0",
            Self::DataLayout => b"FE2O3/INERT-LINEAGE-CONTENT/TARGET-DATA-LAYOUT/V3\0",
            Self::NominalAbi => b"FE2O3/INERT-LINEAGE-CONTENT/ABI/V3\0",
            Self::ExportManifest => b"FE2O3/INERT-LINEAGE-CONTENT/EXPORT-MANIFEST/V3\0",
            Self::AmdgpuLowering => b"FE2O3/INERT-LINEAGE-CONTENT/AMDGPU-LOWERING/V3\0",
            Self::SemanticToLlvm => b"FE2O3/INERT-LINEAGE-CONTENT/SEMANTIC-TO-LLVM/V3\0",
            Self::FinalModuleCommitment => {
                b"FE2O3/INERT-LINEAGE-CONTENT/FINAL-COMPILER-MODULE-COMMITMENT/V3\0"
            }
            Self::ExpandedHistory => EXPANDED_HISTORY_RECEIPT_DOMAIN_V4,
        }
    }
}

/// Complete caller-owned inputs; their backing/header custody is prepaid separately.
pub struct ExpandedCapsuleInputsV4<'a> {
    /// Canonical actual invocation V3 bytes, validated with the unchanged strict decoder.
    pub invocation: &'a [u8],
    /// Actual canonical target spelling, including features.
    pub target: &'a str,
    /// All sixteen complete payloads in normative order.
    pub receipts: [&'a [u8]; EXPANDED_CAPSULE_RECEIPT_COUNT_V4],
}
/// A validated borrowed content slot, not an authenticated compiler receipt.
#[derive(Clone, Copy, Debug)]
pub struct ExpandedReceiptRefV4<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) identity: ExpandedContentIdentityV4,
}
impl<'a> ExpandedReceiptRefV4<'a> {
    /// Complete immutable slot bytes.
    pub const fn canonical_preimage(self) -> &'a [u8] {
        self.bytes
    }
    /// Exact slot-domain identity of those bytes.
    pub const fn identity(self) -> ExpandedContentIdentityV4 {
        self.identity
    }
}

/// Computes the complete aggregate BEFORE allocating or constructing payload owners.
pub fn expanded_capsule_length_v4(input: &ExpandedCapsuleInputsV4<'_>) -> Option<usize> {
    if input.invocation.is_empty()
        || input.invocation.len() > fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3
        || input.target.is_empty()
        || input.target.len() > 128
    {
        return None;
    }
    let mut total = HEADER
        .checked_add(DIRECTORY)?
        .checked_add(input.invocation.len())?
        .checked_add(input.target.len())?;
    for (slot, bytes) in ExpandedReceiptSlotV4::ALL.into_iter().zip(input.receipts) {
        if bytes.is_empty() || bytes.len() > slot.maximum_bytes() {
            return None;
        }
        total = total.checked_add(bytes.len())?;
    }
    (total <= MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4).then_some(total)
}
/// Complete owned header plus ACTUAL transferred Vec capacity.
pub fn expanded_capsule_retained_storage_v4(capacity: usize) -> Option<usize> {
    size_of::<InertProductionSemanticCapsuleV4>().checked_add(capacity)
}
/// Complete owner and additional reader/legacy-child/returned-view domain.
pub fn expanded_capsule_validation_storage_v4(capacity: usize) -> Option<usize> {
    expanded_capsule_retained_storage_v4(capacity)?.checked_add(EXPANDED_CAPSULE_READ_STORAGE_V4)
}

/// Move-only strict V4 bytes, not a signed producer or semantic authority.
/// ```compile_fail
/// use fe2o3_compiler_lineage::InertProductionSemanticCapsuleV4;
/// fn duplicate(x: InertProductionSemanticCapsuleV4) { let _ = x.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_lineage::{InertProductionSemanticCapsuleV3, InertProductionSemanticCapsuleV4};
/// fn relabel(x: InertProductionSemanticCapsuleV4) -> InertProductionSemanticCapsuleV3 { x.into() }
/// ```
pub struct InertProductionSemanticCapsuleV4 {
    bytes: Vec<u8>,
    identity: ExpandedContentIdentityV4,
}
impl InertProductionSemanticCapsuleV4 {
    /// Produces the explicit successor. Inputs stay prepaid while output and validation coexist.
    pub fn new<E>(
        input: &ExpandedCapsuleInputsV4<'_>,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedPublicationErrorV4<E>> {
        let length = expanded_capsule_length_v4(input).ok_or(
            ExpandedPublicationErrorV4::Format("capsule aggregate or slot cap"),
        )?;
        storage(
            expanded_capsule_validation_storage_v4(length)
                .ok_or(ExpandedPublicationErrorV4::Overflow)?,
            available,
        )?;
        let digest =
            crate::capsule_v4_decode::invocation_digest(input.invocation, input.target, charge)?;
        work(length, charge)?;
        let mut bytes = allocate(length)?;
        storage(
            expanded_capsule_validation_storage_v4(bytes.capacity())
                .ok_or(ExpandedPublicationErrorV4::Overflow)?,
            available,
        )?;
        bytes.extend_from_slice(&INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V4);
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&(length as u64).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(&(input.target.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(input.invocation.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&digest);
        for slot in ExpandedReceiptSlotV4::ALL {
            let payload = input.receipts[slot as usize];
            let id = expanded_content_identity_v4(
                slot.identity_domain(),
                payload,
                EXPANDED_PUBLICATION_HASH_STORAGE_V4,
                charge,
            )?;
            bytes.extend_from_slice(&(slot as u16).to_le_bytes());
            bytes.extend_from_slice(&0u16.to_le_bytes());
            bytes.extend_from_slice(&id.byte_len().to_le_bytes());
            bytes.extend_from_slice(id.sha256());
        }
        bytes.extend_from_slice(input.invocation);
        bytes.extend_from_slice(input.target.as_bytes());
        for payload in input.receipts {
            bytes.extend_from_slice(payload);
        }
        Self::decode_owned(bytes, available, charge)
    }
    /// Transfers one owned canonical buffer; no second complete buffer is allocated.
    pub fn decode_owned<E>(
        bytes: Vec<u8>,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedPublicationErrorV4<E>> {
        storage(
            expanded_capsule_validation_storage_v4(bytes.capacity())
                .ok_or(ExpandedPublicationErrorV4::Overflow)?,
            available,
        )?;
        let view = ExpandedCapsuleRefV4::read(&bytes, EXPANDED_CAPSULE_READ_STORAGE_V4, charge)?;
        let identity = view.identity();
        drop(view);
        Ok(Self { bytes, identity })
    }
    /// Returns a freshly validated borrowed view; the complete owner stays included.
    pub fn view<E>(
        &self,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<ExpandedCapsuleRefV4<'_>, ExpandedPublicationErrorV4<E>> {
        storage(
            expanded_capsule_validation_storage_v4(self.bytes.capacity())
                .ok_or(ExpandedPublicationErrorV4::Overflow)?,
            available,
        )?;
        ExpandedCapsuleRefV4::read(&self.bytes, EXPANDED_CAPSULE_READ_STORAGE_V4, charge)
    }
    /// Complete immutable canonical bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Domain/length/content identity of the complete capsule.
    pub const fn identity(&self) -> ExpandedContentIdentityV4 {
        self.identity
    }
    /// Full owned header and actual Vec capacity.
    pub fn retained_storage(&self) -> usize {
        expanded_capsule_retained_storage_v4(self.bytes.capacity()).expect("validated extent")
    }
    /// No source, publication, load or launch authority is granted.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
