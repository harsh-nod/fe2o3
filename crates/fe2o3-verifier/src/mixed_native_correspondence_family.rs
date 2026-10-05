// Shared immutable native receipt joins; no semantic or execution authority.
use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1, PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
    ProductionAmdTargetProfileV1 as Profile,
};
use fe2o3_compiler_ffi::{CompilerModuleKindV1, InertSemanticCompilerModuleHandoffV3};
use fe2o3_compiler_lineage::{
    DataLayoutTranscriptV3, ProductionTargetLineageErrorV3, SemanticToLlvmAssociationTranscriptV3,
    TargetLineageIdentityV3 as Coordinate, derive_semantic_target_layout_identity_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{mem::size_of, panic::AssertUnwindSafe};

/// Strict content refusal. Neither success nor a valid transcript proves execution.
#[derive(Debug)]
pub enum NativeCorrespondenceError {
    /// Original cumulative ledger refusal.
    Resource(Resource),
    /// A strict bounded transcript codec refused.
    Transcript(ProductionTargetLineageErrorV3),
    /// Actual V18 native emission or complete descriptor-text comparison refused.
    Native(NativeReplayError),
    /// Exact immutable content coordinates differ.
    Binding(&'static str),
}
type Error = NativeCorrespondenceError;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mixed native content correspondence: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Transcript(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Binding(_) => None,
        }
    }
}

fn headers() -> usize {
    size_of::<(
        &Owner,
        Profile,
        &[u8],
        &InertSemanticCompilerModuleHandoffV3,
    )>() + size_of::<AssertUnwindSafe<&mut Budget<'_>>>()
        + size_of::<Budget<'_>>()
        + 12 * size_of::<usize>()
        + size_of::<DataLayoutTranscriptV3>()
        + size_of::<SemanticToLlvmAssociationTranscriptV3>()
        + size_of::<fe2o3_compiler_lineage::DataLayoutTranscriptInputsV3<'_>>()
        + size_of::<fe2o3_compiler_lineage::SemanticToLlvmAssociationInputsV3>()
        + size_of::<[(Coordinate, Coordinate); 13]>()
        + size_of::<sha2::Sha256>()
        + 2 * size_of::<Result<Coordinate, ProductionTargetLineageErrorV3>>()
        + size_of::<Result<DataLayoutTranscriptV3, ProductionTargetLineageErrorV3>>()
        + size_of::<Result<SemanticToLlvmAssociationTranscriptV3, ProductionTargetLineageErrorV3>>()
        + 2 * size_of::<Result<(), Error>>()
        + size_of::<std::thread::Result<Result<(), Error>>>()
}

fn transcript_headers() -> usize {
    3 * size_of::<Vec<u8>>()
        + 2 * size_of::<Vec<[u32; 2]>>()
        + 2 * size_of::<Vec<&[u8]>>()
        + 2 * size_of::<(&[u8], Vec<[u32; 2]>)>()
        + size_of::<[&[u8]; 6]>()
        + size_of::<Box<[u8]>>()
        + size_of::<Result<Box<[u8]>, ProductionTargetLineageErrorV3>>()
}

fn semantic_layout_bytes(profile: Profile) -> Result<usize, Resource> {
    [
        6 * size_of::<u64>(),
        b"fe2o3/semantic-mir/rustc-target-layout/v1".len(),
        profile.rustc_target().len(),
        PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1.len(),
        size_of::<u16>(),
        profile.cpu().len(),
        profile.rustc_features().len(),
    ]
    .into_iter()
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or(Resource::Arithmetic)
}

fn transcript_scratch(
    profile: Profile,
    layout: usize,
    association: usize,
) -> Result<usize, Resource> {
    // The strict codecs fix 10 layout and 15 association fields before any
    // reservation. Include their private range rows and temporary field slices,
    // both canonical copies, and the six-field layout digest preimage. Two
    // copies allow Vec-to-box conversion while the original buffer is live.
    // Native emitter allocations remain a separate bounded engine policy.
    [
        transcript_headers(),
        2 * (10 + 15) * size_of::<[u32; 2]>(),
        (10 + 15) * size_of::<&[u8]>(),
        layout.checked_mul(2).ok_or(Resource::Arithmetic)?,
        association.checked_mul(2).ok_or(Resource::Arithmetic)?,
        semantic_layout_bytes(profile)?
            .checked_mul(2)
            .ok_or(Resource::Arithmetic)?,
    ]
    .into_iter()
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or(Resource::Arithmetic)
}

macro_rules! identity {
    ($receipt:expr) => {
        Coordinate::new(
            *$receipt.identity().sha256(),
            $receipt.identity().byte_len(),
        )
        .map_err(Error::Transcript)?
    };
}

