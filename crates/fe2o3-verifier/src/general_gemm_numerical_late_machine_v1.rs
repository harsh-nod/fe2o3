//! Identity-only late-machine binding for General GEMM numerical correspondence.
//!
//! This layer checks bounded identity transport against a retained numerical
//! correspondence. It does not authenticate the graph, worker, or finalizer
//! producers and therefore cannot grant compiler or artifact authority.

use core::fmt;

use sha2::{Digest as _, Sha256};

use crate::{
    GENERAL_GEMM_GFX942_MFMA_INTRINSIC_V1, GENERAL_GEMM_GFX942_MFMA_MNEMONIC_V1,
    GENERAL_GEMM_NUMERICAL_CORRESPONDENCE_SCHEMA_V1, GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1,
    GeneralGemmEvidenceIdentityV1, GeneralGemmFutureMachineRefinementInputV1,
    GeneralGemmNumericalCorrespondenceStatusV1, GeneralGemmNumericalCorrespondenceV1,
    GeneralGemmNumericalPropertyFactV1, GeneralGemmProofScheduleV1,
};

/// Stable schema for the verifier-only late-machine identity binding.
pub const GENERAL_GEMM_NUMERICAL_LATE_MACHINE_SCHEMA_V1: &str =
    "fe2o3.general-gemm.numerical-late-machine-identity-binding.v1";
/// Fixed maximum number of property/theorem identities inspected by this binding.
pub const MAX_GENERAL_GEMM_NUMERICAL_LATE_MACHINE_PROPERTY_BINDINGS_V1: usize =
    GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1;

const LATE_MACHINE_BINDING_DOMAIN_V1: &[u8] =
    b"fe2o3-general-gemm-numerical-late-machine-binding-v1\0";
const LATE_MACHINE_SCHEMA_DOMAIN_V1: &[u8] =
    b"fe2o3-general-gemm-numerical-late-machine-schema-v1\0";

/// One externally owned machine identity required by the late join.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmNumericalMachineIdentityAxisV1 {
    OwnerBoundPlironGraphSerialization = 1,
    DirectLlvmWorkerRequestResponse = 2,
    FinalizerPostLinkIsaResult = 3,
}

/// Exact bounded gfx942 MFMA mnemonic classification carried by the transport claim.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmGfx942MfmaMnemonicV1 {
    VmfmaF32_16x16x16Bf16 = 1,
    UnsupportedOther = 2,
}

impl GeneralGemmGfx942MfmaMnemonicV1 {
    /// Returns the fixed mnemonic represented by this bounded classification.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VmfmaF32_16x16x16Bf16 => GENERAL_GEMM_GFX942_MFMA_MNEMONIC_V1,
            Self::UnsupportedOther => "unsupported-other-mfma",
        }
    }
}

/// Identity-only description of machine evidence owned by later compiler layers.
///
/// These fields authenticate no producer. The rustc final join must derive them
/// from, and retain, the concrete graph, worker, and finalizer owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalMachineJoinV1 {
    /// Exact retained numerical correspondence this machine evidence claims to refine.
    pub numerical_correspondence_identity: GeneralGemmEvidenceIdentityV1,
    pub owner_bound_pliron_graph_serialization_identity: Option<GeneralGemmEvidenceIdentityV1>,
    pub direct_llvm_worker_request_response_identity: Option<GeneralGemmEvidenceIdentityV1>,
    pub finalizer_post_link_isa_result_identity: Option<GeneralGemmEvidenceIdentityV1>,
}

impl GeneralGemmNumericalMachineJoinV1 {
    /// Returns whether all three identity slots are populated, without authenticating them.
    pub const fn has_all_required_identities(self) -> bool {
        self.owner_bound_pliron_graph_serialization_identity
            .is_some()
            && self.direct_llvm_worker_request_response_identity.is_some()
            && self.finalizer_post_link_isa_result_identity.is_some()
    }

    /// Identity-only inputs never grant compiler authority.
    pub const fn grants_compiler_authority(self) -> bool {
        false
    }
}

