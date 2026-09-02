//! Independently checked composition of source-to-MIR and MIR-to-KIR scalar evidence.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    BinaryOp, FunctionId, Module, Operation, OperationKind, ScalarType, Type, ValueId,
};
use fe2o3_mir_model::{
    InertSourceMirScalarRefinementEvidenceV1, SourceMirLocalBindingV1, SourceMirScalarOperatorV1,
    semantic_mir_v1::{
        SemanticLocalRoleV1, SemanticScalarTypeV1, SemanticSourceOriginV1, SemanticTypeShapeV1,
    },
};
use sha2::{Digest, Sha256};

use crate::{
    InertMirKirScalarRefinementEvidenceV1, MirKirScalarOperatorV1, MirKirScalarSemanticOperandV1,
    ProductionCanonicalKernelIrIdentityV1, ProductionSemanticKirOwnerV1,
};

/// Model version for the first mechanically checked source-to-KIR composition.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_MODEL_VERSION_V2: u16 = 3;
/// Closed independently checked production join policy.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_POLICY_V2: u16 = 3;
/// Stable name of the Verus composition theorem.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_THEOREM_V2: &str =
    "fe2o3_source_mir_kir_u32_element_refines_v2";
/// SHA-256 of the exact Verus composition source.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_PROOF_SHA256_V2: [u8; 32] = [
    0x63, 0x98, 0xec, 0x11, 0x72, 0x25, 0x42, 0xe7, 0xbc, 0xdb, 0x4f, 0x7b, 0xa7, 0xcc, 0xf6, 0xc4,
    0x75, 0xe2, 0x85, 0x9f, 0x46, 0xbd, 0x5f, 0x79, 0x12, 0x62, 0x69, 0x5f, 0x79, 0x9e, 0xe4, 0x8d,
];
/// SHA-256 of the pinned Verus executable.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_VERUS_SHA256_V2: [u8; 32] = [
    0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80, 0xa1, 0xda,
    0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0, 0xc9, 0xf3, 0x82, 0xdd,
];
/// SHA-256 of the complete pinned Verus/vstd/Z3 closure manifest.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_CLOSURE_SHA256_V2: [u8; 32] = [
    0xd2, 0x8d, 0xf3, 0xfb, 0x5e, 0x0d, 0x74, 0x76, 0x37, 0x54, 0x39, 0x33, 0xdf, 0xc3, 0x8c, 0xff,
    0x45, 0x57, 0x6d, 0xa9, 0xb9, 0x20, 0xd7, 0x55, 0xb4, 0xb7, 0xe9, 0x19, 0xe4, 0x7a, 0x60, 0x19,
];

const MODEL_DOMAIN_V2: &[u8] = b"FE2O3/SOURCE-MIR-KIR/U32-COMPOSITION-MODEL/V3\0";
const EVIDENCE_DOMAIN_V2: &[u8] = b"FE2O3/SOURCE-MIR-KIR/U32-COMPOSITION-EVIDENCE/V3\0";

/// One exact source expression, semantic-MIR statement, and KIR operation joined by the checker.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceMirKirScalarCompositionStepV2 {
    source_expression_sha256: [u8; 32],
    semantic_function: u32,
    semantic_block: u32,
    semantic_statement: u32,
    correspondence_owner: u32,
    operator: MirKirScalarOperatorV1,
    left: SourceMirLocalBindingV1,
    right: SourceMirLocalBindingV1,
    destination: SourceMirLocalBindingV1,
    kernel_ir_block: u32,
    kernel_ir_operation: u32,
    kernel_ir_left: ValueId,
    kernel_ir_right: ValueId,
    kernel_ir_result: ValueId,
}

impl SourceMirKirScalarCompositionStepV2 {
    /// Returns the exact same-session HIR expression identity.
    pub const fn source_expression_sha256(&self) -> &[u8; 32] {
        &self.source_expression_sha256
    }
    /// Returns the exact admitted semantic function index.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }
    /// Returns the exact admitted semantic block index.
    pub const fn semantic_block(&self) -> u32 {
        self.semantic_block
    }
    /// Returns the exact admitted semantic statement ordinal.
    pub const fn semantic_statement(&self) -> u32 {
        self.semantic_statement
    }
    /// Returns the exact semantic root that owns this KIR function instance.
    pub const fn correspondence_owner(&self) -> u32 {
        self.correspondence_owner
    }
    /// Returns the common classified scalar operator.
    pub const fn operator(&self) -> MirKirScalarOperatorV1 {
        self.operator
    }
    /// Returns the source/raw-MIR/semantic left binding.
    pub const fn left(&self) -> SourceMirLocalBindingV1 {
        self.left
    }
    /// Returns the source/raw-MIR/semantic right binding.
    pub const fn right(&self) -> SourceMirLocalBindingV1 {
        self.right
    }
    /// Returns the source-result/raw-MIR/semantic destination binding.
    pub const fn destination(&self) -> SourceMirLocalBindingV1 {
        self.destination
    }
    /// Returns the exact KIR block index.
    pub const fn kernel_ir_block(&self) -> u32 {
        self.kernel_ir_block
    }
    /// Returns the exact KIR operation ordinal.
    pub const fn kernel_ir_operation(&self) -> u32 {
        self.kernel_ir_operation
    }
    /// Returns the exact ordered KIR operand SSA identities.
    pub const fn kernel_ir_operands(&self) -> (ValueId, ValueId) {
        (self.kernel_ir_left, self.kernel_ir_right)
    }
    /// Returns the exact KIR result SSA identity.
    pub const fn kernel_ir_result(&self) -> ValueId {
        self.kernel_ir_result
    }
}

