use std::{fmt, ops::Range, sync::Arc};

use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_kernel_descriptor::{DeviceTargetV1, ValidationError as TargetValidationError};
use fe2o3_rustc_invocation::{
    InvocationDigestV3, MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV3, decode_descriptor_v3,
    encode_descriptor_v3,
};
use sha2::{Digest, Sha256};

use crate::{
    InertAbiReceiptV3, InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirReceiptV3,
    InertDataLayoutReceiptV3, InertExportManifestReceiptV3,
    InertFinalCompilerModuleCommitmentReceiptV3, InertFormalMemoryReceiptV3,
    InertKernelIrReceiptV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingReceiptV3, InertRustcIdentityInventoryReceiptV3,
    InertRustcPreflightPlanReceiptV3, InertSemanticToLlvmReceiptV3, InertTargetBindingReceiptV3,
    LineageDecodeErrorV3, LineageErrorV3,
    receipt::{ImmutableBytesV3, SharedBackingV3},
};

/// Fixed magic at the start of every inert production semantic capsule V3.
pub const INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V3: [u8; 8] = *b"F2O3ISV3";

/// The only inert production semantic capsule version implemented by this crate.
pub const INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V3: u16 = 3;

/// Maximum complete canonical inert capsule bytes accepted by the V3 decoder.
pub const MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3: usize = 160 * 1024 * 1024;

/// Conservative compatibility ceiling for heap bytes retained by a successful decode.
///
/// V3 shared-range decoding no longer retains separate copies of the receipt
/// preimages. The tighter current payload bound is exposed as
/// [`InertProductionSemanticCapsuleV3::MAX_SUCCESSFUL_DECODE_RETAINED_BYTES`].
/// This historical exported ceiling remains unchanged for downstream resource
/// policies that already use it.
pub const MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_DECODE_OWNED_BYTES_V3: usize =
    2 * MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 + MAX_DESCRIPTOR_BYTES_V3;

const INERT_CAPSULE_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-PRODUCTION-SEMANTIC-CAPSULE/V3\0";
const HEADER_BYTES_V3: usize = 8 + 2 + 2 + 8 + 4;
const MAX_TARGET_BYTES_V3: usize = 128;
const SHA256_BYTES: usize = 32;
const RECEIPT_COUNT_V3: usize = 15;
const MIN_CAPSULE_BYTES_V3: usize = HEADER_BYTES_V3
    + 4
    + 1
    + SHA256_BYTES
    + 2
    + 1
    + RECEIPT_COUNT_V3 * (4 + 1 + SHA256_BYTES)
    + SHA256_BYTES;

/// The fixed, schema-ordered inert content chain retained by one capsule.
///
/// Field order is normative: rustc inventory, rustc preflight, semantic MIR,
/// middle end, Kernel IR, MIR-to-KIR correspondence, formal memory, proof
/// binding, target binding, target data layout, ABI, export manifest, AMDGPU
/// lowering, semantic-to-LLVM derivation, and compact final compiler-module
/// commitment.
#[derive(Debug, Eq, PartialEq)]
pub struct OrderedInertSemanticLineageReceiptsV3 {
    rustc_identity_inventory: InertRustcIdentityInventoryReceiptV3,
    rustc_preflight_plan: InertRustcPreflightPlanReceiptV3,
    semantic_mir: InertCanonicalSemanticMirReceiptV3,
    middle_end: InertMiddleEndReceiptV3,
    kernel_ir: InertKernelIrReceiptV3,
    mir_to_kir_correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
    proof_binding: InertProofBindingReceiptV3,
    target_binding: InertTargetBindingReceiptV3,
    data_layout: InertDataLayoutReceiptV3,
    abi: InertAbiReceiptV3,
    export_manifest: InertExportManifestReceiptV3,
    amdgpu_lowering: InertAmdgpuLoweringReceiptV3,
    semantic_to_llvm: InertSemanticToLlvmReceiptV3,
    final_compiler_module_commitment: InertFinalCompilerModuleCommitmentReceiptV3,
}