/// Bounded transport claim checked against one retained numerical correspondence.
///
/// Public fields permit hostile transport tests. Constructing this value grants
/// no authority; only the private checked binding records exact agreement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalLateMachineClaimV1 {
    pub schema_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_correspondence_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_claim_schema_identity: GeneralGemmEvidenceIdentityV1,
    pub kir_correspondence_identity: GeneralGemmEvidenceIdentityV1,
    pub kir_identity: GeneralGemmEvidenceIdentityV1,
    pub proof_request_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule: GeneralGemmProofScheduleV1,
    pub schedule_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
    pub target_identity: GeneralGemmEvidenceIdentityV1,
    pub compiler_toolchain_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule_model_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule_theorem_set_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule_source_closure_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_source_identity: GeneralGemmEvidenceIdentityV1,
    pub property_theorem_manifest_identity: GeneralGemmEvidenceIdentityV1,
    pub property_theorem_binding_identities:
        [GeneralGemmEvidenceIdentityV1; GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1],
    pub numerical_theorem_set_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_source_closure_identity: GeneralGemmEvidenceIdentityV1,
    pub reviewed_verus_tool_identity: GeneralGemmEvidenceIdentityV1,
    pub exhaustive_bf16_identity: GeneralGemmEvidenceIdentityV1,
    pub differential_fixture_identity: GeneralGemmEvidenceIdentityV1,
    pub properties: [GeneralGemmNumericalPropertyFactV1; GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1],
    pub mfma_contract_identity: GeneralGemmEvidenceIdentityV1,
    pub mfma_mnemonic: GeneralGemmGfx942MfmaMnemonicV1,
    pub open_machine_refinement_join_identity: GeneralGemmEvidenceIdentityV1,
    pub machine_join: GeneralGemmNumericalMachineJoinV1,
}

/// Exact late-machine claim field that failed comparison.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmNumericalLateMachineFieldV1 {
    SchemaIdentity,
    NumericalClaimSchemaIdentity,
    KirCorrespondenceIdentity,
    KirIdentity,
    ProofRequestIdentity,
    Schedule,
    ScheduleIdentity,
    NumericalPolicyIdentity,
    TargetIdentity,
    CompilerToolchainIdentity,
    ScheduleModelIdentity,
    ScheduleTheoremSetIdentity,
    ScheduleSourceClosureIdentity,
    NumericalSourceIdentity,
    PropertyTheoremManifestIdentity,
    PropertyTheoremBindingIdentities,
    NumericalTheoremSetIdentity,
    NumericalSourceClosureIdentity,
    ReviewedVerusToolIdentity,
    ExhaustiveBf16Identity,
    DifferentialFixtureIdentity,
    Properties,
    MfmaContractIdentity,
    MfmaMnemonic,
    OpenMachineRefinementJoinIdentity,
    OwnerBoundPlironGraphSerializationIdentity,
    DirectLlvmWorkerRequestResponseIdentity,
    FinalizerPostLinkIsaResultIdentity,
}

/// Failure to construct or check a late-machine identity binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmNumericalLateMachineErrorV1 {
    ZeroNumericalCorrespondenceIdentity,
    MissingMachineIdentity(GeneralGemmNumericalMachineIdentityAxisV1),
    ZeroMachineIdentity(GeneralGemmNumericalMachineIdentityAxisV1),
    NumericalCorrespondenceIdentityReused(GeneralGemmNumericalMachineIdentityAxisV1),
    MachineIdentityReused {
        first: GeneralGemmNumericalMachineIdentityAxisV1,
        second: GeneralGemmNumericalMachineIdentityAxisV1,
    },
    StaleOrMismatchedNumericalCorrespondenceIdentity,
    RetainedCorrespondenceInvariant(GeneralGemmNumericalLateMachineFieldV1),
    FieldMismatch(GeneralGemmNumericalLateMachineFieldV1),
}

impl fmt::Display for GeneralGemmNumericalLateMachineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM numerical late-machine binding failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmNumericalLateMachineErrorV1 {}

/// Opaque retained identity agreement with no compiler or artifact authority.
///
/// The verifier owns none of the concrete machine objects. Only the existing
/// rustc final join may promote this record, and only while retaining the exact
/// graph, worker request/response, and finalizer post-link owners represented by
/// the three identities.
///
/// ```compile_fail
/// fn duplicate(value: &fe2o3_verifier::GeneralGemmNumericalLateMachineBindingV1) {
///     let _copy: fe2o3_verifier::GeneralGemmNumericalLateMachineBindingV1 = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// fn forge() -> fe2o3_verifier::GeneralGemmNumericalLateMachineBindingV1 {
///     fe2o3_verifier::GeneralGemmNumericalLateMachineBindingV1 {
///         correspondence: todo!(),
///         machine_join: todo!(),
///         claim: todo!(),
///         identity: todo!(),
///     }
/// }
/// ```
#[derive(Debug)]
#[must_use = "identity agreement remains non-authoritative without concrete machine owners"]
pub struct GeneralGemmNumericalLateMachineBindingV1 {
    correspondence: GeneralGemmNumericalCorrespondenceV1,
    machine_join: GeneralGemmNumericalMachineJoinV1,
    claim: GeneralGemmNumericalLateMachineClaimV1,
    identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmNumericalLateMachineBindingV1 {
    pub const fn correspondence(&self) -> &GeneralGemmNumericalCorrespondenceV1 {
        &self.correspondence
    }

    pub const fn machine_join(&self) -> GeneralGemmNumericalMachineJoinV1 {
        self.machine_join
    }

    pub const fn claim(&self) -> GeneralGemmNumericalLateMachineClaimV1 {
        self.claim
    }

    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    /// Identity-only agreement never grants compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Derives the exact bounded transport claim for one retained correspondence.
///
/// `machine_join` remains untrusted identity-only input. This function verifies
/// presence, nonzero encoding, and contextual consistency, not producer custody.
pub fn derive_general_gemm_numerical_late_machine_claim_v1(
    correspondence: &GeneralGemmNumericalCorrespondenceV1,
    machine_join: GeneralGemmNumericalMachineJoinV1,
) -> Result<GeneralGemmNumericalLateMachineClaimV1, GeneralGemmNumericalLateMachineErrorV1> {
    validate_machine_join(machine_join)?;
    if machine_join.numerical_correspondence_identity != correspondence.identity() {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::StaleOrMismatchedNumericalCorrespondenceIdentity,
        );
    }
    validate_retained_correspondence(correspondence)?;
    let claim = correspondence.claim();
    Ok(GeneralGemmNumericalLateMachineClaimV1 {
        schema_identity: hash_parts(
            LATE_MACHINE_SCHEMA_DOMAIN_V1,
            &[GENERAL_GEMM_NUMERICAL_LATE_MACHINE_SCHEMA_V1.as_bytes()],
        ),
        numerical_correspondence_identity: correspondence.identity(),
        numerical_claim_schema_identity: claim.schema_identity,
        kir_correspondence_identity: claim.kir_correspondence_identity,
        kir_identity: claim.kir_identity,
        proof_request_identity: claim.proof_request_identity,
        schedule: claim.schedule,
        schedule_identity: claim.schedule_identity,
        numerical_policy_identity: claim.numerical_policy_identity,
        target_identity: claim.target_identity,
        compiler_toolchain_identity: claim.compiler_toolchain_identity,
        schedule_model_identity: claim.schedule_model_identity,
        schedule_theorem_set_identity: claim.schedule_theorem_set_identity,
        schedule_source_closure_identity: claim.schedule_source_closure_identity,
        numerical_source_identity: claim.numerical_source_identity,
        property_theorem_manifest_identity: claim.property_theorem_manifest_identity,
        property_theorem_binding_identities: claim.property_theorem_binding_identities,
        numerical_theorem_set_identity: claim.numerical_theorem_set_identity,
        numerical_source_closure_identity: claim.numerical_source_closure_identity,
        reviewed_verus_tool_identity: claim.reviewed_verus_tool_identity,
        exhaustive_bf16_identity: claim.exhaustive_bf16_identity,
        differential_fixture_identity: claim.differential_fixture_identity,
        properties: claim.properties,
        mfma_contract_identity: claim.mfma_contract_identity,
        mfma_mnemonic: GeneralGemmGfx942MfmaMnemonicV1::VmfmaF32_16x16x16Bf16,
        open_machine_refinement_join_identity: claim.machine_refinement_join_identity,
        machine_join,
    })
}

/// Checks a transported late-machine claim and consumes its retained correspondence.
///
/// The returned binding proves only exact identity agreement. It does not prove
/// that any identity came from the named machine object or that the emitted
/// machine behavior refines a `ModelOnly` or `Contracted` numerical property.
pub fn check_general_gemm_numerical_late_machine_binding_v1(
    correspondence: GeneralGemmNumericalCorrespondenceV1,
    machine_join: GeneralGemmNumericalMachineJoinV1,
    claimed: GeneralGemmNumericalLateMachineClaimV1,
) -> Result<GeneralGemmNumericalLateMachineBindingV1, GeneralGemmNumericalLateMachineErrorV1> {
    validate_machine_join(claimed.machine_join)?;
    if claimed.machine_join.numerical_correspondence_identity != correspondence.identity() {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::StaleOrMismatchedNumericalCorrespondenceIdentity,
        );
    }
    let expected =
        derive_general_gemm_numerical_late_machine_claim_v1(&correspondence, machine_join)?;
    compare_claims(expected, claimed)?;
    Ok(GeneralGemmNumericalLateMachineBindingV1 {
        correspondence,
        machine_join,
        claim: expected,
        identity: late_machine_binding_identity(expected),
    })
}