/// Checks one already-readmitted final owner and its complete immutable descriptor
/// handoff. The caller separately checks target selection against this same
/// owner and an inspected profile. This establishes deterministic native text
/// and receipt association only, not source-MIR admission, semantic refinement,
/// native theorem execution, producer authenticity, artifact currentness or
/// load/launch authority. No graph is cloned, decoded or substituted here.
pub fn check_correspondence(
    owner: &Owner,
    profile: Profile,
    descriptor: &[u8],
    outer: &InertSemanticCompilerModuleHandoffV3,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.check_prior_denials_v1()?;
    let receipts = outer.capsule().receipts();
    let native = outer.module_handoff();
    let scratch = [
        headers(),
        descriptor.len(),
        native.module_bytes().len(),
        receipts.amdgpu_lowering().canonical_preimage().len(),
        transcript_scratch(
            profile,
            receipts.data_layout().canonical_preimage().len(),
            receipts.semantic_to_llvm().canonical_preimage().len(),
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 1, 1, scratch, |budget| {
        budget.charge_work(
            owner
                .canonical_bytes()
                .len()
                .checked_add(receipts.kernel_ir().canonical_preimage().len())
                .and_then(|n| n.checked_add(descriptor.len().checked_mul(2)?))
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if owner.canonical_bytes() != receipts.kernel_ir().canonical_preimage()
            || descriptor != receipts.abi().canonical_preimage()
            || descriptor != receipts.formal_memory().canonical_preimage()
            || native.kind() != CompilerModuleKindV1::LlvmTextIr
        {
            return Err(Error::Binding(DESCRIPTOR_BINDING));
        }
        budget.charge_work(native.module_bytes().len())?;
        let text = std::str::from_utf8(native.module_bytes())
            .map_err(|_| Error::Binding("complete LLVM UTF-8"))?;
        check_native_relation(
            owner,
            profile,
            receipts.amdgpu_lowering().canonical_preimage(),
            descriptor,
            text,
            budget,
        )
        .map_err(Error::Native)?;
        check_layout_and_association(profile, outer, budget)
    })
}

fn check_layout_and_association(
    profile: Profile,
    outer: &InertSemanticCompilerModuleHandoffV3,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let receipts = outer.capsule().receipts();
    let native = outer.module_handoff();
    let layout_wire = receipts.data_layout().canonical_preimage();
    let association_wire = receipts.semantic_to_llvm().canonical_preimage();
    budget.charge_work(
        layout_wire
            .len()
            .checked_add(association_wire.len())
            .and_then(|n| n.checked_add(2048))
            .ok_or(Resource::Arithmetic)?,
    )?;
    let layout = DataLayoutTranscriptV3::decode(layout_wire).map_err(Error::Transcript)?;
    let data = layout.inputs().map_err(Error::Transcript)?;
    let expected_layout = derive_semantic_target_layout_identity_v1(
        profile.rustc_target(),
        data.live_rustc_data_layout,
        data.default_pointer_width_bits,
        profile.cpu(),
        profile.rustc_features(),
    )
    .map_err(Error::Transcript)?;
    // Strict decoding fixes live Rust layout and width. The replay above checks
    // the actual final header; no claimed layout string replaces those bytes.
    if data.semantic_mir != identity!(receipts.semantic_mir())
        || data.target_binding != identity!(receipts.target_binding())
        || data.semantic_layout != expected_layout
        || data.rustc_llvm_target != profile.rustc_target()
        || data.final_llvm_target != profile.rustc_target()
        || data.final_llvm_data_layout != PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1
    {
        return Err(Error::Binding("exact native layout transcript"));
    }
    let association = SemanticToLlvmAssociationTranscriptV3::decode(association_wire)
        .map_err(Error::Transcript)?;
    let actual = association.inputs().map_err(Error::Transcript)?;
    for (actual, expected) in [
        (actual.semantic_mir, identity!(receipts.semantic_mir())),
        (actual.middle_end, identity!(receipts.middle_end())),
        (actual.kernel_ir, identity!(receipts.kernel_ir())),
        (
            actual.mir_to_kir_correspondence,
            identity!(receipts.mir_to_kir_correspondence()),
        ),
        (actual.formal_memory, identity!(receipts.formal_memory())),
        (actual.proof_binding, identity!(receipts.proof_binding())),
        (actual.target_binding, identity!(receipts.target_binding())),
        (actual.data_layout, identity!(receipts.data_layout())),
        (actual.abi, identity!(receipts.abi())),
        (
            actual.export_manifest,
            identity!(receipts.export_manifest()),
        ),
        (
            actual.amdgpu_lowering,
            identity!(receipts.amdgpu_lowering()),
        ),
        (
            actual.final_llvm,
            Coordinate::new(
                *native.module_identity().sha256(),
                native.module_identity().byte_len(),
            )
            .map_err(Error::Transcript)?,
        ),
        (
            actual.final_compiler_module_commitment,
            identity!(receipts.final_compiler_module_commitment()),
        ),
    ] {
        budget.charge_work(1)?;
        if actual != expected {
            return Err(Error::Binding("exact semantic-to-LLVM receipt axis"));
        }
    }
    Ok(())
}
