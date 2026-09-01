use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::{
    KERNEL_IR_MAGIC_V1, KERNEL_IR_VERSION_V8, KERNEL_IR_VERSION_V9, KernelId, MAX_KERNELS_V1,
    MAX_MODULE_BYTES_V1, MAX_TEXT_BYTES_V1, Module, VerifiedCanonicalKernelIrErrorV8,
    VerifiedCanonicalKernelIrErrorV9, VerifiedCanonicalKernelIrV8, VerifiedCanonicalKernelIrV9,
};
use sha2::{Digest, Sha256};

use crate::{
    LoweringErrors, MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1,
    MAX_PRODUCTION_LEGACY_REPLAY_LLVM_TEXT_BYTES_V1, ProductionLlvmLayoutBindingErrorV1,
    ProductionSemanticAnchorKirIdentityV1, ProductionTargetBindingErrorV1,
    ProductionTargetSemanticValidationErrorV1, ProductionTargetStructuralBindingV1,
    ValidatedProductionTargetSemanticBindingV1, bind_historical_replay_llvm_layout_v1,
    bind_production_llvm22_worker_layout_v1, bind_production_target_v1,
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir,
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_kernel_to_gfx942_xnack_minus_replay_llvm_ir_v1,
    lower_kernel_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_kernel_to_gfx950_xnack_minus_replay_llvm_ir_v1,
    validate_production_target_semantic_binding_v1,
};

const EVIDENCE_MAGIC_V1: &[u8] = b"FE2O3/KIR-TO-LLVM-REPLAY/V1\0";
const EVIDENCE_VERSION_V1: u16 = 1;
const EVIDENCE_VERSION_V2: u16 = 2;
const EXACT_DETERMINISTIC_REPLAY_CLAIM_V1: u8 = 1;
const RESERVED_V1: u8 = 0;
const GFX942_PROFILE_TAG_V1: u8 = 1;
const GFX950_PROFILE_TAG_V1: u8 = 2;
const KIR_V8_TAG_V1: u8 = 8;
const KIR_V9_TAG_V1: u8 = 9;
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/KIR-TO-LLVM-REPLAY/IDENTITY/V1\0";

pub const MAX_PRODUCTION_PRE_DESCRIPTOR_LLVM_BYTES_V1: usize =
    MAX_PRODUCTION_LEGACY_REPLAY_LLVM_TEXT_BYTES_V1;

/// Exact canonical Kernel IR version carried by replay evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionReplayKernelIrVersionV1 {
    V8,
    V9,
}

impl ProductionReplayKernelIrVersionV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::V8 => KIR_V8_TAG_V1,
            Self::V9 => KIR_V9_TAG_V1,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, ProductionKirToLlvmReplayErrorV1> {
        match tag {
            KIR_V8_TAG_V1 => Ok(Self::V8),
            KIR_V9_TAG_V1 => Ok(Self::V9),
            _ => Err(ProductionKirToLlvmReplayErrorV1::InvalidHeader),
        }
    }
}

/// Versioned identity of exact canonical Kernel IR bytes used by replay.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionReplayKernelIrIdentityV1 {
    version: ProductionReplayKernelIrVersionV1,
    sha256: [u8; 32],
    byte_len: u64,
}

/// LLVM lowering variant admitted by frozen replay V1 bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionKirToLlvmReplayModeV1 {
    LegacyUninstrumented,
    SemanticAnchorsV1,
}

impl ProductionReplayKernelIrIdentityV1 {
    pub const fn version(self) -> ProductionReplayKernelIrVersionV1 {
        self.version
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Content identity of one canonical replay-evidence record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionKirToLlvmReplayEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ProductionKirToLlvmReplayEvidenceIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Inert canonical evidence for deterministic production KIR-to-LLVM replay.
///
/// Decoding establishes bounded canonical structure only. Call
/// [`validate_against_neutral_kernel_ir`](Self::validate_against_neutral_kernel_ir)
/// to independently reconstruct target-bound KIR and exact LLVM.
#[derive(Debug, Eq, PartialEq)]
pub struct CanonicalProductionKirToLlvmReplayEvidenceV1 {
    canonical_bytes: Box<[u8]>,
    identity: ProductionKirToLlvmReplayEvidenceIdentityV1,
    profile: ProductionAmdTargetProfileV1,
    neutral_kernel_ir: ProductionReplayKernelIrIdentityV1,
    target_bound_kernel_ir: ProductionReplayKernelIrIdentityV1,
    kernel_ids: Box<[KernelId]>,
    pre_descriptor_llvm: Box<str>,
}

impl CanonicalProductionKirToLlvmReplayEvidenceV1 {
    /// Constructs evidence only after replaying and comparing every live input.
    pub fn from_live_inputs(
        neutral_kernel_ir: &[u8],
        target_bound_module: &Module,
        profile: ProductionAmdTargetProfileV1,
        pre_descriptor_llvm: &str,
    ) -> Result<Self, ProductionKirToLlvmReplayErrorV1> {
        let version = infer_kernel_ir_version(neutral_kernel_ir)?;
        let (_, neutral_module, neutral_identity) =
            decode_exact_kernel_ir(neutral_kernel_ir, version)?;
        let target_bound = bind_production_target_v1(&neutral_module, profile)
            .map_err(ProductionKirToLlvmReplayErrorV1::TargetBinding)?;
        if target_bound.module() != target_bound_module {
            return Err(ProductionKirToLlvmReplayErrorV1::LiveTargetModuleMismatch);
        }
        let kernel_ids = target_bound.kernel_ids();
        let (target_owner, target_identity) =
            canonicalize_target_module(target_bound.module(), version)?;
        classify_replay_llvm(
            target_bound.module(),
            kernel_ids,
            profile,
            target_owner.semantic_anchor_identity(),
            pre_descriptor_llvm,
        )?;

        let canonical_bytes = encode_evidence(
            profile,
            neutral_identity,
            target_identity,
            kernel_ids,
            pre_descriptor_llvm,
        )?;
        let evidence = Self::decode(&canonical_bytes)?;
        let validated = evidence.validate_against_neutral_kernel_ir(neutral_kernel_ir)?;
        if validated.target_bound_module() != target_bound_module {
            return Err(ProductionKirToLlvmReplayErrorV1::LiveTargetModuleMismatch);
        }
        Ok(validated.into_evidence())
    }

    /// Strictly decodes and byte-for-byte re-encodes one bounded evidence record.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionKirToLlvmReplayErrorV1> {
        if bytes.len() > MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1 {
            return Err(ProductionKirToLlvmReplayErrorV1::TooLarge);
        }
        let mut reader = Reader::new(bytes);
        if reader.take(EVIDENCE_MAGIC_V1.len())? != EVIDENCE_MAGIC_V1 {
            return Err(ProductionKirToLlvmReplayErrorV1::InvalidHeader);
        }
        let version = reader.u16()?;
        if !matches!(version, EVIDENCE_VERSION_V1 | EVIDENCE_VERSION_V2)
            || reader.u8()? != EXACT_DETERMINISTIC_REPLAY_CLAIM_V1
        {
            return Err(ProductionKirToLlvmReplayErrorV1::InvalidHeader);
        }
        let profile = decode_profile(reader.u8()?)?;
        let kernel_ir_version = ProductionReplayKernelIrVersionV1::from_tag(reader.u8()?)?;
        if reader.u8()? != RESERVED_V1 {
            return Err(ProductionKirToLlvmReplayErrorV1::InvalidHeader);
        }
        let neutral_kernel_ir = decode_kernel_ir_identity(&mut reader, kernel_ir_version)?;
        let target_bound_kernel_ir = decode_kernel_ir_identity(&mut reader, kernel_ir_version)?;
        let kernel_count = if version == EVIDENCE_VERSION_V1 {
            1
        } else {
            reader.usize_u32()?
        };
        const MIN_KERNEL_ID_FRAME_BYTES_V1: usize = 5;
        const MIN_TRAILING_LLVM_FRAME_BYTES_V1: usize = 5;
        let minimum_framing = kernel_count
            .checked_mul(MIN_KERNEL_ID_FRAME_BYTES_V1)
            .and_then(|length| length.checked_add(MIN_TRAILING_LLVM_FRAME_BYTES_V1))
            .ok_or(ProductionKirToLlvmReplayErrorV1::InvalidLength)?;
        if kernel_count == 0
            || kernel_count > MAX_KERNELS_V1
            || minimum_framing > reader.remaining()
        {
            return Err(ProductionKirToLlvmReplayErrorV1::InvalidLength);
        }
        let mut kernel_ids = Vec::new();
        kernel_ids
            .try_reserve_exact(kernel_count)
            .map_err(|_| ProductionKirToLlvmReplayErrorV1::InvalidLength)?;
        let mut seen_kernel_ids = BTreeSet::new();
        for _ in 0..kernel_count {
            let kernel_id_length = reader.usize_u32()?;
            if kernel_id_length == 0 || kernel_id_length > MAX_TEXT_BYTES_V1 {
                return Err(ProductionKirToLlvmReplayErrorV1::InvalidLength);
            }
            let kernel_id = std::str::from_utf8(reader.take(kernel_id_length)?)
                .map_err(|_| ProductionKirToLlvmReplayErrorV1::InvalidUtf8)?;
            if !seen_kernel_ids.insert(kernel_id) {
                return Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch);
            }
            kernel_ids.push(KernelId::new(kernel_id));
        }
        let llvm_length = reader.usize_u32()?;
        if llvm_length == 0 || llvm_length > MAX_PRODUCTION_PRE_DESCRIPTOR_LLVM_BYTES_V1 {
            return Err(ProductionKirToLlvmReplayErrorV1::InvalidLength);
        }
        let pre_descriptor_llvm = std::str::from_utf8(reader.take(llvm_length)?)
            .map_err(|_| ProductionKirToLlvmReplayErrorV1::InvalidUtf8)?;
        if pre_descriptor_llvm.as_bytes().contains(&0) {
            return Err(ProductionKirToLlvmReplayErrorV1::InvalidUtf8);
        }
        reader.finish()?;

        let pre_descriptor_llvm = try_owned_string(pre_descriptor_llvm)?;

        let canonical_bytes = encode_evidence(
            profile,
            neutral_kernel_ir,
            target_bound_kernel_ir,
            &kernel_ids,
            &pre_descriptor_llvm,
        )?;
        if canonical_bytes.as_slice() != bytes {
            return Err(ProductionKirToLlvmReplayErrorV1::NonCanonical);
        }
        let identity = evidence_identity(&canonical_bytes);
        Ok(Self {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            identity,
            profile,
            neutral_kernel_ir,
            target_bound_kernel_ir,
            kernel_ids: kernel_ids.into_boxed_slice(),
            pre_descriptor_llvm: pre_descriptor_llvm.into_boxed_str(),
        })
    }