fn validate_machine_join(
    join: GeneralGemmNumericalMachineJoinV1,
) -> Result<(), GeneralGemmNumericalLateMachineErrorV1> {
    if join.numerical_correspondence_identity.as_bytes() == &[0; 32] {
        return Err(GeneralGemmNumericalLateMachineErrorV1::ZeroNumericalCorrespondenceIdentity);
    }
    let graph_axis = GeneralGemmNumericalMachineIdentityAxisV1::OwnerBoundPlironGraphSerialization;
    let worker_axis = GeneralGemmNumericalMachineIdentityAxisV1::DirectLlvmWorkerRequestResponse;
    let finalizer_axis = GeneralGemmNumericalMachineIdentityAxisV1::FinalizerPostLinkIsaResult;
    let graph = validate_machine_identity(
        join.owner_bound_pliron_graph_serialization_identity,
        graph_axis,
    )?;
    let worker = validate_machine_identity(
        join.direct_llvm_worker_request_response_identity,
        worker_axis,
    )?;
    let finalizer =
        validate_machine_identity(join.finalizer_post_link_isa_result_identity, finalizer_axis)?;
    for (axis, identity) in [
        (graph_axis, graph),
        (worker_axis, worker),
        (finalizer_axis, finalizer),
    ] {
        if identity == join.numerical_correspondence_identity {
            return Err(
                GeneralGemmNumericalLateMachineErrorV1::NumericalCorrespondenceIdentityReused(axis),
            );
        }
    }
    for (first, first_identity, second, second_identity) in [
        (graph_axis, graph, worker_axis, worker),
        (graph_axis, graph, finalizer_axis, finalizer),
        (worker_axis, worker, finalizer_axis, finalizer),
    ] {
        if first_identity == second_identity {
            return Err(
                GeneralGemmNumericalLateMachineErrorV1::MachineIdentityReused { first, second },
            );
        }
    }
    Ok(())
}

fn validate_machine_identity(
    identity: Option<GeneralGemmEvidenceIdentityV1>,
    axis: GeneralGemmNumericalMachineIdentityAxisV1,
) -> Result<GeneralGemmEvidenceIdentityV1, GeneralGemmNumericalLateMachineErrorV1> {
    let Some(identity) = identity else {
        return Err(GeneralGemmNumericalLateMachineErrorV1::MissingMachineIdentity(axis));
    };
    if identity.as_bytes() == &[0; 32] {
        return Err(GeneralGemmNumericalLateMachineErrorV1::ZeroMachineIdentity(
            axis,
        ));
    }
    Ok(identity)
}