/// Authority-free exact composition evidence derived from two independently rechecked boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertSourceMirKirScalarCompositionEvidenceV2 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    model_identity: [u8; 32],
    source_mir_evidence_identity: [u8; 32],
    mir_kir_evidence_identity: [u8; 32],
    rustc_hir_owner_sha256: [u8; 32],
    rustc_mir_body_sha256: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    source_expansion: SemanticSourceOriginV1,
    source_call_site: SemanticSourceOriginV1,
    eligible_candidates: u32,
    steps: Box<[SourceMirKirScalarCompositionStepV2]>,
}

impl InertSourceMirKirScalarCompositionEvidenceV2 {
    /// Independently joins source evidence to a revalidated live production KIR owner.
    pub fn from_live_production(
        source: &InertSourceMirScalarRefinementEvidenceV1,
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<Self, SourceMirKirScalarCompositionErrorV2> {
        source
            .revalidate()
            .map_err(|error| SourceMirKirScalarCompositionErrorV2::Source(error.to_string()))?;
        owner
            .verify_equivalence()
            .map_err(|error| SourceMirKirScalarCompositionErrorV2::LiveOwner(error.to_string()))?;
        let semantic = owner.semantic().semantic();
        validate_semantic_identity_v2(
            source.semantic_mir_sha256(),
            semantic.semantic_sha256().as_bytes(),
        )?;
        let mir_kir = InertMirKirScalarRefinementEvidenceV1::from_live_owner(owner)
            .map_err(|error| SourceMirKirScalarCompositionErrorV2::MirKir(error.to_string()))?;
        validate_semantic_identity_v2(mir_kir.semantic_sha256(), source.semantic_mir_sha256())?;
        if mir_kir.canonical_kernel_ir_identity() != owner.canonical_kernel_ir_identity() {
            return Err(SourceMirKirScalarCompositionErrorV2::SemanticIdentityMismatch);
        }

        let mut steps = Vec::with_capacity(source.certificates().len());
        for certificate in source.certificates() {
            validate_source_local_v2(
                semantic,
                certificate.semantic_function(),
                certificate.left(),
                true,
            )?;
            validate_source_local_v2(
                semantic,
                certificate.semantic_function(),
                certificate.right(),
                true,
            )?;
            validate_source_local_v2(
                semantic,
                certificate.semantic_function(),
                certificate.destination(),
                false,
            )?;
            let operator = joined_operator_v2(certificate.operator());
            let matching = mir_kir
                .certificates()
                .iter()
                .filter(|candidate| {
                    candidate.semantic_function() == certificate.semantic_function()
                        && candidate.semantic_block() == certificate.semantic_block()
                        && candidate.semantic_statement() == certificate.semantic_statement()
                        && candidate.operator() == operator
                        && candidate.semantic_left()
                            == MirKirScalarSemanticOperandV1::Parameter(
                                certificate.left().semantic_local().index(),
                            )
                        && candidate.semantic_right()
                            == MirKirScalarSemanticOperandV1::Parameter(
                                certificate.right().semantic_local().index(),
                            )
                        && candidate.semantic_destination()
                            == certificate.destination().semantic_local().index()
                })
                .collect::<Vec<_>>();
            let [mapped] = matching.as_slice() else {
                return Err(if matching.is_empty() {
                    SourceMirKirScalarCompositionErrorV2::MissingExactJoin
                } else {
                    SourceMirKirScalarCompositionErrorV2::AmbiguousExactJoin
                });
            };
            let step = SourceMirKirScalarCompositionStepV2 {
                source_expression_sha256: *certificate.source_expression_sha256(),
                semantic_function: certificate.semantic_function(),
                semantic_block: certificate.semantic_block(),
                semantic_statement: certificate.semantic_statement(),
                correspondence_owner: mapped.correspondence_owner(),
                operator,
                left: certificate.left(),
                right: certificate.right(),
                destination: certificate.destination(),
                kernel_ir_block: mapped.kernel_ir_block(),
                kernel_ir_operation: mapped.kernel_ir_operation(),
                kernel_ir_left: mapped.kernel_ir_left(),
                kernel_ir_right: mapped.kernel_ir_right(),
                kernel_ir_result: mapped.kernel_ir_result(),
            };
            validate_live_kir_step_v2(owner, &step)?;
            steps.push(step);
        }
        steps.sort();
        if steps.is_empty() || steps.windows(2).any(|window| window[0] == window[1]) {
            return Err(SourceMirKirScalarCompositionErrorV2::InvalidStepRoster);
        }
        let model_identity = source_mir_kir_scalar_composition_model_identity_v2();
        let source_mir_evidence_identity = *source.identity();
        let mir_kir_evidence_identity = *mir_kir.identity();
        let rustc_hir_owner_sha256 = *source.rustc_hir_owner_sha256();
        let rustc_mir_body_sha256 = *source.rustc_mir_body_sha256();
        let semantic_mir_sha256 = *source.semantic_mir_sha256();
        let canonical_kernel_ir = owner.canonical_kernel_ir_identity();
        let source_expansion = source
            .source()
            .expansion()
            .ok_or(SourceMirKirScalarCompositionErrorV2::InvalidStepRoster)?;
        let source_call_site = source
            .source()
            .call_site()
            .ok_or(SourceMirKirScalarCompositionErrorV2::InvalidStepRoster)?;
        let eligible_candidates = u32::try_from(source.certificates().len())
            .map_err(|_| SourceMirKirScalarCompositionErrorV2::Overflow)?;
        if eligible_candidates == 0 || steps.len() != eligible_candidates as usize {
            return Err(SourceMirKirScalarCompositionErrorV2::InvalidStepRoster);
        }
        let canonical_bytes = encode_v2(
            model_identity,
            source_mir_evidence_identity,
            mir_kir_evidence_identity,
            rustc_hir_owner_sha256,
            rustc_mir_body_sha256,
            semantic_mir_sha256,
            canonical_kernel_ir,
            Some(source_expansion),
            Some(source_call_site),
            eligible_candidates,
            &steps,
        )?;
        let identity = evidence_identity_v2(&canonical_bytes);
        let evidence = Self {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            identity,
            model_identity,
            source_mir_evidence_identity,
            mir_kir_evidence_identity,
            rustc_hir_owner_sha256,
            rustc_mir_body_sha256,
            semantic_mir_sha256,
            canonical_kernel_ir,
            source_expansion,
            source_call_site,
            eligible_candidates,
            steps: steps.into_boxed_slice(),
        };
        evidence.revalidate()?;
        Ok(evidence)
    }

    /// Rechecks the unique encoding and every retained proof/input identity.
    pub fn revalidate(&self) -> Result<(), SourceMirKirScalarCompositionErrorV2> {
        if self.model_identity != source_mir_kir_scalar_composition_model_identity_v2()
            || self.source_mir_evidence_identity == [0; 32]
            || self.mir_kir_evidence_identity == [0; 32]
            || self.rustc_hir_owner_sha256 == [0; 32]
            || self.rustc_mir_body_sha256 == [0; 32]
            || self.semantic_mir_sha256 == [0; 32]
            || self.canonical_kernel_ir.digest() == &[0; 32]
            || self.canonical_kernel_ir.canonical_length() == 0
            || self.steps.is_empty()
            || self.eligible_candidates == 0
            || self.steps.len() != self.eligible_candidates as usize
            || self.steps.windows(2).any(|window| window[0] >= window[1])
            || self.steps.iter().any(|step| {
                step.source_expression_sha256 == [0; 32]
                    || step.left.source_binding_sha256() == &[0; 32]
                    || step.right.source_binding_sha256() == &[0; 32]
                    || step.destination.source_binding_sha256() == &[0; 32]
                    || step.left.semantic_local() == step.destination.semantic_local()
                    || step.right.semantic_local() == step.destination.semantic_local()
                    || step.kernel_ir_left == step.kernel_ir_result
                    || step.kernel_ir_right == step.kernel_ir_result
            })
        {
            return Err(SourceMirKirScalarCompositionErrorV2::NonCanonical);
        }
        // Source coordinates are already bound into the nested source evidence identity.
        let encoded = encode_v2(
            self.model_identity,
            self.source_mir_evidence_identity,
            self.mir_kir_evidence_identity,
            self.rustc_hir_owner_sha256,
            self.rustc_mir_body_sha256,
            self.semantic_mir_sha256,
            self.canonical_kernel_ir,
            Some(self.source_expansion),
            Some(self.source_call_site),
            self.eligible_candidates,
            &self.steps,
        )?;
        if encoded.as_slice() != self.canonical_bytes.as_ref()
            || evidence_identity_v2(&encoded) != self.identity
        {
            return Err(SourceMirKirScalarCompositionErrorV2::NonCanonical);
        }
        Ok(())
    }

    /// Returns the canonical authority-free evidence bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Returns the domain-separated composition identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    /// Returns the exact nested source-to-MIR evidence identity.
    pub const fn source_mir_evidence_identity(&self) -> &[u8; 32] {
        &self.source_mir_evidence_identity
    }
    /// Returns the exact nested MIR-to-KIR evidence identity.
    pub const fn mir_kir_evidence_identity(&self) -> &[u8; 32] {
        &self.mir_kir_evidence_identity
    }
    /// Returns the exact admitted semantic-MIR identity shared by both boundaries.
    pub const fn semantic_mir_sha256(&self) -> &[u8; 32] {
        &self.semantic_mir_sha256
    }
    /// Returns the exact canonical production KIR identity.
    pub const fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.canonical_kernel_ir
    }
    /// Returns every exact non-vacuous composed step.
    pub fn steps(&self) -> &[SourceMirKirScalarCompositionStepV2] {
        &self.steps
    }
    /// Returns the exact nonzero source-candidate count covered by this record.
    pub const fn eligible_candidates(&self) -> u32 {
        self.eligible_candidates
    }
    /// Composition evidence never grants compiler, artifact, LLVM, runtime, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Returns the domain-separated identity of the executable checker and formal theorem.
pub fn source_mir_kir_scalar_composition_model_identity_v2() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MODEL_DOMAIN_V2);
    hash.update(SOURCE_MIR_KIR_SCALAR_COMPOSITION_MODEL_VERSION_V2.to_le_bytes());
    hash.update(SOURCE_MIR_KIR_SCALAR_COMPOSITION_POLICY_V2.to_le_bytes());
    hash.update(SOURCE_MIR_KIR_SCALAR_COMPOSITION_THEOREM_V2.as_bytes());
    hash.update(SOURCE_MIR_KIR_SCALAR_COMPOSITION_PROOF_SHA256_V2);
    hash.update(SOURCE_MIR_KIR_SCALAR_COMPOSITION_VERUS_SHA256_V2);
    hash.update(SOURCE_MIR_KIR_SCALAR_COMPOSITION_CLOSURE_SHA256_V2);
    hash.finalize().into()
}