    /// Reconstructs target-bound KIR and LLVM from independently supplied neutral KIR.
    pub fn validate_against_neutral_kernel_ir(
        self,
        neutral_kernel_ir: &[u8],
    ) -> Result<ValidatedProductionKirToLlvmReplayV1, ProductionKirToLlvmReplayErrorV1> {
        let version = self.neutral_kernel_ir.version;
        let (neutral_owner, neutral_module, neutral_identity) =
            decode_exact_kernel_ir(neutral_kernel_ir, version)?;
        if neutral_identity != self.neutral_kernel_ir {
            return Err(ProductionKirToLlvmReplayErrorV1::IdentityMismatch {
                field: "neutral Kernel IR",
            });
        }

        let target_bound = bind_production_target_v1(&neutral_module, self.profile)
            .map_err(ProductionKirToLlvmReplayErrorV1::TargetBinding)?;
        if target_bound.kernel_ids() != self.kernel_ids.as_ref() {
            return Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch);
        }
        let (target_owner, target_identity) =
            canonicalize_target_module(target_bound.module(), version)?;
        if target_identity != self.target_bound_kernel_ir {
            return Err(ProductionKirToLlvmReplayErrorV1::IdentityMismatch {
                field: "target-bound Kernel IR",
            });
        }
        let llvm_mode = classify_replay_llvm(
            target_bound.module(),
            &self.kernel_ids,
            self.profile,
            target_owner.semantic_anchor_identity(),
            &self.pre_descriptor_llvm,
        )?;
        let structural_binding = target_bound
            .admit_exact_structural_binding_v1(&neutral_module, neutral_identity, target_identity)
            .map_err(|_| ProductionKirToLlvmReplayErrorV1::IdentityMismatch {
                field: "target structural coordinate binding",
            })?;
        let semantic_binding = validate_production_target_semantic_binding_v1(
            &neutral_module,
            &target_bound,
            self.profile,
        )
        .map_err(ProductionKirToLlvmReplayErrorV1::TargetSemanticValidation)?;
        let (target_bound_module, _) = target_bound.into_parts();
        Ok(ValidatedProductionKirToLlvmReplayV1 {
            evidence: self,
            llvm_mode,
            neutral_owner,
            target_owner,
            target_bound_module,
            structural_binding,
            semantic_binding,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> ProductionKirToLlvmReplayEvidenceIdentityV1 {
        self.identity
    }

    pub const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }

    pub const fn neutral_kernel_ir_identity(&self) -> ProductionReplayKernelIrIdentityV1 {
        self.neutral_kernel_ir
    }

    pub const fn target_bound_kernel_ir_identity(&self) -> ProductionReplayKernelIrIdentityV1 {
        self.target_bound_kernel_ir
    }

    /// Returns every replay-bound kernel identity in canonical module order.
    pub fn kernel_ids(&self) -> &[KernelId] {
        &self.kernel_ids
    }

    pub fn pre_descriptor_llvm(&self) -> &str {
        &self.pre_descriptor_llvm
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Independently reconstructed exact production KIR-to-LLVM derivation.
#[derive(Debug)]
#[must_use = "dropping validated replay abandons exact KIR-to-LLVM custody"]
pub struct ValidatedProductionKirToLlvmReplayV1 {
    evidence: CanonicalProductionKirToLlvmReplayEvidenceV1,
    llvm_mode: ProductionKirToLlvmReplayModeV1,
    neutral_owner: ExactKernelIrOwnerV1,
    target_owner: ExactKernelIrOwnerV1,
    target_bound_module: Module,
    structural_binding: ProductionTargetStructuralBindingV1,
    semantic_binding: ValidatedProductionTargetSemanticBindingV1,
}

impl ValidatedProductionKirToLlvmReplayV1 {
    pub const fn evidence(&self) -> &CanonicalProductionKirToLlvmReplayEvidenceV1 {
        &self.evidence
    }

    pub const fn llvm_mode(&self) -> ProductionKirToLlvmReplayModeV1 {
        self.llvm_mode
    }

    pub fn neutral_kernel_ir_bytes(&self) -> &[u8] {
        self.neutral_owner.canonical_bytes()
    }

    pub fn target_bound_kernel_ir_bytes(&self) -> &[u8] {
        self.target_owner.canonical_bytes()
    }

    pub const fn target_bound_module(&self) -> &Module {
        &self.target_bound_module
    }

    pub const fn structural_binding(&self) -> ProductionTargetStructuralBindingV1 {
        self.structural_binding
    }

    pub const fn semantic_binding(&self) -> &ValidatedProductionTargetSemanticBindingV1 {
        &self.semantic_binding
    }

    pub const fn has_exact_target_binding_replay(&self) -> bool {
        true
    }

    pub const fn has_exact_kir_to_llvm_replay(&self) -> bool {
        true
    }

    pub const fn establishes_formal_semantic_refinement(&self) -> bool {
        false
    }

    pub const fn grants_object_or_runtime_authority(&self) -> bool {
        false
    }

    pub fn into_evidence(self) -> CanonicalProductionKirToLlvmReplayEvidenceV1 {
        self.evidence
    }
}

#[derive(Debug)]
enum ExactKernelIrOwnerV1 {
    V8(VerifiedCanonicalKernelIrV8),
    V9(VerifiedCanonicalKernelIrV9),
}

impl ExactKernelIrOwnerV1 {
    fn canonical_bytes(&self) -> &[u8] {
        match self {
            Self::V8(owner) => owner.canonical_bytes(),
            Self::V9(owner) => owner.canonical_bytes(),
        }
    }

    fn identity(&self) -> ProductionReplayKernelIrIdentityV1 {
        match self {
            Self::V8(owner) => ProductionReplayKernelIrIdentityV1 {
                version: ProductionReplayKernelIrVersionV1::V8,
                sha256: *owner.identity().digest(),
                byte_len: owner.identity().canonical_length(),
            },
            Self::V9(owner) => ProductionReplayKernelIrIdentityV1 {
                version: ProductionReplayKernelIrVersionV1::V9,
                sha256: *owner.identity().digest(),
                byte_len: owner.identity().canonical_length(),
            },
        }
    }

    fn semantic_anchor_identity(&self) -> ProductionSemanticAnchorKirIdentityV1 {
        match self {
            Self::V8(owner) => ProductionSemanticAnchorKirIdentityV1::from_v8(owner),
            Self::V9(owner) => ProductionSemanticAnchorKirIdentityV1::from_v9(owner),
        }
    }
}

fn infer_kernel_ir_version(
    bytes: &[u8],
) -> Result<ProductionReplayKernelIrVersionV1, ProductionKirToLlvmReplayErrorV1> {
    if bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(ProductionKirToLlvmReplayErrorV1::TooLarge);
    }
    if bytes.get(..KERNEL_IR_MAGIC_V1.len()) != Some(KERNEL_IR_MAGIC_V1.as_slice()) {
        return Err(ProductionKirToLlvmReplayErrorV1::InvalidKernelIrHeader);
    }
    let version_offset = KERNEL_IR_MAGIC_V1.len();
    let version_bytes = bytes
        .get(version_offset..version_offset + 2)
        .ok_or(ProductionKirToLlvmReplayErrorV1::Truncated)?;
    match u16::from_le_bytes([version_bytes[0], version_bytes[1]]) {
        KERNEL_IR_VERSION_V8 => Ok(ProductionReplayKernelIrVersionV1::V8),
        KERNEL_IR_VERSION_V9 => Ok(ProductionReplayKernelIrVersionV1::V9),
        _ => Err(ProductionKirToLlvmReplayErrorV1::InvalidKernelIrHeader),
    }
}

fn decode_exact_kernel_ir(
    bytes: &[u8],
    version: ProductionReplayKernelIrVersionV1,
) -> Result<
    (
        ExactKernelIrOwnerV1,
        Module,
        ProductionReplayKernelIrIdentityV1,
    ),
    ProductionKirToLlvmReplayErrorV1,
> {
    let canonical_bytes = try_owned_bytes(bytes)?;
    let (owner, module) = match version {
        ProductionReplayKernelIrVersionV1::V8 => {
            let (owner, module) =
                VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(canonical_bytes)
                    .map_err(ProductionKirToLlvmReplayErrorV1::KernelIrV8)?;
            (ExactKernelIrOwnerV1::V8(owner), module)
        }
        ProductionReplayKernelIrVersionV1::V9 => {
            let (owner, module) =
                VerifiedCanonicalKernelIrV9::from_canonical_bytes_with_module(canonical_bytes)
                    .map_err(ProductionKirToLlvmReplayErrorV1::KernelIrV9)?;
            (ExactKernelIrOwnerV1::V9(owner), module)
        }
    };
    let identity = owner.identity();
    Ok((owner, module, identity))
}

fn canonicalize_target_module(
    module: &Module,
    version: ProductionReplayKernelIrVersionV1,
) -> Result<
    (ExactKernelIrOwnerV1, ProductionReplayKernelIrIdentityV1),
    ProductionKirToLlvmReplayErrorV1,
> {
    let owner = match version {
        ProductionReplayKernelIrVersionV1::V8 => ExactKernelIrOwnerV1::V8(
            VerifiedCanonicalKernelIrV8::from_module(module.clone())
                .map_err(ProductionKirToLlvmReplayErrorV1::KernelIrV8)?,
        ),
        ProductionReplayKernelIrVersionV1::V9 => ExactKernelIrOwnerV1::V9(
            VerifiedCanonicalKernelIrV9::from_module(module.clone())
                .map_err(ProductionKirToLlvmReplayErrorV1::KernelIrV9)?,
        ),
    };
    let identity = owner.identity();
    Ok((owner, identity))
}

fn replay_llvm(
    target_bound_module: &Module,
    profile: ProductionAmdTargetProfileV1,
    mode: ProductionKirToLlvmReplayModeV1,
    target_kir_identity: ProductionSemanticAnchorKirIdentityV1,
) -> Result<String, ProductionKirToLlvmReplayErrorV1> {
    let dialect_llvm = match (profile, mode) {
        (
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
        ) => lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(target_bound_module),
        (
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
        ) => lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            target_bound_module,
            target_kir_identity,
        ),
        (
            ProductionAmdTargetProfileV1::Gfx950,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
        ) => lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(target_bound_module),
        (
            ProductionAmdTargetProfileV1::Gfx950,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
        ) => lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            target_bound_module,
            target_kir_identity,
        ),
    }
    .map_err(ProductionKirToLlvmReplayErrorV1::TargetLowering)?;
    match mode {
        ProductionKirToLlvmReplayModeV1::LegacyUninstrumented => {
            bind_historical_replay_llvm_layout_v1(&dialect_llvm)
        }
        ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1 => {
            bind_production_llvm22_worker_layout_v1(&dialect_llvm)
        }
    }
    .map_err(ProductionKirToLlvmReplayErrorV1::LayoutBinding)
}

