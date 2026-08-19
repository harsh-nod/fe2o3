//! Exact, non-authoritative correspondence between canonical general-GEMM KIR and Verus inputs.
//!
//! This layer consumes the kernel-IR crate's canonical representation and verifier. It does not
//! recreate the GEMM algorithm. The resulting record binds one concrete checked plan and KIR to
//! the exact verifier-owned universal model, theorem set, positive source, retained source closure,
//! and symbolic proof request. It deliberately makes no BF16/FP32 rounding or LLVM/ISA claim.

use core::fmt;

use fe2o3_kernel_ir::{
    GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1, GENERAL_GEMM_KIR_LDS_ELEMENTS_V1,
    GENERAL_GEMM_KIR_TILE_EXTENT_V1, GENERAL_GEMM_KIR_WAVE_LANES_V1, GeneralGemmKirFindingV1,
    GeneralGemmKirV1, GeneralGemmPropertyV1, GeneralGemmVerificationStageV1,
    verify_general_gemm_kir_v1,
};
use sha2::{Digest as _, Sha256};

use crate::general_gemm_proof_v1::derive_general_gemm_verus_input_identities_v1;
use crate::{
    AuthenticatedGeneralGemmScheduleProofV1, GENERAL_GEMM_PROOF_PROPERTIES_V1,
    GeneralGemmEvidenceIdentityV1, GeneralGemmProofExecutionErrorV1, GeneralGemmProofPropertyV1,
    GeneralGemmProofRequestV1, GeneralGemmProofScheduleV1, GeneralGemmVerusRuntimeClosureLeaseV2,
    execute_general_gemm_schedule_proof_with_runtime_closure_v2,
};

const CORRESPONDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3-general-gemm-kir-verus-correspondence-v1\0";
const PROOF_REQUEST_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3-general-gemm-correspondence-proof-request-v1\0";

/// Number of exact semantic property mappings carried by the correspondence.
pub const GENERAL_GEMM_KIR_MODEL_PROPERTY_COUNT_V1: usize = 12;

/// Honest scope of one KIR-to-model property mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmKirModelPropertyScopeV1 {
    /// Canonical KIR structure and its matching universal model inputs are bound.
    StructuralModelCorrespondence = 1,
    /// Exact-real recurrence is bound; BF16 decode and FP32/MFMA rounding remain open.
    ExactRealModelOnly = 2,
    /// KIR-to-LLVM/ISA and post-link machine refinement remain open.
    MachineRefinementOpen = 3,
}

/// Typed mapping between one kernel-IR property and one Verus proof property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmKirModelPropertyFactV1 {
    /// Property named by the canonical KIR verifier.
    pub kir_property: GeneralGemmPropertyV1,
    /// Matching verifier-owned property in the retained proof suite.
    pub proof_property: GeneralGemmProofPropertyV1,
    /// Earliest compiler stage that owns the KIR obligation.
    pub verification_stage: GeneralGemmVerificationStageV1,
    /// Stable KIR diagnostic code for a hostile counterexample.
    pub diagnostic_code: u32,
    /// Maximum claim justified by this correspondence layer.
    pub scope: GeneralGemmKirModelPropertyScopeV1,
}