/// Failure while independently joining the two production evidence boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceMirKirScalarCompositionErrorV2 {
    /// Nested source-to-MIR evidence did not revalidate.
    Source(String),
    /// The live semantic/KIR owner did not replay.
    LiveOwner(String),
    /// Parameter-rooted MIR-to-KIR evidence could not be derived.
    MirKir(String),
    /// The two evidence boundaries name different admitted semantic MIR.
    SemanticIdentityMismatch,
    /// A source/raw-MIR/semantic local axis differs from the live semantic owner.
    SourceLocalMismatch,
    /// A source expression has no exact parameter-rooted KIR step.
    MissingExactJoin,
    /// A source expression maps to more than one KIR step.
    AmbiguousExactJoin,
    /// A retained operand or result SSA does not name the live KIR operation.
    LiveStepMismatch,
    /// The composed step roster is empty or duplicated.
    InvalidStepRoster,
    /// A bounded encoding coordinate overflowed.
    Overflow,
    /// Retained bytes, ordering, or identities are not canonical.
    NonCanonical,
}

impl fmt::Display for SourceMirKirScalarCompositionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "source-to-MIR evidence failed: {error}"),
            Self::LiveOwner(error) => write!(formatter, "live semantic/KIR owner failed: {error}"),
            Self::MirKir(error) => write!(
                formatter,
                "parameter-rooted MIR-to-KIR evidence failed: {error}"
            ),
            Self::SemanticIdentityMismatch => {
                formatter.write_str("source and KIR boundaries name different semantic MIR")
            }
            Self::SourceLocalMismatch => formatter
                .write_str("source/raw-MIR/semantic local binding differs from the live owner"),
            Self::MissingExactJoin => formatter
                .write_str("source scalar expression has no exact parameter-rooted KIR step"),
            Self::AmbiguousExactJoin => {
                formatter.write_str("source scalar expression has an ambiguous KIR step")
            }
            Self::LiveStepMismatch => formatter
                .write_str("composed operand/result SSA does not match the live KIR operation"),
            Self::InvalidStepRoster => {
                formatter.write_str("composed scalar step roster is empty or duplicated")
            }
            Self::Overflow => formatter.write_str("composition encoding overflowed"),
            Self::NonCanonical => formatter.write_str("composition evidence is not canonical"),
        }
    }
}