fn classify_replay_llvm(
    target_bound_module: &Module,
    kernel_ids: &[KernelId],
    profile: ProductionAmdTargetProfileV1,
    target_kir_identity: ProductionSemanticAnchorKirIdentityV1,
    expected: &str,
) -> Result<ProductionKirToLlvmReplayModeV1, ProductionKirToLlvmReplayErrorV1> {
    if let [kernel_id] = kernel_ids {
        let historical_legacy = replay_historical_kernel_llvm(
            target_bound_module,
            kernel_id,
            profile,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
            target_kir_identity,
        )?;
        if historical_legacy.as_bytes() == expected.as_bytes() {
            return Ok(ProductionKirToLlvmReplayModeV1::LegacyUninstrumented);
        }
    }
    let legacy_matches = {
        let legacy = replay_llvm(
            target_bound_module,
            profile,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
            target_kir_identity,
        )?;
        legacy.as_bytes() == expected.as_bytes()
    };
    if legacy_matches {
        return Ok(ProductionKirToLlvmReplayModeV1::LegacyUninstrumented);
    }
    if let [kernel_id] = kernel_ids {
        let historical_anchored = replay_historical_kernel_llvm(
            target_bound_module,
            kernel_id,
            profile,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
            target_kir_identity,
        )?;
        if historical_anchored.as_bytes() == expected.as_bytes() {
            return Ok(ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1);
        }
    }
    let anchored = replay_llvm(
        target_bound_module,
        profile,
        ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
        target_kir_identity,
    )?;
    if anchored.as_bytes() == expected.as_bytes() {
        Ok(ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1)
    } else {
        Err(ProductionKirToLlvmReplayErrorV1::LlvmMismatch)
    }
}