impl OrderedInertSemanticLineageReceiptsV3 {
    /// Joins the fifteen required inert content receipts in canonical schema order.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        rustc_identity_inventory: InertRustcIdentityInventoryReceiptV3,
        rustc_preflight_plan: InertRustcPreflightPlanReceiptV3,
        semantic_mir: InertCanonicalSemanticMirReceiptV3,
        middle_end: InertMiddleEndReceiptV3,
        kernel_ir: InertKernelIrReceiptV3,
        mir_to_kir_correspondence: InertMirToKirCorrespondenceReceiptV3,
        formal_memory: InertFormalMemoryReceiptV3,
        proof_binding: InertProofBindingReceiptV3,
        target_binding: InertTargetBindingReceiptV3,
        data_layout: InertDataLayoutReceiptV3,
        abi: InertAbiReceiptV3,
        export_manifest: InertExportManifestReceiptV3,
        amdgpu_lowering: InertAmdgpuLoweringReceiptV3,
        semantic_to_llvm: InertSemanticToLlvmReceiptV3,
        final_compiler_module_commitment: InertFinalCompilerModuleCommitmentReceiptV3,
    ) -> Self {
        Self {
            rustc_identity_inventory,
            rustc_preflight_plan,
            semantic_mir,
            middle_end,
            kernel_ir,
            mir_to_kir_correspondence,
            formal_memory,
            proof_binding,
            target_binding,
            data_layout,
            abi,
            export_manifest,
            amdgpu_lowering,
            semantic_to_llvm,
            final_compiler_module_commitment,
        }
    }

    /// Returns the rustc identity-inventory receipt.
    pub const fn rustc_identity_inventory(&self) -> &InertRustcIdentityInventoryReceiptV3 {
        &self.rustc_identity_inventory
    }

    /// Returns the rustc preflight-plan receipt.
    pub const fn rustc_preflight_plan(&self) -> &InertRustcPreflightPlanReceiptV3 {
        &self.rustc_preflight_plan
    }

    /// Returns the exact semantic-MIR receipt.
    pub const fn semantic_mir(&self) -> &InertCanonicalSemanticMirReceiptV3 {
        &self.semantic_mir
    }

    /// Returns the ordered middle-end receipt.
    pub const fn middle_end(&self) -> &InertMiddleEndReceiptV3 {
        &self.middle_end
    }

    /// Returns the canonical Kernel IR receipt.
    pub const fn kernel_ir(&self) -> &InertKernelIrReceiptV3 {
        &self.kernel_ir
    }

    /// Returns the MIR-to-KIR correspondence receipt.
    pub const fn mir_to_kir_correspondence(&self) -> &InertMirToKirCorrespondenceReceiptV3 {
        &self.mir_to_kir_correspondence
    }

    /// Returns the formal-memory receipt.
    pub const fn formal_memory(&self) -> &InertFormalMemoryReceiptV3 {
        &self.formal_memory
    }

    /// Returns the proof-binding-set receipt.
    pub const fn proof_binding(&self) -> &InertProofBindingReceiptV3 {
        &self.proof_binding
    }

    /// Returns the target-binding receipt.
    pub const fn target_binding(&self) -> &InertTargetBindingReceiptV3 {
        &self.target_binding
    }

    /// Returns the exact target data-layout receipt.
    pub const fn data_layout(&self) -> &InertDataLayoutReceiptV3 {
        &self.data_layout
    }

    /// Returns the ABI receipt.
    pub const fn abi(&self) -> &InertAbiReceiptV3 {
        &self.abi
    }

    /// Returns the exact export-manifest receipt.
    pub const fn export_manifest(&self) -> &InertExportManifestReceiptV3 {
        &self.export_manifest
    }

    /// Returns the AMDGPU lowering receipt.
    pub const fn amdgpu_lowering(&self) -> &InertAmdgpuLoweringReceiptV3 {
        &self.amdgpu_lowering
    }

    /// Returns the semantic-to-LLVM derivation receipt.
    ///
    /// Its stage-specific producer codec is responsible for binding the exact
    /// final LLVM module identity and compact final-module commitment identity
    /// as distinct axes.
    pub const fn semantic_to_llvm(&self) -> &InertSemanticToLlvmReceiptV3 {
        &self.semantic_to_llvm
    }

    /// Returns the compact final compiler-module commitment receipt.
    ///
    /// Exact final LLVM bytes are retained by the surrounding V2 module handoff,
    /// not duplicated in this capsule receipt.
    pub const fn final_compiler_module_commitment(
        &self,
    ) -> &InertFinalCompilerModuleCommitmentReceiptV3 {
        &self.final_compiler_module_commitment
    }
}