impl Error for SourceMirKirScalarCompositionErrorV2 {}

fn joined_operator_v2(operator: SourceMirScalarOperatorV1) -> MirKirScalarOperatorV1 {
    match operator {
        SourceMirScalarOperatorV1::Add => MirKirScalarOperatorV1::Add,
        SourceMirScalarOperatorV1::Subtract => MirKirScalarOperatorV1::Subtract,
        SourceMirScalarOperatorV1::Multiply => MirKirScalarOperatorV1::Multiply,
        SourceMirScalarOperatorV1::BitAnd => MirKirScalarOperatorV1::BitAnd,
        SourceMirScalarOperatorV1::BitOr => MirKirScalarOperatorV1::BitOr,
        SourceMirScalarOperatorV1::BitXor => MirKirScalarOperatorV1::BitXor,
    }
}

fn validate_semantic_identity_v2(
    left: &[u8; 32],
    right: &[u8; 32],
) -> Result<(), SourceMirKirScalarCompositionErrorV2> {
    if left == &[0; 32] || right == &[0; 32] || left != right {
        return Err(SourceMirKirScalarCompositionErrorV2::SemanticIdentityMismatch);
    }
    Ok(())
}

fn kernel_operator_v2(operator: MirKirScalarOperatorV1) -> BinaryOp {
    match operator {
        MirKirScalarOperatorV1::Add => BinaryOp::Add,
        MirKirScalarOperatorV1::Subtract => BinaryOp::Subtract,
        MirKirScalarOperatorV1::Multiply => BinaryOp::Multiply,
        MirKirScalarOperatorV1::BitAnd => BinaryOp::BitAnd,
        MirKirScalarOperatorV1::BitOr => BinaryOp::BitOr,
        MirKirScalarOperatorV1::BitXor => BinaryOp::BitXor,
    }
}