fn replay_historical_kernel_llvm(
    target_bound_module: &Module,
    kernel_id: &KernelId,
    profile: ProductionAmdTargetProfileV1,
    mode: ProductionKirToLlvmReplayModeV1,
    target_kir_identity: ProductionSemanticAnchorKirIdentityV1,
) -> Result<String, ProductionKirToLlvmReplayErrorV1> {
    let dialect_llvm = match (profile, mode) {
        (
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
        ) => lower_kernel_to_gfx942_xnack_minus_replay_llvm_ir_v1(target_bound_module, kernel_id),
        (
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
        ) => lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            target_bound_module,
            kernel_id,
            target_kir_identity,
        ),
        (
            ProductionAmdTargetProfileV1::Gfx950,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
        ) => lower_kernel_to_gfx950_xnack_minus_replay_llvm_ir_v1(target_bound_module, kernel_id),
        (
            ProductionAmdTargetProfileV1::Gfx950,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
        ) => lower_kernel_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            target_bound_module,
            kernel_id,
            target_kir_identity,
        ),
    }
    .map_err(ProductionKirToLlvmReplayErrorV1::TargetLowering)?;
    match mode {
        ProductionKirToLlvmReplayModeV1::LegacyUninstrumented => {
            bind_historical_replay_llvm_layout_v1(&dialect_llvm)
        }
        ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1 => {
            bind_production_llvm22_worker_layout_v1(&dialect_llvm)
        }
    }
    .map_err(ProductionKirToLlvmReplayErrorV1::LayoutBinding)
}

fn encode_evidence(
    profile: ProductionAmdTargetProfileV1,
    neutral_kernel_ir: ProductionReplayKernelIrIdentityV1,
    target_bound_kernel_ir: ProductionReplayKernelIrIdentityV1,
    kernel_ids: &[KernelId],
    pre_descriptor_llvm: &str,
) -> Result<Vec<u8>, ProductionKirToLlvmReplayErrorV1> {
    if neutral_kernel_ir.version != target_bound_kernel_ir.version
        || neutral_kernel_ir.byte_len == 0
        || target_bound_kernel_ir.byte_len == 0
        || neutral_kernel_ir.byte_len > MAX_MODULE_BYTES_V1 as u64
        || target_bound_kernel_ir.byte_len > MAX_MODULE_BYTES_V1 as u64
        || neutral_kernel_ir.sha256 == [0; 32]
        || target_bound_kernel_ir.sha256 == [0; 32]
        || kernel_ids.is_empty()
        || kernel_ids.len() > MAX_KERNELS_V1
        || kernel_ids.iter().any(|kernel_id| {
            kernel_id.as_str().is_empty() || kernel_id.as_str().len() > MAX_TEXT_BYTES_V1
        })
        || kernel_ids.iter().collect::<BTreeSet<_>>().len() != kernel_ids.len()
        || pre_descriptor_llvm.is_empty()
        || pre_descriptor_llvm.len() > MAX_PRODUCTION_PRE_DESCRIPTOR_LLVM_BYTES_V1
        || pre_descriptor_llvm.as_bytes().contains(&0)
    {
        return Err(ProductionKirToLlvmReplayErrorV1::InvalidLength);
    }
    let kernel_bytes = kernel_ids.iter().try_fold(0_usize, |total, kernel_id| {
        total
            .checked_add(4)
            .and_then(|value| value.checked_add(kernel_id.as_str().len()))
            .ok_or(ProductionKirToLlvmReplayErrorV1::Overflow)
    })?;
    let roster_header = usize::from(kernel_ids.len() != 1) * 4;
    let total = EVIDENCE_MAGIC_V1
        .len()
        .checked_add(2 + 4 + 32 + 8 + 32 + 8 + 4 + 4)
        .and_then(|value| value.checked_add(roster_header))
        .and_then(|value| value.checked_add(kernel_bytes.saturating_sub(4)))
        .and_then(|value| value.checked_add(pre_descriptor_llvm.len()))
        .ok_or(ProductionKirToLlvmReplayErrorV1::Overflow)?;
    if total > MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1 {
        return Err(ProductionKirToLlvmReplayErrorV1::TooLarge);
    }
    let llvm_length = u32::try_from(pre_descriptor_llvm.len())
        .map_err(|_| ProductionKirToLlvmReplayErrorV1::Overflow)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| ProductionKirToLlvmReplayErrorV1::AllocationFailure)?;
    bytes.extend_from_slice(EVIDENCE_MAGIC_V1);
    let evidence_version = if kernel_ids.len() == 1 {
        EVIDENCE_VERSION_V1
    } else {
        EVIDENCE_VERSION_V2
    };
    bytes.extend_from_slice(&evidence_version.to_le_bytes());
    bytes.push(EXACT_DETERMINISTIC_REPLAY_CLAIM_V1);
    bytes.push(encode_profile(profile));
    bytes.push(neutral_kernel_ir.version.tag());
    bytes.push(RESERVED_V1);
    encode_kernel_ir_identity(&mut bytes, neutral_kernel_ir);
    encode_kernel_ir_identity(&mut bytes, target_bound_kernel_ir);
    if evidence_version == EVIDENCE_VERSION_V2 {
        bytes.extend_from_slice(
            &u32::try_from(kernel_ids.len())
                .map_err(|_| ProductionKirToLlvmReplayErrorV1::Overflow)?
                .to_le_bytes(),
        );
    }
    for kernel_id in kernel_ids {
        bytes.extend_from_slice(
            &u32::try_from(kernel_id.as_str().len())
                .map_err(|_| ProductionKirToLlvmReplayErrorV1::Overflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(kernel_id.as_str().as_bytes());
    }
    bytes.extend_from_slice(&llvm_length.to_le_bytes());
    bytes.extend_from_slice(pre_descriptor_llvm.as_bytes());
    if bytes.len() != total {
        return Err(ProductionKirToLlvmReplayErrorV1::InvalidLength);
    }
    Ok(bytes)
}

fn try_owned_bytes(bytes: &[u8]) -> Result<Vec<u8>, ProductionKirToLlvmReplayErrorV1> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| ProductionKirToLlvmReplayErrorV1::AllocationFailure)?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

fn try_owned_string(value: &str) -> Result<String, ProductionKirToLlvmReplayErrorV1> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| ProductionKirToLlvmReplayErrorV1::AllocationFailure)?;
    owned.push_str(value);
    Ok(owned)
}

const fn encode_profile(profile: ProductionAmdTargetProfileV1) -> u8 {
    match profile {
        ProductionAmdTargetProfileV1::Gfx942 => GFX942_PROFILE_TAG_V1,
        ProductionAmdTargetProfileV1::Gfx950 => GFX950_PROFILE_TAG_V1,
    }
}

fn decode_profile(
    tag: u8,
) -> Result<ProductionAmdTargetProfileV1, ProductionKirToLlvmReplayErrorV1> {
    match tag {
        GFX942_PROFILE_TAG_V1 => Ok(ProductionAmdTargetProfileV1::Gfx942),
        GFX950_PROFILE_TAG_V1 => Ok(ProductionAmdTargetProfileV1::Gfx950),
        _ => Err(ProductionKirToLlvmReplayErrorV1::InvalidHeader),
    }
}

fn encode_kernel_ir_identity(bytes: &mut Vec<u8>, identity: ProductionReplayKernelIrIdentityV1) {
    bytes.extend_from_slice(&identity.sha256);
    bytes.extend_from_slice(&identity.byte_len.to_le_bytes());
}

fn decode_kernel_ir_identity(
    reader: &mut Reader<'_>,
    version: ProductionReplayKernelIrVersionV1,
) -> Result<ProductionReplayKernelIrIdentityV1, ProductionKirToLlvmReplayErrorV1> {
    let sha256 = reader.fixed::<32>()?;
    let byte_len = reader.u64()?;
    if sha256 == [0; 32] || byte_len == 0 || byte_len > MAX_MODULE_BYTES_V1 as u64 {
        return Err(ProductionKirToLlvmReplayErrorV1::InvalidLength);
    }
    Ok(ProductionReplayKernelIrIdentityV1 {
        version,
        sha256,
        byte_len,
    })
}