/// Domain-separated identity of one complete canonical capsule preimage.
///
/// The SHA-256 preimage is the domain, the little-endian length of the bytes
/// preceding the terminal identity, and those exact bytes. `byte_len` includes
/// the terminal 32-byte identity stored by the wire format.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertProductionSemanticCapsuleIdentityV3 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertProductionSemanticCapsuleIdentityV3 {
    /// Returns the capsule's domain-separated SHA-256 bytes.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    /// Returns the complete canonical wire length, including terminal identity.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks exact canonical bytes against this identity without granting authority.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        if self.byte_len != bytes.len() as u64 || bytes.len() < SHA256_BYTES {
            return false;
        }
        let preimage_len = bytes.len() - SHA256_BYTES;
        bytes[preimage_len..] == self.sha256
            && derive_capsule_sha256(&bytes[..preimage_len])
                .is_some_and(|value| value == self.sha256)
    }
}

impl fmt::Debug for InertProductionSemanticCapsuleIdentityV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionSemanticCapsuleIdentityV3")
            .field("sha256", &self.sha256)
            .field("byte_len", &self.byte_len)
            .finish()
    }
}

/// Complete inert content record for one caller-selected semantic lineage.
///
/// Public construction and successful decoding establish only internal byte
/// consistency. They do not authenticate a producer, prove stage derivations,
/// or grant compiler, artifact, publication, load, or launch authority.
#[derive(Eq, PartialEq)]
pub struct InertProductionSemanticCapsuleV3 {
    invocation: RustcInvocationDescriptorV3,
    invocation_digest: InvocationDigestV3,
    target: DeviceTargetV1,
    receipts: OrderedInertSemanticLineageReceiptsV3,
    identity: InertProductionSemanticCapsuleIdentityV3,
    canonical_bytes: ImmutableBytesV3,
}

impl InertProductionSemanticCapsuleV3 {
    /// Tight retained payload-byte bound for successful borrowed-input decoding.
    ///
    /// This includes one admitted canonical capsule buffer and one decoded
    /// invocation representation. [`Self::decode_shared`] reuses its caller's
    /// admitted buffer, so its additional retained payload is bounded by
    /// `MAX_DESCRIPTOR_BYTES_V3`.
    pub const MAX_SUCCESSFUL_DECODE_RETAINED_BYTES: usize =
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 + MAX_DESCRIPTOR_BYTES_V3;

    /// Constructs one internally consistent inert capsule from exact preimages.
    pub fn new(
        invocation: RustcInvocationDescriptorV3,
        target: DeviceTargetV1,
        receipts: OrderedInertSemanticLineageReceiptsV3,
    ) -> Result<Self, LineageErrorV3> {
        let invocation_bytes =
            encode_descriptor_v3(&invocation).map_err(LineageErrorV3::Invocation)?;
        let invocation_digest =
            InvocationDigestV3::calculate(&invocation).map_err(|error| match error {
                fe2o3_rustc_invocation::DigestError::Encoding(validation) => {
                    LineageErrorV3::Invocation(validation)
                }
                _ => LineageErrorV3::ZeroIdentity {
                    field: "rustc invocation",
                },
            })?;
        if invocation.amd_target() != target.to_string() {
            return Err(LineageErrorV3::TargetMismatch);
        }

        let target_text = target.to_string();
        if target_text.is_empty() || target_text.len() > MAX_TARGET_BYTES_V3 {
            return Err(LineageErrorV3::LengthOverflow);
        }
        let total_len = encoded_len(&invocation_bytes, &target_text, &receipts)?;
        if total_len > MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 {
            return Err(LineageErrorV3::CapsuleTooLarge {
                max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
            });
        }
        let total_len_u64 = u64::try_from(total_len).map_err(|_| LineageErrorV3::LengthOverflow)?;

        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(total_len)
            .map_err(|_| LineageErrorV3::LengthOverflow)?;
        canonical.extend_from_slice(&INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V3);
        canonical.extend_from_slice(&INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V3.to_le_bytes());
        canonical.extend_from_slice(&0_u16.to_le_bytes());
        canonical.extend_from_slice(&total_len_u64.to_le_bytes());
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        push_blob(&mut canonical, &invocation_bytes)?;
        canonical.extend_from_slice(invocation_digest.as_bytes());
        let target_len =
            u16::try_from(target_text.len()).map_err(|_| LineageErrorV3::LengthOverflow)?;
        canonical.extend_from_slice(&target_len.to_le_bytes());
        canonical.extend_from_slice(target_text.as_bytes());
        encode_receipts(&mut canonical, &receipts)?;
        let capsule_sha256 =
            derive_capsule_sha256(&canonical).ok_or(LineageErrorV3::ZeroIdentity {
                field: "inert production semantic capsule",
            })?;
        canonical.extend_from_slice(&capsule_sha256);
        debug_assert_eq!(canonical.len(), total_len);
        let identity = InertProductionSemanticCapsuleIdentityV3 {
            sha256: capsule_sha256,
            byte_len: total_len_u64,
        };

        Ok(Self {
            invocation,
            invocation_digest,
            target,
            receipts,
            identity,
            canonical_bytes: ImmutableBytesV3::from_owned(canonical.into_boxed_slice()),
        })
    }