/// Untrusted complete claim about one canonical KIR-to-Verus correspondence.
///
/// Public fields make single-field transport corruption testable. This value grants no authority;
/// only [`check_general_gemm_kir_model_correspondence_v1`] can construct the private checked record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmKirModelCorrespondenceClaimV1 {
    /// Canonical kernel-IR identity.
    pub kir_identity: GeneralGemmEvidenceIdentityV1,
    /// Exact ordered semantic property mappings.
    pub properties: [GeneralGemmKirModelPropertyFactV1; GENERAL_GEMM_KIR_MODEL_PROPERTY_COUNT_V1],
    /// Checked `[M, N, K]`.
    pub dimensions: [u32; 3],
    /// Checked row-major `[lda, ldb, ldc]`.
    pub strides: [u32; 3],
    /// Exact accessed `[A, B, C]` element extents.
    pub storage_elements: [u64; 3],
    /// Checked output tile counts.
    pub block_counts: [u32; 3],
    /// Checked AQL grid work-item counts.
    pub aql_grid_work_items: [u32; 3],
    /// Checked number of K phases.
    pub reduction_phases: u32,
    /// Exact runtime alpha bits retained by KIR.
    pub alpha_bits: u32,
    /// Exact runtime beta bits retained by KIR.
    pub beta_bits: u32,
    /// Live extents in the final M/N/K tiles.
    pub tail_shape: [u8; 3],
    /// Whether the checked plan requires a dispatch.
    pub requires_dispatch: bool,
    /// Exact retained proof schedule.
    pub schedule: GeneralGemmProofScheduleV1,
    /// Fixed output/reduction tile extent.
    pub tile_extent: u32,
    /// Fixed wave width.
    pub wave_lanes: u32,
    /// Accumulator/staging components per lane.
    pub components_per_lane: u32,
    /// Elements in each single-buffered LDS operand tile.
    pub lds_elements_per_operand: u64,
    /// Preferred A global transfer width in BF16 elements.
    pub a_global_transfer_width: u8,
    /// Preferred B global transfer width in BF16 elements.
    pub b_global_transfer_width: u8,
    /// Whether A has a scalar masked tail path distinct from its preferred transfer.
    pub a_scalar_tail_fallback: bool,
    /// Whether LDS staging is the canonical single-buffered schedule.
    pub single_buffered_lds: bool,
    /// Identity of every field consumed from the existing symbolic proof request.
    pub proof_request_identity: GeneralGemmEvidenceIdentityV1,
    /// Identity of the exact embedded universal Verus model bytes.
    pub model_identity: GeneralGemmEvidenceIdentityV1,
    /// Identity of the schedule-selected positive Verus source bytes.
    pub positive_source_identity: GeneralGemmEvidenceIdentityV1,
    /// Identity of the exact typed theorem/definition/open-obligation mapping.
    pub theorem_set_identity: GeneralGemmEvidenceIdentityV1,
    /// Identity of the complete positive and expected-negative retained source set.
    pub source_closure_identity: GeneralGemmEvidenceIdentityV1,
}

/// Exact field rejected by the correspondence checker.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmKirModelCorrespondenceFieldV1 {
    KirIdentity,
    Properties,
    Dimensions,
    Strides,
    StorageElements,
    BlockCounts,
    AqlGridWorkItems,
    ReductionPhases,
    AlphaBits,
    BetaBits,
    TailShape,
    RequiresDispatch,
    Schedule,
    TileExtent,
    WaveLanes,
    ComponentsPerLane,
    LdsElementsPerOperand,
    AGlobalTransferWidth,
    BGlobalTransferWidth,
    AScalarTailFallback,
    SingleBufferedLds,
    ProofRequestIdentity,
    ModelIdentity,
    PositiveSourceIdentity,
    TheoremSetIdentity,
    SourceClosureIdentity,
}

/// Failure to establish exact KIR-to-Verus-model correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmKirModelCorrespondenceErrorV1 {
    /// The KIR is not the exact canonical schedule for its checked plan.
    NonCanonicalKir,
    /// The canonical KIR verifier rejected a semantic property.
    KirRejected(GeneralGemmKirFindingV1),
    /// A transported claim substituted one exact field.
    FieldMismatch(GeneralGemmKirModelCorrespondenceFieldV1),
}

impl fmt::Display for GeneralGemmKirModelCorrespondenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalKir => formatter.write_str("general GEMM KIR is not canonical"),
            Self::KirRejected(finding) => write!(formatter, "general GEMM KIR rejected: {finding}"),
            Self::FieldMismatch(field) => {
                write!(
                    formatter,
                    "general GEMM KIR/model field mismatch: {field:?}"
                )
            }
        }
    }
}