fn evidence_identity(bytes: &[u8]) -> ProductionKirToLlvmReplayEvidenceIdentityV1 {
    let mut digest = Sha256::new();
    digest.update((IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    ProductionKirToLlvmReplayEvidenceIdentityV1 {
        sha256: digest.finalize().into(),
        byte_len: bytes.len() as u64,
    }
}

/// Closed failures for canonical replay evidence and independent reconstruction.
#[derive(Debug)]
pub enum ProductionKirToLlvmReplayErrorV1 {
    TooLarge,
    AllocationFailure,
    Overflow,
    Truncated,
    TrailingBytes,
    InvalidHeader,
    InvalidKernelIrHeader,
    InvalidLength,
    InvalidUtf8,
    NonCanonical,
    IdentityMismatch { field: &'static str },
    KernelIdMismatch,
    LiveTargetModuleMismatch,
    LlvmMismatch,
    KernelIrV8(VerifiedCanonicalKernelIrErrorV8),
    KernelIrV9(VerifiedCanonicalKernelIrErrorV9),
    TargetBinding(ProductionTargetBindingErrorV1),
    TargetSemanticValidation(ProductionTargetSemanticValidationErrorV1),
    TargetLowering(LoweringErrors),
    LayoutBinding(ProductionLlvmLayoutBindingErrorV1),
}

impl fmt::Display for ProductionKirToLlvmReplayErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter
                .write_str("production KIR-to-LLVM replay evidence exceeds its hard byte bound"),
            Self::AllocationFailure => formatter
                .write_str("production KIR-to-LLVM replay allocation failed within its hard bound"),
            Self::Overflow => {
                formatter.write_str("production KIR-to-LLVM replay evidence length overflowed")
            }
            Self::Truncated => {
                formatter.write_str("production KIR-to-LLVM replay evidence is truncated")
            }
            Self::TrailingBytes => {
                formatter.write_str("production KIR-to-LLVM replay evidence has trailing bytes")
            }
            Self::InvalidHeader => {
                formatter.write_str("production KIR-to-LLVM replay evidence has an invalid header")
            }
            Self::InvalidKernelIrHeader => formatter.write_str(
                "production KIR-to-LLVM replay input is not exact canonical KIR V8 or V9",
            ),
            Self::InvalidLength => formatter
                .write_str("production KIR-to-LLVM replay evidence has an invalid bounded length"),
            Self::InvalidUtf8 => {
                formatter.write_str("production KIR-to-LLVM replay evidence contains invalid text")
            }
            Self::NonCanonical => {
                formatter.write_str("production KIR-to-LLVM replay evidence is not canonical")
            }
            Self::IdentityMismatch { field } => write!(
                formatter,
                "production KIR-to-LLVM replay changed the exact {field} identity"
            ),
            Self::KernelIdMismatch => formatter
                .write_str("production KIR-to-LLVM replay changed the exact kernel identity"),
            Self::LiveTargetModuleMismatch => formatter
                .write_str("live target-bound KIR differs from deterministic target replay"),
            Self::LlvmMismatch => {
                formatter.write_str("retained LLVM differs from deterministic KIR lowering replay")
            }
            Self::KernelIrV8(error) => write!(formatter, "exact KIR V8 validation failed: {error}"),
            Self::KernelIrV9(error) => write!(formatter, "exact KIR V9 validation failed: {error}"),
            Self::TargetBinding(error) => {
                write!(formatter, "target-binding replay failed: {error}")
            }
            Self::TargetSemanticValidation(error) => {
                write!(formatter, "target semantic validation failed: {error}")
            }
            Self::TargetLowering(error) => {
                write!(formatter, "AMDGPU lowering replay failed: {error}")
            }
            Self::LayoutBinding(error) => {
                write!(formatter, "upstream LLVM layout replay failed: {error}")
            }
        }
    }
}

impl Error for ProductionKirToLlvmReplayErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::KernelIrV8(error) => Some(error),
            Self::KernelIrV9(error) => Some(error),
            Self::TargetBinding(error) => Some(error),
            Self::TargetSemanticValidation(error) => Some(error),
            Self::TargetLowering(error) => Some(error),
            Self::LayoutBinding(error) => Some(error),
            _ => None,
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionKirToLlvmReplayErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionKirToLlvmReplayErrorV1::Overflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionKirToLlvmReplayErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ProductionKirToLlvmReplayErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionKirToLlvmReplayErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProductionKirToLlvmReplayErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionKirToLlvmReplayErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionKirToLlvmReplayErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionKirToLlvmReplayErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn usize_u32(&mut self) -> Result<usize, ProductionKirToLlvmReplayErrorV1> {
        usize::try_from(self.u32()?).map_err(|_| ProductionKirToLlvmReplayErrorV1::Overflow)
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn finish(self) -> Result<(), ProductionKirToLlvmReplayErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProductionKirToLlvmReplayErrorV1::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Constant, Function, LaunchDomain, LaunchExtent, Module, Operation,
        OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
        VerifiedCanonicalKernelIrV8, WorkgroupSize,
    };

    use super::*;

    fn neutral_module(name: &str) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::kernel_entry(
            format!("{name}_entry"),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            format!("{name}_kernel"),
            format!("{name}_entry"),
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new(name);
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    // This matches the exact no-operation fixture in the pre-anchor parent revision. Keep it
    // separate from `neutral_module`, whose operation is needed by active anchor tests.
    fn historical_neutral_module(name: &str) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::kernel_entry(
            format!("{name}_entry"),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            format!("{name}_kernel"),
            format!("{name}_entry"),
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new(name);
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    fn neutral_module_above_anchor_limit(name: &str) -> Module {
        let mut module = historical_neutral_module(name);
        let operations = (0..=crate::MAX_PRODUCTION_SEMANTIC_ANCHORS_V1)
            .map(|index| {
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(u32::try_from(index).unwrap()),
                        Type::Scalar(ScalarType::U32),
                    ),
                    OperationKind::Constant(Constant::U32(7)),
                )
            })
            .collect();
        module.functions[0].body.as_mut().unwrap().blocks[0].operations = operations;
        module
    }

    fn multi_neutral_module(name: &str, count: usize) -> Module {
        let mut module = Module::new(name);
        for index in 0..count {
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: vec![] });
            let entry = format!("{name}_entry_{index}");
            module.functions.push(Function::kernel_entry(
                entry.clone(),
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
            let mut kernel = fe2o3_kernel_ir::Kernel::new(
                format!("{name}_kernel_{index}"),
                entry,
                LaunchDomain::D1 {
                    x: LaunchExtent::Static(1),
                },
            );
            kernel.workgroup_size = Some(WorkgroupSize::new(64 << index, 1, 1));
            module.kernels.push(kernel);
        }
        module
    }

    fn fixture(
        name: &str,
    ) -> (
        Vec<u8>,
        Module,
        String,
        CanonicalProductionKirToLlvmReplayEvidenceV1,
    ) {
        fixture_with_mode(name, ProductionKirToLlvmReplayModeV1::LegacyUninstrumented)
    }

    fn fixture_with_mode(
        name: &str,
        mode: ProductionKirToLlvmReplayModeV1,
    ) -> (
        Vec<u8>,
        Module,
        String,
        CanonicalProductionKirToLlvmReplayEvidenceV1,
    ) {
        let neutral = VerifiedCanonicalKernelIrV8::from_module(neutral_module(name)).unwrap();
        let neutral_bytes = neutral.into_canonical_bytes();
        let (_, neutral_module, _) =
            decode_exact_kernel_ir(&neutral_bytes, ProductionReplayKernelIrVersionV1::V8).unwrap();
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV8::from_module(target.module().clone()).unwrap();
        let llvm = replay_llvm(
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            mode,
            ProductionSemanticAnchorKirIdentityV1::from_v8(&target_owner),
        )
        .unwrap();
        let target_module = target.module().clone();
        let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            &neutral_bytes,
            &target_module,
            ProductionAmdTargetProfileV1::Gfx942,
            &llvm,
        )
        .unwrap();
        (neutral_bytes, target_module, llvm, evidence)
    }

    #[test]
    fn exact_replay_round_trips_and_retains_no_later_authority() {
        let (neutral, target_module, llvm, evidence) = fixture("replay");
        let identity = evidence.identity();
        let decoded =
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(evidence.canonical_bytes())
                .unwrap();
        assert_eq!(decoded.identity(), identity);
        assert_eq!(decoded.pre_descriptor_llvm(), llvm);
        assert!(!decoded.grants_runtime_authority());
        let validated = decoded
            .validate_against_neutral_kernel_ir(&neutral)
            .unwrap();
        assert_eq!(validated.target_bound_module(), &target_module);
        assert_eq!(
            validated.llvm_mode(),
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented
        );
        let structure = validated.structural_binding();
        assert_eq!(structure.version(), ProductionReplayKernelIrVersionV1::V8);
        assert_eq!(structure.neutral_kernel_ir(), evidence.neutral_kernel_ir);
        assert_eq!(
            structure.target_bound_kernel_ir(),
            evidence.target_bound_kernel_ir
        );
        assert_eq!(structure.counts().functions(), 1);
        assert_eq!(structure.counts().defined_bodies(), 1);
        assert_eq!(structure.counts().blocks(), 1);
        assert_eq!(structure.counts().operations(), 1);
        assert!(structure.preserves_function_block_operation_coordinates());
        assert!(!structure.proves_semantic_refinement());
        assert!(!structure.grants_runtime_authority());
        assert!(
            validated
                .semantic_binding()
                .matches_formal_target_binding_relation_v1()
        );
        assert!(!validated.semantic_binding().grants_later_stage_authority());
        assert!(validated.has_exact_target_binding_replay());
        assert!(validated.has_exact_kir_to_llvm_replay());
        assert!(!validated.establishes_formal_semantic_refinement());
        assert!(!validated.grants_object_or_runtime_authority());
    }

    #[test]
    fn frozen_pre_anchor_v1_bytes_remain_exact_legacy_replay() {
        const FROZEN: &[u8] =
            include_bytes!("../tests/fixtures/production-kir-to-llvm-replay-v1-legacy.bin");
        const FROZEN_SHA256: [u8; 32] = [
            0xe2, 0x83, 0x42, 0xea, 0x91, 0x8e, 0x0c, 0xd2, 0xdb, 0xef, 0xd4, 0x5b, 0x3a, 0xf5,
            0x22, 0x80, 0x6d, 0x12, 0xcd, 0x4c, 0xd9, 0xd4, 0x4e, 0xe4, 0xdd, 0x97, 0x86, 0x71,
            0x42, 0x78, 0x95, 0xf7,
        ];
        let neutral = VerifiedCanonicalKernelIrV8::from_module(historical_neutral_module("replay"))
            .unwrap()
            .into_canonical_bytes();
        assert_eq!(FROZEN.len(), 1_054);
        assert_eq!(<[u8; 32]>::from(Sha256::digest(FROZEN)), FROZEN_SHA256);

        let decoded = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(FROZEN).unwrap();
        assert!(
            decoded
                .pre_descriptor_llvm()
                .contains("target datalayout = \"e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32")
        );
        assert!(
            !decoded
                .pre_descriptor_llvm()
                .contains("fe2o3.semantic_anchor")
        );
        let validated = decoded
            .validate_against_neutral_kernel_ir(&neutral)
            .unwrap();
        assert_eq!(
            validated.llvm_mode(),
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented
        );
    }

    #[test]
    fn historical_kernel_only_anchored_v1_bytes_remain_exact_replay() {
        let neutral = VerifiedCanonicalKernelIrV8::from_module(neutral_module("historical_anchor"))
            .unwrap()
            .into_canonical_bytes();
        let (_, neutral_module, _) =
            decode_exact_kernel_ir(&neutral, ProductionReplayKernelIrVersionV1::V8).unwrap();
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV8::from_module(target.module().clone()).unwrap();
        let [kernel_id] = target.kernel_ids() else {
            panic!("historical replay fixture must contain one kernel");
        };
        let llvm = replay_historical_kernel_llvm(
            target.module(),
            kernel_id,
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
            ProductionSemanticAnchorKirIdentityV1::from_v8(&target_owner),
        )
        .unwrap();
        assert!(llvm.contains("!fe2o3.semantic_anchor.v1"));

        let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            &neutral,
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            &llvm,
        )
        .unwrap();
        assert_eq!(
            evidence
                .validate_against_neutral_kernel_ir(&neutral)
                .unwrap()
                .llvm_mode(),
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1
        );
    }

    #[test]
    fn exact_replay_classifies_anchored_bytes_without_changing_v1_encoding() {
        let (neutral, _, llvm, evidence) = fixture_with_mode(
            "anchored",
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
        );
        assert!(llvm.contains("!fe2o3.semantic_anchor.v1"));
        let validated = evidence
            .validate_against_neutral_kernel_ir(&neutral)
            .unwrap();
        assert_eq!(
            validated.llvm_mode(),
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1
        );

        let mut hybrid = llvm;
        hybrid = hybrid.replace(
            "!fe2o3.semantic_anchor.v1",
            "!fe2o3.semantic_anchor.hybrid.v1",
        );
        assert!(matches!(
            CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
                &neutral,
                validated.target_bound_module(),
                ProductionAmdTargetProfileV1::Gfx942,
                &hybrid,
            ),
            Err(ProductionKirToLlvmReplayErrorV1::LlvmMismatch)
        ));
    }

    #[test]
    fn replay_v1_text_limit_exactly_accounts_for_maximum_kernel_id() {
        assert_eq!(
            EVIDENCE_MAGIC_V1.len() + (2 + 4 + 32 + 8 + 32 + 8 + 4 + 4),
            crate::PRODUCTION_KIR_TO_LLVM_REPLAY_FIXED_BYTES_V1
        );
        let identity = ProductionReplayKernelIrIdentityV1 {
            version: ProductionReplayKernelIrVersionV1::V8,
            sha256: [7; 32],
            byte_len: 1,
        };
        let kernel_id = KernelId::new("k".repeat(MAX_TEXT_BYTES_V1));
        let maximum = "x".repeat(crate::MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1);
        let encoded = encode_evidence(
            ProductionAmdTargetProfileV1::Gfx942,
            identity,
            identity,
            std::slice::from_ref(&kernel_id),
            &maximum,
        )
        .unwrap();
        assert_eq!(
            encoded.len(),
            MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1
        );

        let short_kernel_id = KernelId::new("k");
        let historical_maximum = "x".repeat(MAX_PRODUCTION_PRE_DESCRIPTOR_LLVM_BYTES_V1);
        assert!(
            encode_evidence(
                ProductionAmdTargetProfileV1::Gfx942,
                identity,
                identity,
                std::slice::from_ref(&short_kernel_id),
                &historical_maximum,
            )
            .is_ok()
        );

        let over = "x".repeat(MAX_PRODUCTION_PRE_DESCRIPTOR_LLVM_BYTES_V1 + 1);
        assert!(matches!(
            encode_evidence(
                ProductionAmdTargetProfileV1::Gfx942,
                identity,
                identity,
                std::slice::from_ref(&short_kernel_id),
                &over,
            ),
            Err(ProductionKirToLlvmReplayErrorV1::InvalidLength)
        ));
    }

    #[test]
    fn exact_legacy_match_does_not_construct_over_limit_anchor_candidate() {
        let neutral_module = neutral_module_above_anchor_limit("large_legacy");
        let neutral = VerifiedCanonicalKernelIrV8::from_module(neutral_module.clone()).unwrap();
        let neutral_bytes = neutral.canonical_bytes();
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV8::from_module(target.module().clone()).unwrap();
        let anchor_identity = ProductionSemanticAnchorKirIdentityV1::from_v8(&target_owner);
        let legacy = replay_llvm(
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
            anchor_identity,
        )
        .unwrap();
        assert!(matches!(
            replay_llvm(
                target.module(),
                ProductionAmdTargetProfileV1::Gfx942,
                ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
                anchor_identity,
            ),
            Err(ProductionKirToLlvmReplayErrorV1::TargetLowering(_))
        ));
        let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            neutral_bytes,
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            &legacy,
        )
        .unwrap();
        assert_eq!(
            evidence
                .validate_against_neutral_kernel_ir(neutral_bytes)
                .unwrap()
                .llvm_mode(),
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented
        );
    }

    #[test]
    fn anchored_replay_retains_v9_for_a_graph_that_is_also_v8_representable() {
        let neutral_owner =
            VerifiedCanonicalKernelIrV9::from_module(neutral_module("v9_compatible")).unwrap();
        let neutral_bytes = neutral_owner.canonical_bytes();
        let (_, neutral_module, _) =
            decode_exact_kernel_ir(neutral_bytes, ProductionReplayKernelIrVersionV1::V9).unwrap();
        assert!(VerifiedCanonicalKernelIrV8::from_module(neutral_module.clone()).is_ok());
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV9::from_module(target.module().clone()).unwrap();
        let llvm = replay_llvm(
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
            ProductionSemanticAnchorKirIdentityV1::from_v9(&target_owner),
        )
        .unwrap();
        assert!(llvm.contains("!\"kir-version:9\""));
        let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            neutral_bytes,
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            &llvm,
        )
        .unwrap();
        assert_eq!(
            evidence.target_bound_kernel_ir_identity().version(),
            ProductionReplayKernelIrVersionV1::V9
        );
        assert_eq!(
            evidence
                .validate_against_neutral_kernel_ir(neutral_bytes)
                .unwrap()
                .llvm_mode(),
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1
        );
    }

    #[test]
    fn two_and_three_root_replay_retains_one_ordered_multi_entry_llvm_module() {
        for count in [2, 3] {
            let neutral = VerifiedCanonicalKernelIrV8::from_module(multi_neutral_module(
                "multi_replay",
                count,
            ))
            .unwrap();
            let neutral_bytes = neutral.into_canonical_bytes();
            let (_, neutral_module, _) =
                decode_exact_kernel_ir(&neutral_bytes, ProductionReplayKernelIrVersionV1::V8)
                    .unwrap();
            let target =
                bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                    .unwrap();
            let expected_ids = target.kernel_ids().to_vec();
            let target_owner =
                VerifiedCanonicalKernelIrV8::from_module(target.module().clone()).unwrap();
            let llvm = replay_llvm(
                target.module(),
                ProductionAmdTargetProfileV1::Gfx942,
                ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
                ProductionSemanticAnchorKirIdentityV1::from_v8(&target_owner),
            )
            .unwrap();
            let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
                &neutral_bytes,
                target.module(),
                ProductionAmdTargetProfileV1::Gfx942,
                &llvm,
            )
            .unwrap();
            assert_eq!(evidence.kernel_ids(), expected_ids);
            assert_eq!(
                u16::from_le_bytes(
                    evidence.canonical_bytes()
                        [EVIDENCE_MAGIC_V1.len()..EVIDENCE_MAGIC_V1.len() + 2]
                        .try_into()
                        .unwrap(),
                ),
                EVIDENCE_VERSION_V2,
            );
            let mut previous = 0;
            for index in 0..count {
                let entry = format!("multi_replay_kernel_{index}");
                let marker = format!("define amdgpu_kernel void @{entry}(");
                assert_eq!(llvm.matches(&marker).count(), 1);
                let offset = llvm.find(&marker).unwrap();
                assert!(offset >= previous);
                previous = offset;
            }
            let decoded =
                CanonicalProductionKirToLlvmReplayEvidenceV1::decode(evidence.canonical_bytes())
                    .unwrap();
            assert_eq!(decoded.kernel_ids(), expected_ids);
            assert_eq!(
                decoded
                    .validate_against_neutral_kernel_ir(&neutral_bytes)
                    .unwrap()
                    .target_bound_module()
                    .kernels
                    .len(),
                count,
            );
        }
    }

    #[test]
    fn multi_root_v2_replay_accepts_exact_full_module_semantic_anchors() {
        let neutral = VerifiedCanonicalKernelIrV9::from_module(multi_neutral_module(
            "multi_anchor_replay",
            3,
        ))
        .unwrap();
        let neutral_bytes = neutral.into_canonical_bytes();
        let (_, neutral_module, _) =
            decode_exact_kernel_ir(&neutral_bytes, ProductionReplayKernelIrVersionV1::V9).unwrap();
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV9::from_module(target.module().clone()).unwrap();
        let llvm = replay_llvm(
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
            ProductionSemanticAnchorKirIdentityV1::from_v9(&target_owner),
        )
        .unwrap();
        let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            &neutral_bytes,
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            &llvm,
        )
        .unwrap();

        assert_eq!(evidence.kernel_ids(), target.kernel_ids());
        assert!(llvm.contains("!\"kir-version:9\""));
        assert!(llvm.contains("!\"multiple_defined_bodies\""));
        assert_eq!(llvm.matches("!fe2o3.semantic_anchor.absence.v1").count(), 1,);
        assert_eq!(
            evidence
                .validate_against_neutral_kernel_ir(&neutral_bytes)
                .unwrap()
                .llvm_mode(),
            ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1,
        );
    }

    #[test]
    fn replay_v2_rejects_reordered_substituted_omitted_added_and_duplicate_rosters() {
        let neutral =
            VerifiedCanonicalKernelIrV8::from_module(multi_neutral_module("hostile_roster", 3))
                .unwrap();
        let neutral_bytes = neutral.into_canonical_bytes();
        let (_, neutral_module, _) =
            decode_exact_kernel_ir(&neutral_bytes, ProductionReplayKernelIrVersionV1::V8).unwrap();
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV8::from_module(target.module().clone()).unwrap();
        let llvm = replay_llvm(
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
            ProductionSemanticAnchorKirIdentityV1::from_v8(&target_owner),
        )
        .unwrap();
        let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            &neutral_bytes,
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            &llvm,
        )
        .unwrap();
        let canonical = evidence.canonical_bytes();
        let kernel_count_offset = EVIDENCE_MAGIC_V1.len() + 2 + 4 + (32 + 8) * 2;
        let mut cursor = kernel_count_offset;
        assert_eq!(
            u32::from_le_bytes(canonical[cursor..cursor + 4].try_into().unwrap()),
            3,
        );
        cursor += 4;
        let mut frames = Vec::new();
        for expected in evidence.kernel_ids() {
            let start = cursor;
            let length = usize::try_from(u32::from_le_bytes(
                canonical[cursor..cursor + 4].try_into().unwrap(),
            ))
            .unwrap();
            cursor += 4;
            assert_eq!(
                &canonical[cursor..cursor + length],
                expected.as_str().as_bytes()
            );
            cursor += length;
            frames.push((start, cursor));
        }
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].1 - frames[0].0, frames[1].1 - frames[1].0);
        assert_eq!(frames[0].1 - frames[0].0, frames[2].1 - frames[2].0);

        let mut reordered = canonical.to_vec();
        let first = canonical[frames[0].0..frames[0].1].to_vec();
        let second = canonical[frames[1].0..frames[1].1].to_vec();
        reordered[frames[0].0..frames[0].1].copy_from_slice(&second);
        reordered[frames[1].0..frames[1].1].copy_from_slice(&first);
        let reordered = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&reordered).unwrap();
        assert!(matches!(
            reordered.validate_against_neutral_kernel_ir(&neutral_bytes),
            Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch)
        ));

        let mut substituted = canonical.to_vec();
        substituted[frames[1].1 - 1] = b'x';
        let substituted =
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&substituted).unwrap();
        assert!(matches!(
            substituted.validate_against_neutral_kernel_ir(&neutral_bytes),
            Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch)
        ));

        let omitted = encode_evidence(
            evidence.profile(),
            evidence.neutral_kernel_ir_identity(),
            evidence.target_bound_kernel_ir_identity(),
            &evidence.kernel_ids()[..2],
            evidence.pre_descriptor_llvm(),
        )
        .unwrap();
        let omitted = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&omitted).unwrap();
        assert!(matches!(
            omitted.validate_against_neutral_kernel_ir(&neutral_bytes),
            Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch)
        ));

        let mut added_ids = evidence.kernel_ids().to_vec();
        added_ids.push(KernelId::new("hostile_roster_kernel_added"));
        let added = encode_evidence(
            evidence.profile(),
            evidence.neutral_kernel_ir_identity(),
            evidence.target_bound_kernel_ir_identity(),
            &added_ids,
            evidence.pre_descriptor_llvm(),
        )
        .unwrap();
        let added = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&added).unwrap();
        assert!(matches!(
            added.validate_against_neutral_kernel_ir(&neutral_bytes),
            Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch)
        ));

        let mut adjacent_duplicate = canonical.to_vec();
        adjacent_duplicate[frames[1].0..frames[1].1].copy_from_slice(&first);
        assert!(matches!(
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&adjacent_duplicate),
            Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch)
        ));

        let mut nonadjacent_duplicate = canonical.to_vec();
        nonadjacent_duplicate[frames[2].0..frames[2].1].copy_from_slice(&first);
        assert!(matches!(
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&nonadjacent_duplicate),
            Err(ProductionKirToLlvmReplayErrorV1::KernelIdMismatch)
        ));

        let mut zero_count = canonical.to_vec();
        zero_count[kernel_count_offset..kernel_count_offset + 4]
            .copy_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&zero_count),
            Err(ProductionKirToLlvmReplayErrorV1::InvalidLength)
        ));
    }

    #[test]
    fn replay_v2_encoder_rejects_adjacent_and_nonadjacent_duplicate_kernel_ids() {
        let identity = ProductionReplayKernelIrIdentityV1 {
            version: ProductionReplayKernelIrVersionV1::V8,
            sha256: [0x71; 32],
            byte_len: 1,
        };
        let adjacent = [KernelId::new("first"), KernelId::new("first")];
        assert!(matches!(
            encode_evidence(
                ProductionAmdTargetProfileV1::Gfx942,
                identity,
                identity,
                &adjacent,
                "target triple = \"amdgcn-amd-amdhsa\"\n",
            ),
            Err(ProductionKirToLlvmReplayErrorV1::InvalidLength)
        ));

        let nonadjacent = [
            KernelId::new("first"),
            KernelId::new("second"),
            KernelId::new("first"),
        ];
        assert!(matches!(
            encode_evidence(
                ProductionAmdTargetProfileV1::Gfx942,
                identity,
                identity,
                &nonadjacent,
                "target triple = \"amdgcn-amd-amdhsa\"\n",
            ),
            Err(ProductionKirToLlvmReplayErrorV1::InvalidLength)
        ));
    }

    #[test]
    fn replay_v2_singleton_framing_is_decodable_but_noncanonical() {
        let (_, _, _, evidence) = fixture("hostile_v2_singleton");
        let mut hostile = evidence.canonical_bytes().to_vec();
        let version_offset = EVIDENCE_MAGIC_V1.len();
        hostile[version_offset..version_offset + 2]
            .copy_from_slice(&EVIDENCE_VERSION_V2.to_le_bytes());
        let kernel_count_offset = EVIDENCE_MAGIC_V1.len() + 2 + 4 + (32 + 8) * 2;
        hostile.splice(
            kernel_count_offset..kernel_count_offset,
            1_u32.to_le_bytes(),
        );

        assert!(matches!(
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&hostile),
            Err(ProductionKirToLlvmReplayErrorV1::NonCanonical)
        ));
    }

    #[test]
    fn replay_rejects_hostile_evidence_framing_and_llvm_mutation() {
        let (neutral, _, _, evidence) = fixture("hostile");
        let canonical = evidence.canonical_bytes();
        for prefix in 0..canonical.len() {
            assert!(
                CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&canonical[..prefix]).is_err()
            );
        }
        let mut trailing = canonical.to_vec();
        trailing.push(0);
        assert!(CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&trailing).is_err());

        let mut mutated = canonical.to_vec();
        let llvm = evidence.pre_descriptor_llvm().as_bytes();
        let llvm_start = mutated.len() - llvm.len();
        let return_offset = llvm
            .windows(8)
            .position(|window| window == b"ret void")
            .expect("fixture LLVM contains a return");
        mutated[llvm_start + return_offset] = b'R';
        let decoded = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&mutated).unwrap();
        assert!(matches!(
            decoded.validate_against_neutral_kernel_ir(&neutral),
            Err(ProductionKirToLlvmReplayErrorV1::LlvmMismatch)
        ));
    }

    #[test]
    fn replay_rejects_cross_spliced_neutral_kir_and_live_target_module() {
        let (neutral_a, target_a, llvm_a, evidence_a) = fixture("alpha");
        let (neutral_b, _, _, _) = fixture("beta");
        assert!(matches!(
            evidence_a.validate_against_neutral_kernel_ir(&neutral_b),
            Err(ProductionKirToLlvmReplayErrorV1::IdentityMismatch {
                field: "neutral Kernel IR"
            })
        ));

        let mut hostile_target = target_a;
        hostile_target.id = "substituted".into();
        assert!(matches!(
            CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
                &neutral_a,
                &hostile_target,
                ProductionAmdTargetProfileV1::Gfx942,
                &llvm_a,
            ),
            Err(ProductionKirToLlvmReplayErrorV1::LiveTargetModuleMismatch)
        ));
    }

    #[test]
    fn replay_rejects_oversized_input_before_field_allocation() {
        let oversized = vec![0; MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1 + 1];
        assert!(matches!(
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&oversized),
            Err(ProductionKirToLlvmReplayErrorV1::TooLarge)
        ));
    }

    #[test]
    fn replay_v2_rejects_maximum_kernel_count_without_allocating_or_panicking() {
        let neutral =
            VerifiedCanonicalKernelIrV8::from_module(multi_neutral_module("hostile_count", 2))
                .unwrap();
        let neutral_bytes = neutral.into_canonical_bytes();
        let (_, neutral_module, _) =
            decode_exact_kernel_ir(&neutral_bytes, ProductionReplayKernelIrVersionV1::V8).unwrap();
        let target =
            bind_production_target_v1(&neutral_module, ProductionAmdTargetProfileV1::Gfx942)
                .unwrap();
        let target_owner =
            VerifiedCanonicalKernelIrV8::from_module(target.module().clone()).unwrap();
        let llvm = replay_llvm(
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionKirToLlvmReplayModeV1::LegacyUninstrumented,
            ProductionSemanticAnchorKirIdentityV1::from_v8(&target_owner),
        )
        .unwrap();
        let evidence = CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
            &neutral_bytes,
            target.module(),
            ProductionAmdTargetProfileV1::Gfx942,
            &llvm,
        )
        .unwrap();
        let mut hostile = evidence.canonical_bytes().to_vec();
        let kernel_count_offset = EVIDENCE_MAGIC_V1.len() + 2 + 4 + (32 + 8) * 2;
        hostile[kernel_count_offset..kernel_count_offset + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        let result = std::panic::catch_unwind(|| {
            CanonicalProductionKirToLlvmReplayEvidenceV1::decode(&hostile)
        });
        assert!(
            result.is_ok(),
            "hostile kernel count panicked during decode"
        );
        assert!(matches!(
            result.unwrap(),
            Err(ProductionKirToLlvmReplayErrorV1::InvalidLength)
        ));
    }
}