fn validate_retained_correspondence(
    correspondence: &GeneralGemmNumericalCorrespondenceV1,
) -> Result<(), GeneralGemmNumericalLateMachineErrorV1> {
    use GeneralGemmNumericalLateMachineFieldV1 as Field;

    let claim = correspondence.claim();
    let manifest = correspondence.property_manifest();
    if claim.schema_identity
        != hash_parts(
            b"fe2o3-general-gemm-numerical-source-v1\0",
            &[GENERAL_GEMM_NUMERICAL_CORRESPONDENCE_SCHEMA_V1.as_bytes()],
        )
    {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::RetainedCorrespondenceInvariant(
                Field::NumericalClaimSchemaIdentity,
            ),
        );
    }
    if claim.property_theorem_manifest_identity != manifest.identity() {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::RetainedCorrespondenceInvariant(
                Field::PropertyTheoremManifestIdentity,
            ),
        );
    }
    if claim.numerical_theorem_set_identity != manifest.theorem_set_identity() {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::RetainedCorrespondenceInvariant(
                Field::NumericalTheoremSetIdentity,
            ),
        );
    }
    for (index, binding) in manifest.bindings().iter().copied().enumerate() {
        if claim.property_theorem_binding_identities[index] != binding.statement_source_identity() {
            return Err(
                GeneralGemmNumericalLateMachineErrorV1::RetainedCorrespondenceInvariant(
                    Field::PropertyTheoremBindingIdentities,
                ),
            );
        }
        if claim.properties[index] != binding.fact() {
            return Err(
                GeneralGemmNumericalLateMachineErrorV1::RetainedCorrespondenceInvariant(
                    Field::Properties,
                ),
            );
        }
    }
    let mfma = claim.mfma_contract;
    if mfma.target != "gfx942:xnack-:wave64"
        || mfma.llvm_intrinsic != GENERAL_GEMM_GFX942_MFMA_INTRINSIC_V1
        || mfma.isa_mnemonic != GENERAL_GEMM_GFX942_MFMA_MNEMONIC_V1
        || mfma.matrix_shape != [16, 16, 16]
        || mfma.wave_lanes != 64
        || mfma.accumulators_per_lane != 4
        || mfma.control_immediates != [0, 0, 0]
        || mfma.input_element_bits != 16
        || mfma.accumulator_element_bits != 32
        || mfma.numerical_status != GeneralGemmNumericalCorrespondenceStatusV1::Contracted
    {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::RetainedCorrespondenceInvariant(
                Field::MfmaContractIdentity,
            ),
        );
    }
    let future = claim.machine_refinement_join;
    if future.required_inputs
        != [
            GeneralGemmFutureMachineRefinementInputV1::OwnerBoundPlironGraph,
            GeneralGemmFutureMachineRefinementInputV1::DirectLlvmWorkerRequestResponse,
            GeneralGemmFutureMachineRefinementInputV1::FinalizerPostLinkIsaResult,
        ]
        || future.has_all_required_input_identities()
        || future.status != GeneralGemmNumericalCorrespondenceStatusV1::Unsupported
    {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::RetainedCorrespondenceInvariant(
                Field::OpenMachineRefinementJoinIdentity,
            ),
        );
    }
    Ok(())
}

macro_rules! require_field {
    ($expected:expr, $claimed:expr, $field:ident, $name:ident) => {
        if ($expected).$field != ($claimed).$field {
            return Err(GeneralGemmNumericalLateMachineErrorV1::FieldMismatch(
                Field::$name,
            ));
        }
    };
}