    /// Strictly decodes one complete canonical V3 capsule with no fallback.
    pub fn decode(bytes: &[u8]) -> Result<Self, LineageDecodeErrorV3> {
        if bytes.len() > MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 {
            return Err(LineageDecodeErrorV3::TooLarge {
                max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
            });
        }
        let mut admitted = Vec::new();
        admitted
            .try_reserve_exact(bytes.len())
            .map_err(|_| LineageDecodeErrorV3::TooLarge {
                max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
            })?;
        admitted.extend_from_slice(bytes);
        let admitted_len = admitted.len();
        Self::decode_shared_backing(SharedBackingV3::Vector(Arc::new(admitted)), 0..admitted_len)
    }

    /// Strictly decodes a checked range in one immutable caller-owned buffer.
    ///
    /// The returned capsule and all fifteen receipt preimages retain ranges in
    /// this exact `Arc` allocation. No capsule-sized or receipt-sized payload
    /// copy is made. Reversed ranges are rejected as invalid lengths and ranges
    /// outside `backing` are rejected as truncated input.
    pub fn decode_shared(
        backing: Arc<[u8]>,
        capsule_range: Range<usize>,
    ) -> Result<Self, LineageDecodeErrorV3> {
        Self::decode_shared_backing(SharedBackingV3::Slice(backing), capsule_range)
    }