impl std::error::Error for GeneralGemmKirModelCorrespondenceErrorV1 {}

/// Checked ownership of one exact KIR-to-Verus-model correspondence.
///
/// This record is deliberately non-`Clone` and has private fields. It can select only the already
/// reviewed retained proof inputs; it cannot authenticate a proof process or authorize compilation,
/// artifact publication, module loading, or GPU execution.
///
/// ```compile_fail
/// use fe2o3_verifier::{
///     GeneralGemmEvidenceIdentityV1, GeneralGemmKirModelCorrespondenceV1,
/// };
/// fn forge() -> GeneralGemmKirModelCorrespondenceV1 {
///     GeneralGemmKirModelCorrespondenceV1 {
///         identity: GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([1; 32]),
///         ..todo!()
///     }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::GeneralGemmKirModelCorrespondenceV1;
/// fn duplicate(value: &GeneralGemmKirModelCorrespondenceV1) {
///     let _second_owner: GeneralGemmKirModelCorrespondenceV1 = value.clone();
/// }
/// ```
#[derive(Debug)]
#[must_use = "KIR/model correspondence remains non-authoritative until owning layers consume it"]
pub struct GeneralGemmKirModelCorrespondenceV1 {
    claim: GeneralGemmKirModelCorrespondenceClaimV1,
    proof_request: GeneralGemmProofRequestV1,
    identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmKirModelCorrespondenceV1 {
    /// Returns the exact recomputed claim.
    pub const fn claim(&self) -> GeneralGemmKirModelCorrespondenceClaimV1 {
        self.claim
    }

    /// Returns the exact proof request bound into this correspondence.
    pub const fn proof_request(&self) -> GeneralGemmProofRequestV1 {
        self.proof_request
    }

    /// Returns the domain-separated identity of every checked field.
    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    /// KIR/model agreement alone never grants compiler proof-gate authority.
    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }

    /// KIR/model agreement grants no artifact or runtime authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Retained-root proof evidence paired with its consumed exact KIR/model correspondence.
///
/// This aggregate is non-`Clone`, remains model-local, and does not close numerical machine
/// rounding or emitted-machine refinement.
#[derive(Debug)]
#[must_use = "corresponded schedule proof remains non-authoritative model evidence"]
pub struct CorrespondedGeneralGemmScheduleProofV1 {
    correspondence: GeneralGemmKirModelCorrespondenceV1,
    schedule_proof: AuthenticatedGeneralGemmScheduleProofV1,
}

impl CorrespondedGeneralGemmScheduleProofV1 {
    /// Returns the consumed checked KIR/model correspondence.
    pub const fn correspondence(&self) -> &GeneralGemmKirModelCorrespondenceV1 {
        &self.correspondence
    }

    /// Returns exact retained-root schedule proof evidence.
    pub const fn schedule_proof(&self) -> &AuthenticatedGeneralGemmScheduleProofV1 {
        &self.schedule_proof
    }

    /// BF16/FP32 machine rounding and LLVM/ISA refinement remain open.
    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }

    /// Corresponded model evidence grants no artifact or execution authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Executes only the exact proof request and retained sources selected by checked correspondence.
///
/// The runtime lease still performs its root-owned retained-object checks. No caller path, digest,
/// source bytes, theorem name, proof output, or model identity enters this execution boundary.
pub fn execute_corresponded_general_gemm_schedule_proof_v1(
    correspondence: GeneralGemmKirModelCorrespondenceV1,
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
    timeout_seconds: u32,
) -> Result<CorrespondedGeneralGemmScheduleProofV1, GeneralGemmProofExecutionErrorV1> {
    let schedule_proof = execute_general_gemm_schedule_proof_with_runtime_closure_v2(
        correspondence.proof_request,
        runtime,
        timeout_seconds,
    )?;
    if schedule_proof.source_closure_identity() != correspondence.claim.source_closure_identity {
        return Err(GeneralGemmProofExecutionErrorV1::RetainedSourceCorrespondenceMismatch);
    }
    Ok(CorrespondedGeneralGemmScheduleProofV1 {
        correspondence,
        schedule_proof,
    })
}

/// Derives every exact correspondence field from typed canonical inputs.
pub fn derive_general_gemm_kir_model_correspondence_claim_v1(
    kir: &GeneralGemmKirV1,
    proof_request: GeneralGemmProofRequestV1,
) -> Result<GeneralGemmKirModelCorrespondenceClaimV1, GeneralGemmKirModelCorrespondenceErrorV1> {
    if *kir != GeneralGemmKirV1::canonical(kir.plan()) {
        return Err(GeneralGemmKirModelCorrespondenceErrorV1::NonCanonicalKir);
    }
    verify_general_gemm_kir_v1(kir)
        .map_err(GeneralGemmKirModelCorrespondenceErrorV1::KirRejected)?;

    let plan = kir.plan();
    let tails = plan.tails();
    let schedule = proof_request.schedule();
    let verus = derive_general_gemm_verus_input_identities_v1(schedule);
    let (a_global_transfer_width, a_scalar_tail_fallback) = match schedule {
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1 => (1, false),
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => (4, true),
    };
    Ok(GeneralGemmKirModelCorrespondenceClaimV1 {
        kir_identity: GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(
            *kir.identity().as_bytes(),
        ),
        properties: canonical_property_facts(),
        dimensions: plan.dimensions(),
        strides: plan.strides(),
        storage_elements: plan.storage_elements(),
        block_counts: plan.block_counts(),
        aql_grid_work_items: plan.aql_grid_work_items(),
        reduction_phases: plan.reduction_phases(),
        alpha_bits: plan.alpha_bits(),
        beta_bits: plan.beta_bits(),
        tail_shape: [tails.m, tails.n, tails.k],
        requires_dispatch: plan.requires_dispatch(),
        schedule,
        tile_extent: GENERAL_GEMM_KIR_TILE_EXTENT_V1,
        wave_lanes: GENERAL_GEMM_KIR_WAVE_LANES_V1,
        components_per_lane: GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1,
        lds_elements_per_operand: GENERAL_GEMM_KIR_LDS_ELEMENTS_V1,
        a_global_transfer_width,
        b_global_transfer_width: 1,
        a_scalar_tail_fallback,
        single_buffered_lds: true,
        proof_request_identity: proof_request_identity(proof_request),
        model_identity: verus.model_identity,
        positive_source_identity: verus.positive_source_identity,
        theorem_set_identity: verus.theorem_set_identity,
        source_closure_identity: verus.source_closure_identity,
    })
}

/// Checks an untrusted transported claim and returns non-forgeable correspondence ownership.
pub fn check_general_gemm_kir_model_correspondence_v1(
    kir: &GeneralGemmKirV1,
    proof_request: GeneralGemmProofRequestV1,
    claimed: GeneralGemmKirModelCorrespondenceClaimV1,
) -> Result<GeneralGemmKirModelCorrespondenceV1, GeneralGemmKirModelCorrespondenceErrorV1> {
    let expected = derive_general_gemm_kir_model_correspondence_claim_v1(kir, proof_request)?;
    compare_claims(expected, claimed)?;
    Ok(GeneralGemmKirModelCorrespondenceV1 {
        claim: expected,
        proof_request,
        identity: correspondence_identity(expected),
    })
}

fn canonical_property_facts()
-> [GeneralGemmKirModelPropertyFactV1; GENERAL_GEMM_KIR_MODEL_PROPERTY_COUNT_V1] {
    use GeneralGemmKirModelPropertyScopeV1::{
        ExactRealModelOnly, MachineRefinementOpen, StructuralModelCorrespondence,
    };
    use GeneralGemmProofPropertyV1 as Proof;
    use GeneralGemmPropertyV1 as Kir;

    let mappings = [
        (
            Kir::MemorySafe,
            Proof::MemorySafe,
            StructuralModelCorrespondence,
        ),
        (
            Kir::BoundsSafe,
            Proof::BoundsSafe,
            StructuralModelCorrespondence,
        ),
        (
            Kir::Initialized,
            Proof::Initialized,
            StructuralModelCorrespondence,
        ),
        (
            Kir::RaceFree,
            Proof::RaceFree,
            StructuralModelCorrespondence,
        ),
        (
            Kir::BarrierConvergent,
            Proof::BarrierConvergent,
            StructuralModelCorrespondence,
        ),
        (
            Kir::OutputRegionInjective,
            Proof::OutputRegionInjective,
            StructuralModelCorrespondence,
        ),
        (
            Kir::LdsEpochCorrect,
            Proof::LdsEpochCorrect,
            StructuralModelCorrespondence,
        ),
        (
            Kir::AccumulatorPhaseRefinement,
            Proof::AccumulatorPhaseRefinement,
            StructuralModelCorrespondence,
        ),
        (
            Kir::TailRefinement,
            Proof::TailRefinement,
            StructuralModelCorrespondence,
        ),
        (
            Kir::EpilogueRefinement,
            Proof::EpilogueRefinement,
            StructuralModelCorrespondence,
        ),
        (
            Kir::NumericalContract,
            Proof::NumericalContract,
            ExactRealModelOnly,
        ),
        (
            Kir::MachineRefinementBoundary,
            Proof::MachineRefinementBoundary,
            MachineRefinementOpen,
        ),
    ];
    core::array::from_fn(|index| {
        let (kir_property, proof_property, scope) = mappings[index];
        debug_assert_eq!(proof_property, GENERAL_GEMM_PROOF_PROPERTIES_V1[index]);
        GeneralGemmKirModelPropertyFactV1 {
            kir_property,
            proof_property,
            verification_stage: kir_property.verification_stage(),
            diagnostic_code: kir_property.diagnostic_code(),
            scope,
        }
    })
}

fn proof_request_identity(request: GeneralGemmProofRequestV1) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(PROOF_REQUEST_IDENTITY_DOMAIN_V1);
    hasher.update([request.schedule() as u8]);
    for identity in request.identities() {
        hasher.update(identity.as_bytes());
    }
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

fn correspondence_identity(
    claim: GeneralGemmKirModelCorrespondenceClaimV1,
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(CORRESPONDENCE_IDENTITY_DOMAIN_V1);
    hasher.update(claim.kir_identity.as_bytes());
    for property in claim.properties {
        hasher.update([
            kir_property_tag(property.kir_property),
            property.proof_property as u8,
            property.verification_stage as u8,
            property.scope as u8,
        ]);
        hasher.update(property.diagnostic_code.to_le_bytes());
    }
    put_u32_array(&mut hasher, claim.dimensions);
    put_u32_array(&mut hasher, claim.strides);
    for value in claim.storage_elements {
        hasher.update(value.to_le_bytes());
    }
    put_u32_array(&mut hasher, claim.block_counts);
    put_u32_array(&mut hasher, claim.aql_grid_work_items);
    hasher.update(claim.reduction_phases.to_le_bytes());
    hasher.update(claim.alpha_bits.to_le_bytes());
    hasher.update(claim.beta_bits.to_le_bytes());
    hasher.update(claim.tail_shape);
    hasher.update([u8::from(claim.requires_dispatch), claim.schedule as u8]);
    hasher.update(claim.tile_extent.to_le_bytes());
    hasher.update(claim.wave_lanes.to_le_bytes());
    hasher.update(claim.components_per_lane.to_le_bytes());
    hasher.update(claim.lds_elements_per_operand.to_le_bytes());
    hasher.update([
        claim.a_global_transfer_width,
        claim.b_global_transfer_width,
        u8::from(claim.a_scalar_tail_fallback),
        u8::from(claim.single_buffered_lds),
    ]);
    for identity in [
        claim.proof_request_identity,
        claim.model_identity,
        claim.positive_source_identity,
        claim.theorem_set_identity,
        claim.source_closure_identity,
    ] {
        hasher.update(identity.as_bytes());
    }
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

const fn kir_property_tag(property: GeneralGemmPropertyV1) -> u8 {
    match property {
        GeneralGemmPropertyV1::MemorySafe => 1,
        GeneralGemmPropertyV1::BoundsSafe => 2,
        GeneralGemmPropertyV1::Initialized => 3,
        GeneralGemmPropertyV1::RaceFree => 4,
        GeneralGemmPropertyV1::BarrierConvergent => 5,
        GeneralGemmPropertyV1::OutputRegionInjective => 6,
        GeneralGemmPropertyV1::LdsEpochCorrect => 7,
        GeneralGemmPropertyV1::AccumulatorPhaseRefinement => 8,
        GeneralGemmPropertyV1::TailRefinement => 9,
        GeneralGemmPropertyV1::EpilogueRefinement => 10,
        GeneralGemmPropertyV1::NumericalContract => 11,
        GeneralGemmPropertyV1::MachineRefinementBoundary => 12,
    }
}

fn put_u32_array(hasher: &mut Sha256, values: [u32; 3]) {
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

macro_rules! require_field {
    ($expected:ident, $claimed:ident, $field:ident, $name:ident) => {
        if $expected.$field != $claimed.$field {
            return Err(GeneralGemmKirModelCorrespondenceErrorV1::FieldMismatch(
                GeneralGemmKirModelCorrespondenceFieldV1::$name,
            ));
        }
    };
}

fn compare_claims(
    expected: GeneralGemmKirModelCorrespondenceClaimV1,
    claimed: GeneralGemmKirModelCorrespondenceClaimV1,
) -> Result<(), GeneralGemmKirModelCorrespondenceErrorV1> {
    require_field!(expected, claimed, kir_identity, KirIdentity);
    require_field!(expected, claimed, properties, Properties);
    require_field!(expected, claimed, dimensions, Dimensions);
    require_field!(expected, claimed, strides, Strides);
    require_field!(expected, claimed, storage_elements, StorageElements);
    require_field!(expected, claimed, block_counts, BlockCounts);
    require_field!(expected, claimed, aql_grid_work_items, AqlGridWorkItems);
    require_field!(expected, claimed, reduction_phases, ReductionPhases);
    require_field!(expected, claimed, alpha_bits, AlphaBits);
    require_field!(expected, claimed, beta_bits, BetaBits);
    require_field!(expected, claimed, tail_shape, TailShape);
    require_field!(expected, claimed, requires_dispatch, RequiresDispatch);
    require_field!(expected, claimed, schedule, Schedule);
    require_field!(expected, claimed, tile_extent, TileExtent);
    require_field!(expected, claimed, wave_lanes, WaveLanes);
    require_field!(expected, claimed, components_per_lane, ComponentsPerLane);
    require_field!(
        expected,
        claimed,
        lds_elements_per_operand,
        LdsElementsPerOperand
    );
    require_field!(
        expected,
        claimed,
        a_global_transfer_width,
        AGlobalTransferWidth
    );
    require_field!(
        expected,
        claimed,
        b_global_transfer_width,
        BGlobalTransferWidth
    );
    require_field!(
        expected,
        claimed,
        a_scalar_tail_fallback,
        AScalarTailFallback
    );
    require_field!(expected, claimed, single_buffered_lds, SingleBufferedLds);
    require_field!(
        expected,
        claimed,
        proof_request_identity,
        ProofRequestIdentity
    );
    require_field!(expected, claimed, model_identity, ModelIdentity);
    require_field!(
        expected,
        claimed,
        positive_source_identity,
        PositiveSourceIdentity
    );
    require_field!(expected, claimed, theorem_set_identity, TheoremSetIdentity);
    require_field!(
        expected,
        claimed,
        source_closure_identity,
        SourceClosureIdentity
    );
    Ok(())
}