fn compare_claims(
    expected: GeneralGemmNumericalLateMachineClaimV1,
    claimed: GeneralGemmNumericalLateMachineClaimV1,
) -> Result<(), GeneralGemmNumericalLateMachineErrorV1> {
    use GeneralGemmNumericalLateMachineFieldV1 as Field;

    if expected.numerical_correspondence_identity != claimed.numerical_correspondence_identity {
        return Err(
            GeneralGemmNumericalLateMachineErrorV1::StaleOrMismatchedNumericalCorrespondenceIdentity,
        );
    }
    require_field!(expected, claimed, schema_identity, SchemaIdentity);
    require_field!(
        expected,
        claimed,
        numerical_claim_schema_identity,
        NumericalClaimSchemaIdentity
    );
    require_field!(
        expected,
        claimed,
        kir_correspondence_identity,
        KirCorrespondenceIdentity
    );
    require_field!(expected, claimed, kir_identity, KirIdentity);
    require_field!(
        expected,
        claimed,
        proof_request_identity,
        ProofRequestIdentity
    );
    require_field!(expected, claimed, schedule, Schedule);
    require_field!(expected, claimed, schedule_identity, ScheduleIdentity);
    require_field!(
        expected,
        claimed,
        numerical_policy_identity,
        NumericalPolicyIdentity
    );
    require_field!(expected, claimed, target_identity, TargetIdentity);
    require_field!(
        expected,
        claimed,
        compiler_toolchain_identity,
        CompilerToolchainIdentity
    );
    require_field!(
        expected,
        claimed,
        schedule_model_identity,
        ScheduleModelIdentity
    );
    require_field!(
        expected,
        claimed,
        schedule_theorem_set_identity,
        ScheduleTheoremSetIdentity
    );
    require_field!(
        expected,
        claimed,
        schedule_source_closure_identity,
        ScheduleSourceClosureIdentity
    );
    require_field!(
        expected,
        claimed,
        numerical_source_identity,
        NumericalSourceIdentity
    );
    require_field!(
        expected,
        claimed,
        property_theorem_manifest_identity,
        PropertyTheoremManifestIdentity
    );
    require_field!(
        expected,
        claimed,
        property_theorem_binding_identities,
        PropertyTheoremBindingIdentities
    );
    require_field!(
        expected,
        claimed,
        numerical_theorem_set_identity,
        NumericalTheoremSetIdentity
    );
    require_field!(
        expected,
        claimed,
        numerical_source_closure_identity,
        NumericalSourceClosureIdentity
    );
    require_field!(
        expected,
        claimed,
        reviewed_verus_tool_identity,
        ReviewedVerusToolIdentity
    );
    require_field!(
        expected,
        claimed,
        exhaustive_bf16_identity,
        ExhaustiveBf16Identity
    );
    require_field!(
        expected,
        claimed,
        differential_fixture_identity,
        DifferentialFixtureIdentity
    );
    require_field!(expected, claimed, properties, Properties);
    require_field!(
        expected,
        claimed,
        mfma_contract_identity,
        MfmaContractIdentity
    );
    require_field!(expected, claimed, mfma_mnemonic, MfmaMnemonic);
    require_field!(
        expected,
        claimed,
        open_machine_refinement_join_identity,
        OpenMachineRefinementJoinIdentity
    );
    require_field!(
        expected.machine_join,
        claimed.machine_join,
        owner_bound_pliron_graph_serialization_identity,
        OwnerBoundPlironGraphSerializationIdentity
    );
    require_field!(
        expected.machine_join,
        claimed.machine_join,
        direct_llvm_worker_request_response_identity,
        DirectLlvmWorkerRequestResponseIdentity
    );
    require_field!(
        expected.machine_join,
        claimed.machine_join,
        finalizer_post_link_isa_result_identity,
        FinalizerPostLinkIsaResultIdentity
    );
    Ok(())
}

fn late_machine_binding_identity(
    claim: GeneralGemmNumericalLateMachineClaimV1,
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(LATE_MACHINE_BINDING_DOMAIN_V1);
    for identity in [
        claim.machine_join.numerical_correspondence_identity,
        claim.schema_identity,
        claim.numerical_correspondence_identity,
        claim.numerical_claim_schema_identity,
        claim.kir_correspondence_identity,
        claim.kir_identity,
        claim.proof_request_identity,
        claim.schedule_identity,
        claim.numerical_policy_identity,
        claim.target_identity,
        claim.compiler_toolchain_identity,
        claim.schedule_model_identity,
        claim.schedule_theorem_set_identity,
        claim.schedule_source_closure_identity,
        claim.numerical_source_identity,
        claim.property_theorem_manifest_identity,
        claim.numerical_theorem_set_identity,
        claim.numerical_source_closure_identity,
        claim.reviewed_verus_tool_identity,
        claim.exhaustive_bf16_identity,
        claim.differential_fixture_identity,
        claim.mfma_contract_identity,
        claim.open_machine_refinement_join_identity,
    ] {
        hasher.update(identity.as_bytes());
    }
    for identity in claim.property_theorem_binding_identities {
        hasher.update(identity.as_bytes());
    }
    for fact in claim.properties {
        hasher.update([fact.property as u8, fact.status as u8, fact.basis as u8]);
    }
    hasher.update([claim.schedule as u8, claim.mfma_mnemonic as u8]);
    for identity in [
        claim
            .machine_join
            .owner_bound_pliron_graph_serialization_identity,
        claim
            .machine_join
            .direct_llvm_worker_request_response_identity,
        claim.machine_join.finalizer_post_link_isa_result_identity,
    ]
    .into_iter()
    .flatten()
    {
        hasher.update(identity.as_bytes());
    }
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}