fn validate_joined_kir_operation_v2(
    step: &SourceMirKirScalarCompositionStepV2,
    operation: &Operation,
) -> Result<(), SourceMirKirScalarCompositionErrorV2> {
    let OperationKind::Binary { op, lhs, rhs } = operation.kind else {
        return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
    };
    let [result] = operation.results.as_slice() else {
        return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
    };
    if op != kernel_operator_v2(step.operator)
        || lhs != step.kernel_ir_left
        || rhs != step.kernel_ir_right
        || result.id != step.kernel_ir_result
        || result.id == lhs
        || result.id == rhs
        || result.ty != Type::Scalar(ScalarType::U32)
        || !operation.memory_effects().is_empty()
    {
        return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveKirFunctionBindingV2 {
    correspondence_owner: u32,
    semantic_function: u32,
    kernel_ir_function: FunctionId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiveKirStatementSpanV2 {
    correspondence_owner: u32,
    semantic_function: u32,
    semantic_block: u32,
    semantic_statement: u32,
    kernel_ir_block: u32,
    kernel_ir_operation: u32,
    operation_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiveKirParameterBindingV2 {
    correspondence_owner: u32,
    semantic_function: u32,
    semantic_local: u32,
    kernel_ir_value: ValueId,
}

fn validate_live_kir_step_v2(
    owner: &ProductionSemanticKirOwnerV1,
    step: &SourceMirKirScalarCompositionStepV2,
) -> Result<(), SourceMirKirScalarCompositionErrorV2> {
    let functions = owner
        .correspondence()
        .lowered_functions()
        .iter()
        .map(|binding| LiveKirFunctionBindingV2 {
            correspondence_owner: binding.correspondence_owner().index(),
            semantic_function: binding.semantic_function().index(),
            kernel_ir_function: binding.kernel_ir_function().clone(),
        })
        .collect::<Vec<_>>();
    let spans = owner
        .correspondence()
        .statement_operation_spans()
        .iter()
        .copied()
        .map(|span| LiveKirStatementSpanV2 {
            correspondence_owner: span.correspondence_owner().index(),
            semantic_function: span.semantic_function().index(),
            semantic_block: span.semantic_block().index(),
            semantic_statement: span.statement_ordinal(),
            kernel_ir_block: span.kernel_ir_block().0,
            kernel_ir_operation: span.first_operation_ordinal(),
            operation_count: span.operation_count(),
        })
        .collect::<Vec<_>>();
    let parameters = owner
        .correspondence()
        .parameter_bindings()
        .iter()
        .copied()
        .map(|binding| LiveKirParameterBindingV2 {
            correspondence_owner: binding.correspondence_owner().index(),
            semantic_function: binding.semantic_function().index(),
            semantic_local: binding.semantic_local().index(),
            kernel_ir_value: binding.kernel_ir_value(),
        })
        .collect::<Vec<_>>();
    validate_live_kir_step_lookup_v2(owner.module(), &functions, &spans, &parameters, step)
}

fn validate_live_kir_step_lookup_v2(
    module: &Module,
    functions: &[LiveKirFunctionBindingV2],
    spans: &[LiveKirStatementSpanV2],
    parameters: &[LiveKirParameterBindingV2],
    step: &SourceMirKirScalarCompositionStepV2,
) -> Result<(), SourceMirKirScalarCompositionErrorV2> {
    let functions = functions
        .iter()
        .filter(|binding| {
            binding.correspondence_owner == step.correspondence_owner
                && binding.semantic_function == step.semantic_function
        })
        .collect::<Vec<_>>();
    let [function_binding] = functions.as_slice() else {
        return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
    };
    let spans = spans
        .iter()
        .copied()
        .filter(|span| {
            span.correspondence_owner == step.correspondence_owner
                && span.semantic_function == step.semantic_function
                && span.semantic_block == step.semantic_block
                && span.semantic_statement == step.semantic_statement
        })
        .collect::<Vec<_>>();
    let [span] = spans.as_slice() else {
        return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
    };
    if span.kernel_ir_block != step.kernel_ir_block
        || span.operation_count != 1
        || span.kernel_ir_operation != step.kernel_ir_operation
    {
        return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
    }
    for (binding, local, value) in [
        (
            &step.left,
            step.left.semantic_local().index(),
            step.kernel_ir_left,
        ),
        (
            &step.right,
            step.right.semantic_local().index(),
            step.kernel_ir_right,
        ),
    ] {
        let parameter_bindings = parameters
            .iter()
            .copied()
            .filter(|candidate| {
                candidate.correspondence_owner == step.correspondence_owner
                    && candidate.semantic_function == step.semantic_function
                    && candidate.semantic_local == local
            })
            .collect::<Vec<_>>();
        let [parameter] = parameter_bindings.as_slice() else {
            return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
        };
        if binding.semantic_local().index() != local || parameter.kernel_ir_value != value {
            return Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch);
        }
    }
    let operation = module
        .function(&function_binding.kernel_ir_function)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| {
            body.blocks
                .iter()
                .find(|block| block.id.0 == step.kernel_ir_block)
        })
        .and_then(|block| block.operations.get(step.kernel_ir_operation as usize))
        .ok_or(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch)?;
    validate_joined_kir_operation_v2(step, operation)
}

fn validate_source_local_v2(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    function: u32,
    binding: SourceMirLocalBindingV1,
    require_parameter: bool,
) -> Result<(), SourceMirKirScalarCompositionErrorV2> {
    let declaration = semantic
        .functions()
        .get(function as usize)
        .and_then(|function| {
            function
                .locals()
                .get(binding.semantic_local().index() as usize)
        })
        .ok_or(SourceMirKirScalarCompositionErrorV2::SourceLocalMismatch)?;
    if declaration.identity().as_bytes() != binding.semantic_local_identity()
        || binding.source_binding_sha256() == &[0; 32]
        || !matches!(
            semantic
                .types()
                .get(declaration.ty().index() as usize)
                .map(|declaration| declaration.shape()),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }))
        )
        || (require_parameter && !matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)))
    {
        return Err(SourceMirKirScalarCompositionErrorV2::SourceLocalMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_v2(
    model_identity: [u8; 32],
    source_mir_evidence_identity: [u8; 32],
    mir_kir_evidence_identity: [u8; 32],
    rustc_hir_owner_sha256: [u8; 32],
    rustc_mir_body_sha256: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    expansion: Option<SemanticSourceOriginV1>,
    call_site: Option<SemanticSourceOriginV1>,
    eligible_candidates: u32,
    steps: &[SourceMirKirScalarCompositionStepV2],
) -> Result<Vec<u8>, SourceMirKirScalarCompositionErrorV2> {
    let count =
        u32::try_from(steps.len()).map_err(|_| SourceMirKirScalarCompositionErrorV2::Overflow)?;
    let mut bytes = Vec::with_capacity(320 + steps.len() * 260);
    bytes.extend_from_slice(b"F2SMKC3\0");
    bytes.extend_from_slice(&SOURCE_MIR_KIR_SCALAR_COMPOSITION_MODEL_VERSION_V2.to_le_bytes());
    bytes.extend_from_slice(&SOURCE_MIR_KIR_SCALAR_COMPOSITION_POLICY_V2.to_le_bytes());
    for identity in [
        model_identity,
        source_mir_evidence_identity,
        mir_kir_evidence_identity,
        rustc_hir_owner_sha256,
        rustc_mir_body_sha256,
        semantic_mir_sha256,
    ] {
        bytes.extend_from_slice(&identity);
    }
    bytes.extend_from_slice(canonical_kernel_ir.digest());
    bytes.extend_from_slice(&canonical_kernel_ir.canonical_length().to_le_bytes());
    bytes.push(match canonical_kernel_ir.version() {
        crate::ProductionCanonicalKernelIrVersionV1::V8 => 8,
        crate::ProductionCanonicalKernelIrVersionV1::V9 => 9,
    });
    encode_origin_v2(&mut bytes, expansion);
    encode_origin_v2(&mut bytes, call_site);
    bytes.extend_from_slice(&eligible_candidates.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    for step in steps {
        bytes.extend_from_slice(&step.source_expression_sha256);
        for value in [
            step.semantic_function,
            step.semantic_block,
            step.semantic_statement,
            step.correspondence_owner,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(step.operator as u8);
        for binding in [step.left, step.right, step.destination] {
            bytes.extend_from_slice(binding.source_binding_sha256());
            bytes.extend_from_slice(&binding.rustc_mir_local().to_le_bytes());
            bytes.extend_from_slice(&binding.semantic_local().index().to_le_bytes());
            bytes.extend_from_slice(binding.semantic_local_identity());
        }
        for value in [
            step.kernel_ir_block,
            step.kernel_ir_operation,
            step.kernel_ir_left.0,
            step.kernel_ir_right.0,
            step.kernel_ir_result.0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}

fn encode_origin_v2(bytes: &mut Vec<u8>, origin: Option<SemanticSourceOriginV1>) {
    let Some(origin) = origin else {
        bytes.push(0);
        return;
    };
    bytes.push(1);
    bytes.extend_from_slice(origin.file().as_bytes());
    let (byte_start, byte_end) = origin.byte_range();
    bytes.extend_from_slice(&byte_start.to_le_bytes());
    bytes.extend_from_slice(&byte_end.to_le_bytes());
    let (line_start, column_start) = origin.start_coordinate();
    let (line_end, column_end) = origin.end_coordinate();
    for coordinate in [line_start, column_start, line_end, column_end] {
        bytes.extend_from_slice(&coordinate.to_le_bytes());
    }
}

fn evidence_identity_v2(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN_V2);
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_lookup_fixture_v2() -> (
        Module,
        Vec<LiveKirFunctionBindingV2>,
        Vec<LiveKirStatementSpanV2>,
        Vec<LiveKirParameterBindingV2>,
        SourceMirKirScalarCompositionStepV2,
    ) {
        use fe2o3_kernel_ir::{BasicBlock, BlockId, Function, Signature, ValueDef};

        let step = inert_fixture_v2().steps[0].clone();
        let function_id = FunctionId::new("composition_helper");
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Subtract,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ));
        let mut module = Module::new("composition_fixture");
        module.functions.push(Function::internal_helper(
            function_id.clone(),
            Signature::new(
                vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
                vec![Type::Scalar(ScalarType::U32)],
            ),
            vec![ValueId(10), ValueId(11)],
            vec![block],
        ));
        (
            module,
            vec![LiveKirFunctionBindingV2 {
                correspondence_owner: 7,
                semantic_function: 2,
                kernel_ir_function: function_id,
            }],
            vec![LiveKirStatementSpanV2 {
                correspondence_owner: 7,
                semantic_function: 2,
                semantic_block: 0,
                semantic_statement: 1,
                kernel_ir_block: 0,
                kernel_ir_operation: 0,
                operation_count: 1,
            }],
            vec![
                LiveKirParameterBindingV2 {
                    correspondence_owner: 7,
                    semantic_function: 2,
                    semantic_local: 1,
                    kernel_ir_value: ValueId(10),
                },
                LiveKirParameterBindingV2 {
                    correspondence_owner: 7,
                    semantic_function: 2,
                    semantic_local: 2,
                    kernel_ir_value: ValueId(11),
                },
            ],
            step,
        )
    }

    fn inert_fixture_v2() -> InertSourceMirKirScalarCompositionEvidenceV2 {
        use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalIdV1, SemanticSourceFileIdentityV1};

        let binding = |tag, raw, local| {
            SourceMirLocalBindingV1::new(
                [tag; 32],
                raw,
                SemanticLocalIdV1::from_index(local),
                [tag.wrapping_add(20); 32],
            )
        };
        let steps = vec![SourceMirKirScalarCompositionStepV2 {
            source_expression_sha256: [1; 32],
            semantic_function: 2,
            semantic_block: 0,
            semantic_statement: 1,
            correspondence_owner: 7,
            operator: MirKirScalarOperatorV1::Subtract,
            left: binding(2, 1, 1),
            right: binding(3, 2, 2),
            destination: binding(4, 0, 0),
            kernel_ir_block: 0,
            kernel_ir_operation: 0,
            kernel_ir_left: ValueId(10),
            kernel_ir_right: ValueId(11),
            kernel_ir_result: ValueId(12),
        }];
        let model_identity = source_mir_kir_scalar_composition_model_identity_v2();
        let source_mir_evidence_identity = [5; 32];
        let mir_kir_evidence_identity = [6; 32];
        let rustc_hir_owner_sha256 = [7; 32];
        let rustc_mir_body_sha256 = [8; 32];
        let semantic_mir_sha256 = [9; 32];
        let canonical_kernel_ir = ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
            crate::ProductionCanonicalKernelIrVersionV1::V8,
            [10; 32],
            128,
        );
        let source_expansion = SemanticSourceOriginV1::new(
            SemanticSourceFileIdentityV1::from_sha256([11; 32]),
            0,
            4,
            1,
            1,
            1,
            5,
        )
        .unwrap();
        let source_call_site = source_expansion;
        let eligible_candidates = 1;
        let canonical_bytes = encode_v2(
            model_identity,
            source_mir_evidence_identity,
            mir_kir_evidence_identity,
            rustc_hir_owner_sha256,
            rustc_mir_body_sha256,
            semantic_mir_sha256,
            canonical_kernel_ir,
            Some(source_expansion),
            Some(source_call_site),
            eligible_candidates,
            &steps,
        )
        .unwrap();
        InertSourceMirKirScalarCompositionEvidenceV2 {
            identity: evidence_identity_v2(&canonical_bytes),
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            model_identity,
            source_mir_evidence_identity,
            mir_kir_evidence_identity,
            rustc_hir_owner_sha256,
            rustc_mir_body_sha256,
            semantic_mir_sha256,
            canonical_kernel_ir,
            source_expansion,
            source_call_site,
            eligible_candidates,
            steps: steps.into_boxed_slice(),
        }
    }

    #[test]
    fn composition_model_binds_exact_proof_and_tool_closure() {
        let proof: [u8; 32] = Sha256::digest(include_bytes!(
            "../verus/source_mir_kir_scalar_composition_v2.rs"
        ))
        .into();
        assert_eq!(proof, SOURCE_MIR_KIR_SCALAR_COMPOSITION_PROOF_SHA256_V2);
        let closure: [u8; 32] =
            Sha256::digest(include_bytes!("../verus/pins/VERUS_CLOSURE_MANIFEST")).into();
        assert_eq!(closure, SOURCE_MIR_KIR_SCALAR_COMPOSITION_CLOSURE_SHA256_V2);
        assert_eq!(
            source_mir_kir_scalar_composition_model_identity_v2(),
            [
                0x95, 0x9b, 0x79, 0x6a, 0x56, 0x73, 0xad, 0xb3, 0x41, 0x96, 0xf6, 0x77, 0x2c, 0x52,
                0x5d, 0xca, 0x9c, 0xcf, 0xc9, 0xab, 0x5c, 0x0b, 0x1e, 0x4c, 0x3d, 0x57, 0x9d, 0x12,
                0xc6, 0xcf, 0x7f, 0x7e,
            ]
        );
        assert_eq!(&inert_fixture_v2().canonical_bytes[..8], b"F2SMKC3\0");
    }

    #[test]
    fn revalidation_rejects_cross_owner_parameter_destination_and_ssa_splices() {
        let exact = inert_fixture_v2();
        exact.revalidate().unwrap();

        let mut cross_owner = exact.clone();
        cross_owner.steps[0].correspondence_owner += 1;
        assert_eq!(
            cross_owner.revalidate(),
            Err(SourceMirKirScalarCompositionErrorV2::NonCanonical)
        );

        let mut parameter = exact.clone();
        parameter.steps[0].left = parameter.steps[0].right;
        assert_eq!(
            parameter.revalidate(),
            Err(SourceMirKirScalarCompositionErrorV2::NonCanonical)
        );

        let mut destination = exact.clone();
        destination.steps[0].destination = destination.steps[0].left;
        assert_eq!(
            destination.revalidate(),
            Err(SourceMirKirScalarCompositionErrorV2::NonCanonical)
        );

        let mut ssa = exact;
        ssa.steps[0].kernel_ir_result = ValueId(99);
        assert_eq!(
            ssa.revalidate(),
            Err(SourceMirKirScalarCompositionErrorV2::NonCanonical)
        );
    }

    #[test]
    fn live_join_validator_rejects_swapped_parameters_result_and_semantic_identity() {
        use fe2o3_kernel_ir::ValueDef;

        let exact = inert_fixture_v2();
        let operation = Operation::effect_free(
            ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Subtract,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        );
        validate_joined_kir_operation_v2(&exact.steps[0], &operation).unwrap();

        let mut swapped_parameters = exact.steps[0].clone();
        (
            swapped_parameters.kernel_ir_left,
            swapped_parameters.kernel_ir_right,
        ) = (
            swapped_parameters.kernel_ir_right,
            swapped_parameters.kernel_ir_left,
        );
        assert_eq!(
            validate_joined_kir_operation_v2(&swapped_parameters, &operation),
            Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch),
        );

        let mut wrong_result = exact.steps[0].clone();
        wrong_result.kernel_ir_result = ValueId(13);
        assert_eq!(
            validate_joined_kir_operation_v2(&wrong_result, &operation),
            Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch),
        );

        assert_eq!(
            validate_semantic_identity_v2(&[1; 32], &[2; 32]),
            Err(SourceMirKirScalarCompositionErrorV2::SemanticIdentityMismatch),
        );
    }

    #[test]
    fn live_step_lookup_rejects_parameter_binding_substitution() {
        let (module, functions, spans, mut parameters, step) = live_lookup_fixture_v2();
        validate_live_kir_step_lookup_v2(&module, &functions, &spans, &parameters, &step).unwrap();

        parameters[0].kernel_ir_value = ValueId(99);
        assert_eq!(
            validate_live_kir_step_lookup_v2(&module, &functions, &spans, &parameters, &step),
            Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch),
        );
    }

    #[test]
    fn live_step_lookup_rejects_statement_span_substitution() {
        let (module, functions, mut spans, parameters, step) = live_lookup_fixture_v2();
        validate_live_kir_step_lookup_v2(&module, &functions, &spans, &parameters, &step).unwrap();

        spans[0].kernel_ir_operation = 1;
        assert_eq!(
            validate_live_kir_step_lookup_v2(&module, &functions, &spans, &parameters, &step),
            Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch),
        );
    }

    #[test]
    fn live_step_lookup_rejects_wrong_live_binary_opcode() {
        let (mut module, functions, spans, parameters, step) = live_lookup_fixture_v2();
        validate_live_kir_step_lookup_v2(&module, &functions, &spans, &parameters, &step).unwrap();

        let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
        operation.kind = OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(10),
            rhs: ValueId(11),
        };
        assert_eq!(
            validate_live_kir_step_lookup_v2(&module, &functions, &spans, &parameters, &step),
            Err(SourceMirKirScalarCompositionErrorV2::LiveStepMismatch),
        );
    }
}