    fn decode_shared_backing(
        backing: SharedBackingV3,
        capsule_range: Range<usize>,
    ) -> Result<Self, LineageDecodeErrorV3> {
        let capsule_len = validate_capsule_range(backing.as_slice().len(), &capsule_range)?;
        let bytes = backing
            .as_slice()
            .get(capsule_range.clone())
            .ok_or(LineageDecodeErrorV3::Truncated)?;
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V3 {
            return Err(LineageDecodeErrorV3::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V3 {
            return Err(LineageDecodeErrorV3::UnsupportedVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(LineageDecodeErrorV3::UnsupportedFlags(flags));
        }
        let declared_len = reader.u64()?;
        if declared_len < MIN_CAPSULE_BYTES_V3 as u64 {
            return Err(LineageDecodeErrorV3::InvalidLength(declared_len));
        }
        let declared_len_usize = usize::try_from(declared_len)
            .map_err(|_| LineageDecodeErrorV3::InvalidLength(declared_len))?;
        if declared_len_usize > MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 {
            return Err(LineageDecodeErrorV3::TooLarge {
                max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
            });
        }
        if declared_len_usize > capsule_len {
            return Err(LineageDecodeErrorV3::Truncated);
        }
        if declared_len_usize < capsule_len {
            return Err(LineageDecodeErrorV3::TrailingBytes);
        }
        if reader.u32()? != 0 {
            return Err(LineageDecodeErrorV3::NonzeroReserved);
        }

        let invocation_len = reader.bounded_u32("rustc invocation", MAX_DESCRIPTOR_BYTES_V3)?;
        let invocation_range = reader.take_range(invocation_len)?;
        let invocation_bytes = &bytes[invocation_range];
        let invocation =
            decode_descriptor_v3(invocation_bytes).map_err(LineageDecodeErrorV3::Invocation)?;
        let canonical_invocation =
            encode_descriptor_v3(&invocation).map_err(|_| LineageDecodeErrorV3::NonCanonical)?;
        if canonical_invocation.as_slice() != invocation_bytes {
            return Err(LineageDecodeErrorV3::NonCanonical);
        }
        let declared_invocation_digest = reader.fixed::<32>()?;
        if declared_invocation_digest == [0; 32] {
            return Err(LineageDecodeErrorV3::ZeroIdentity {
                field: "rustc invocation",
            });
        }
        let invocation_digest = InvocationDigestV3::calculate(&invocation)
            .map_err(|_| LineageDecodeErrorV3::NonCanonical)?;
        if invocation_digest.into_bytes() != declared_invocation_digest {
            return Err(LineageDecodeErrorV3::InvocationDigestMismatch);
        }

        let target_len = usize::from(reader.u16()?);
        if target_len == 0 || target_len > MAX_TARGET_BYTES_V3 {
            return Err(LineageDecodeErrorV3::InvalidTarget);
        }
        let target_range = reader.take_range(target_len)?;
        let target_text = std::str::from_utf8(&bytes[target_range])
            .map_err(|_| LineageDecodeErrorV3::InvalidTargetText)?;
        let target = DeviceTargetV1::parse(target_text).map_err(|error| match error {
            TargetValidationError::NonCanonicalOrder { .. } => LineageDecodeErrorV3::NonCanonical,
            _ => LineageDecodeErrorV3::InvalidTarget,
        })?;
        if target.to_string() != target_text {
            return Err(LineageDecodeErrorV3::NonCanonical);
        }
        if invocation.amd_target() != target_text {
            return Err(LineageDecodeErrorV3::TargetMismatch);
        }

        macro_rules! decode_receipt {
            ($type:ty) => {{
                let len = reader.bounded_u32(<$type>::FIELD, <$type>::MAX_BYTES)?;
                let local_range = reader.take_range(len)?;
                let identity = reader.fixed::<32>()?;
                let absolute_range =
                    absolute_range(capsule_range.start, local_range, capsule_range.end)?;
                <$type>::decode_shared(backing.clone(), absolute_range, identity)?
            }};
        }

        let receipts = OrderedInertSemanticLineageReceiptsV3::new(
            decode_receipt!(InertRustcIdentityInventoryReceiptV3),
            decode_receipt!(InertRustcPreflightPlanReceiptV3),
            decode_receipt!(InertCanonicalSemanticMirReceiptV3),
            decode_receipt!(InertMiddleEndReceiptV3),
            decode_receipt!(InertKernelIrReceiptV3),
            decode_receipt!(InertMirToKirCorrespondenceReceiptV3),
            decode_receipt!(InertFormalMemoryReceiptV3),
            decode_receipt!(InertProofBindingReceiptV3),
            decode_receipt!(InertTargetBindingReceiptV3),
            decode_receipt!(InertDataLayoutReceiptV3),
            decode_receipt!(InertAbiReceiptV3),
            decode_receipt!(InertExportManifestReceiptV3),
            decode_receipt!(InertAmdgpuLoweringReceiptV3),
            decode_receipt!(InertSemanticToLlvmReceiptV3),
            decode_receipt!(InertFinalCompilerModuleCommitmentReceiptV3),
        );

        let terminal_offset = reader.offset();
        let declared_capsule_sha256 = reader.fixed::<32>()?;
        if declared_capsule_sha256 == [0; 32] {
            return Err(LineageDecodeErrorV3::ZeroIdentity {
                field: "inert production semantic capsule",
            });
        }
        if !reader.is_empty() {
            return Err(LineageDecodeErrorV3::TrailingBytes);
        }
        if derive_capsule_sha256(&bytes[..terminal_offset]) != Some(declared_capsule_sha256) {
            return Err(LineageDecodeErrorV3::CapsuleIdentityMismatch);
        }

        let identity = InertProductionSemanticCapsuleIdentityV3 {
            sha256: declared_capsule_sha256,
            byte_len: declared_len,
        };
        let canonical_bytes = ImmutableBytesV3::from_shared(backing, capsule_range)
            .ok_or(LineageDecodeErrorV3::Truncated)?;
        Ok(Self {
            invocation,
            invocation_digest,
            target,
            receipts,
            identity,
            canonical_bytes,
        })
    }

    /// Returns the exact canonical V3 rustc invocation.
    pub const fn invocation(&self) -> &RustcInvocationDescriptorV3 {
        &self.invocation
    }

    /// Returns the digest rederived from the exact retained invocation.
    pub const fn invocation_digest(&self) -> InvocationDigestV3 {
        self.invocation_digest
    }

    /// Returns the compiler closure revalidated by the retained invocation.
    pub const fn compiler_closure(&self) -> &CompilerClosureV2 {
        self.invocation.compiler_closure()
    }

    /// Returns the canonical target that matches the retained invocation.
    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    /// Returns the fixed ordered receipt chain.
    pub const fn receipts(&self) -> &OrderedInertSemanticLineageReceiptsV3 {
        &self.receipts
    }

    /// Returns the capsule identity derived from all preceding canonical bytes.
    pub const fn identity(&self) -> InertProductionSemanticCapsuleIdentityV3 {
        self.identity
    }

    /// Returns the complete canonical encoding, including terminal identity.
    pub fn canonical_bytes(&self) -> &[u8] {
        self.canonical_bytes.as_slice()
    }

    /// Reports the security limit that this inert object does not authenticate a producer.
    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    /// Reports the security limit that this inert object grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports the security limit that this inert object grants no artifact authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    /// Reports the security limit that this inert object grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports the security limit that this inert object grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports the security limit that this inert object grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for InertProductionSemanticCapsuleV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionSemanticCapsuleV3")
            .field("invocation_digest", &self.invocation_digest)
            .field(
                "compiler_closure_identity",
                &self.compiler_closure().identity_sha256(),
            )
            .field("target", &self.target)
            .field("receipts", &self.receipts)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

fn encoded_len(
    invocation_bytes: &[u8],
    target_text: &str,
    receipts: &OrderedInertSemanticLineageReceiptsV3,
) -> Result<usize, LineageErrorV3> {
    let mut length = HEADER_BYTES_V3;
    add_len(&mut length, 4)?;
    add_len(&mut length, invocation_bytes.len())?;
    add_len(&mut length, SHA256_BYTES)?;
    add_len(&mut length, 2)?;
    add_len(&mut length, target_text.len())?;
    for payload_len in [
        receipts.rustc_identity_inventory.canonical_preimage().len(),
        receipts.rustc_preflight_plan.canonical_preimage().len(),
        receipts.semantic_mir.canonical_preimage().len(),
        receipts.middle_end.canonical_preimage().len(),
        receipts.kernel_ir.canonical_preimage().len(),
        receipts
            .mir_to_kir_correspondence
            .canonical_preimage()
            .len(),
        receipts.formal_memory.canonical_preimage().len(),
        receipts.proof_binding.canonical_preimage().len(),
        receipts.target_binding.canonical_preimage().len(),
        receipts.data_layout.canonical_preimage().len(),
        receipts.abi.canonical_preimage().len(),
        receipts.export_manifest.canonical_preimage().len(),
        receipts.amdgpu_lowering.canonical_preimage().len(),
        receipts.semantic_to_llvm.canonical_preimage().len(),
        receipts
            .final_compiler_module_commitment
            .canonical_preimage()
            .len(),
    ] {
        add_len(&mut length, 4)?;
        add_len(&mut length, payload_len)?;
        add_len(&mut length, SHA256_BYTES)?;
    }
    add_len(&mut length, SHA256_BYTES)?;
    Ok(length)
}

fn add_len(total: &mut usize, amount: usize) -> Result<(), LineageErrorV3> {
    *total = total
        .checked_add(amount)
        .ok_or(LineageErrorV3::LengthOverflow)?;
    Ok(())
}

fn push_blob(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), LineageErrorV3> {
    let length = u32::try_from(bytes.len()).map_err(|_| LineageErrorV3::LengthOverflow)?;
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn push_receipt(
    output: &mut Vec<u8>,
    bytes: &[u8],
    identity: &[u8; 32],
) -> Result<(), LineageErrorV3> {
    push_blob(output, bytes)?;
    output.extend_from_slice(identity);
    Ok(())
}

fn encode_receipts(
    output: &mut Vec<u8>,
    receipts: &OrderedInertSemanticLineageReceiptsV3,
) -> Result<(), LineageErrorV3> {
    push_receipt(
        output,
        receipts.rustc_identity_inventory.canonical_preimage(),
        receipts.rustc_identity_inventory.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.rustc_preflight_plan.canonical_preimage(),
        receipts.rustc_preflight_plan.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.semantic_mir.canonical_preimage(),
        receipts.semantic_mir.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.middle_end.canonical_preimage(),
        receipts.middle_end.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.kernel_ir.canonical_preimage(),
        receipts.kernel_ir.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.mir_to_kir_correspondence.canonical_preimage(),
        receipts.mir_to_kir_correspondence.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.formal_memory.canonical_preimage(),
        receipts.formal_memory.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.proof_binding.canonical_preimage(),
        receipts.proof_binding.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.target_binding.canonical_preimage(),
        receipts.target_binding.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.data_layout.canonical_preimage(),
        receipts.data_layout.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.abi.canonical_preimage(),
        receipts.abi.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.export_manifest.canonical_preimage(),
        receipts.export_manifest.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.amdgpu_lowering.canonical_preimage(),
        receipts.amdgpu_lowering.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts.semantic_to_llvm.canonical_preimage(),
        receipts.semantic_to_llvm.identity().sha256(),
    )?;
    push_receipt(
        output,
        receipts
            .final_compiler_module_commitment
            .canonical_preimage(),
        receipts
            .final_compiler_module_commitment
            .identity()
            .sha256(),
    )
}

fn derive_capsule_sha256(bytes: &[u8]) -> Option<[u8; 32]> {
    let byte_len = u64::try_from(bytes.len()).ok()?;
    let mut digest = Sha256::new();
    digest.update(INERT_CAPSULE_IDENTITY_DOMAIN_V3);
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    let sha256: [u8; 32] = digest.finalize().into();
    (sha256 != [0; 32]).then_some(sha256)
}

fn validate_capsule_range(
    backing_len: usize,
    range: &Range<usize>,
) -> Result<usize, LineageDecodeErrorV3> {
    let length = range
        .end
        .checked_sub(range.start)
        .ok_or(LineageDecodeErrorV3::InvalidLength(u64::MAX))?;
    if range.end > backing_len {
        return Err(LineageDecodeErrorV3::Truncated);
    }
    if length > MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 {
        return Err(LineageDecodeErrorV3::TooLarge {
            max: MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
        });
    }
    Ok(length)
}

fn absolute_range(
    base: usize,
    local: Range<usize>,
    capsule_end: usize,
) -> Result<Range<usize>, LineageDecodeErrorV3> {
    let start = base
        .checked_add(local.start)
        .ok_or(LineageDecodeErrorV3::Truncated)?;
    let end = base
        .checked_add(local.end)
        .ok_or(LineageDecodeErrorV3::Truncated)?;
    if start > end || end > capsule_end {
        return Err(LineageDecodeErrorV3::Truncated);
    }
    Ok(start..end)
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], LineageDecodeErrorV3> {
        let range = self.take_range(length)?;
        self.bytes.get(range).ok_or(LineageDecodeErrorV3::Truncated)
    }

    fn take_range(&mut self, length: usize) -> Result<Range<usize>, LineageDecodeErrorV3> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(LineageDecodeErrorV3::Truncated)?;
        self.bytes
            .get(self.offset..end)
            .ok_or(LineageDecodeErrorV3::Truncated)?;
        let range = self.offset..end;
        self.offset = end;
        Ok(range)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], LineageDecodeErrorV3> {
        self.take(N)?
            .try_into()
            .map_err(|_| LineageDecodeErrorV3::Truncated)
    }

    fn u16(&mut self) -> Result<u16, LineageDecodeErrorV3> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, LineageDecodeErrorV3> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, LineageDecodeErrorV3> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn bounded_u32(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<usize, LineageDecodeErrorV3> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| LineageDecodeErrorV3::PreimageTooLarge { field, max })?;
        if length > max {
            return Err(LineageDecodeErrorV3::PreimageTooLarge { field, max });
        }
        Ok(length)
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
