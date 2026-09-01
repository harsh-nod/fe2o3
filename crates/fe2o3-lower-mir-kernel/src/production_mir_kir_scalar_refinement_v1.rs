//! Executable and formally checked semantic refinement for the first scalar MIR-to-KIR slice.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{BinaryOp, Operation, OperationKind, ScalarType, Type, ValueId};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticStatementKindV1,
    SemanticTypeShapeV1,
};
use sha2::{Digest, Sha256};

use crate::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, ProductionCanonicalKernelIrIdentityV1,
    ProductionSemanticKirOwnerV1,
};

/// Wire and semantic-model version for the first scalar MIR-to-KIR theorem.
pub const MIR_KIR_SCALAR_REFINEMENT_MODEL_VERSION_V1: u16 = 1;
/// Closed policy version for live-owner certificate derivation.
pub const MIR_KIR_SCALAR_REFINEMENT_POLICY_V1: u16 = 1;
/// Stable name of the Verus theorem checked by the proof lane.
pub const MIR_KIR_SCALAR_REFINEMENT_THEOREM_V1: &str = "fe2o3_mir_kir_u32_element_refines_v1";

const MODEL_DOMAIN_V1: &[u8] = b"FE2O3/MIR-TO-KIR/U32-ELEMENT-REFINEMENT-MODEL/V1\0";
const EVIDENCE_DOMAIN_V1: &[u8] = b"FE2O3/MIR-TO-KIR/U32-ELEMENT-REFINEMENT-EVIDENCE/V1\0";

/// Closed `u32` arithmetic subset covered by the first refinement theorem.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MirKirScalarOperatorV1 {
    /// Wrapping addition.
    Add = 1,
    /// Wrapping subtraction.
    Subtract = 2,
    /// Wrapping multiplication.
    Multiply = 3,
    /// Bitwise conjunction.
    BitAnd = 4,
    /// Bitwise disjunction.
    BitOr = 5,
    /// Bitwise exclusive-or.
    BitXor = 6,
}

impl MirKirScalarOperatorV1 {
    fn from_mir(operation: SemanticBinaryOpV1) -> Option<Self> {
        match operation {
            SemanticBinaryOpV1::Add => Some(Self::Add),
            SemanticBinaryOpV1::Subtract => Some(Self::Subtract),
            SemanticBinaryOpV1::Multiply => Some(Self::Multiply),
            SemanticBinaryOpV1::BitAnd => Some(Self::BitAnd),
            SemanticBinaryOpV1::BitOr => Some(Self::BitOr),
            SemanticBinaryOpV1::BitXor => Some(Self::BitXor),
            _ => None,
        }
    }

    fn from_kir(operation: BinaryOp) -> Option<Self> {
        match operation {
            BinaryOp::Add => Some(Self::Add),
            BinaryOp::Subtract => Some(Self::Subtract),
            BinaryOp::Multiply => Some(Self::Multiply),
            BinaryOp::BitAnd => Some(Self::BitAnd),
            BinaryOp::BitOr => Some(Self::BitOr),
            BinaryOp::BitXor => Some(Self::BitXor),
            _ => None,
        }
    }

    fn evaluate(self, left: u32, right: u32) -> u32 {
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Subtract => left.wrapping_sub(right),
            Self::Multiply => left.wrapping_mul(right),
            Self::BitAnd => left & right,
            Self::BitOr => left | right,
            Self::BitXor => left ^ right,
        }
    }
}

/// Executable source/MIR state for one selected element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirScalarElementStateV1 {
    /// Source left operand.
    pub left: u32,
    /// Source right operand.
    pub right: u32,
    /// Abstract destination element identity.
    pub destination: u32,
}

/// Executable target/KIR state for the corresponding selected element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KirScalarElementStateV1 {
    /// Target left operand.
    pub left: u32,
    /// Target right operand.
    pub right: u32,
    /// Abstract destination element identity.
    pub destination: u32,
}

/// Observable effect of the bounded scalar-element semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirKirScalarEffectV1 {
    /// Read the left value of the selected element.
    ReadLeft(u32),
    /// Read the right value of the selected element.
    ReadRight(u32),
    /// Publish the result to the selected abstract destination.
    Write {
        /// Abstract destination element identity.
        destination: u32,
        /// Value published to that element.
        value: u32,
    },
}

/// Result and ordered effect trace of one executable semantic step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirKirScalarObservationV1 {
    /// Produced `u32` value.
    pub output: u32,
    /// Ordered reads and write performed by the abstract element step.
    pub effects: [MirKirScalarEffectV1; 3],
}

/// Executes the source/MIR semantics used by the checked theorem.
pub fn execute_mir_scalar_element_v1(
    operator: MirKirScalarOperatorV1,
    state: MirScalarElementStateV1,
) -> MirKirScalarObservationV1 {
    let output = operator.evaluate(state.left, state.right);
    MirKirScalarObservationV1 {
        output,
        effects: [
            MirKirScalarEffectV1::ReadLeft(state.left),
            MirKirScalarEffectV1::ReadRight(state.right),
            MirKirScalarEffectV1::Write {
                destination: state.destination,
                value: output,
            },
        ],
    }
}

/// Executes the target/KIR semantics used by the checked theorem.
pub fn execute_kir_scalar_element_v1(
    operator: MirKirScalarOperatorV1,
    state: KirScalarElementStateV1,
) -> MirKirScalarObservationV1 {
    let output = operator.evaluate(state.left, state.right);
    MirKirScalarObservationV1 {
        output,
        effects: [
            MirKirScalarEffectV1::ReadLeft(state.left),
            MirKirScalarEffectV1::ReadRight(state.right),
            MirKirScalarEffectV1::Write {
                destination: state.destination,
                value: output,
            },
        ],
    }
}

/// Checks the explicit input relation and executable output/effect refinement.
pub fn mir_kir_scalar_element_refines_v1(
    mir_operator: MirKirScalarOperatorV1,
    kir_operator: MirKirScalarOperatorV1,
    mir: MirScalarElementStateV1,
    kir: KirScalarElementStateV1,
) -> bool {
    mir_operator == kir_operator
        && mir.left == kir.left
        && mir.right == kir.right
        && mir.destination == kir.destination
        && execute_mir_scalar_element_v1(mir_operator, mir)
            == execute_kir_scalar_element_v1(kir_operator, kir)
}

/// Exact production statement and KIR operation covered by the theorem.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirKirScalarStepCertificateV1 {
    correspondence_owner: u32,
    semantic_function: u32,
    semantic_block: u32,
    semantic_statement: u32,
    kernel_ir_block: u32,
    kernel_ir_operation: u32,
    operator: MirKirScalarOperatorV1,
    kernel_ir_left: ValueId,
    kernel_ir_right: ValueId,
    kernel_ir_result: ValueId,
}

impl MirKirScalarStepCertificateV1 {
    /// Returns the semantic root or helper instance that owns the KIR function.
    pub const fn correspondence_owner(&self) -> u32 {
        self.correspondence_owner
    }
    /// Returns the semantic function index.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }
    /// Returns the semantic block index.
    pub const fn semantic_block(&self) -> u32 {
        self.semantic_block
    }
    /// Returns the semantic statement ordinal.
    pub const fn semantic_statement(&self) -> u32 {
        self.semantic_statement
    }
    /// Returns the exact KIR block index.
    pub const fn kernel_ir_block(&self) -> u32 {
        self.kernel_ir_block
    }
    /// Returns the exact KIR operation ordinal.
    pub const fn kernel_ir_operation(&self) -> u32 {
        self.kernel_ir_operation
    }
    /// Returns the proved operator.
    pub const fn operator(&self) -> MirKirScalarOperatorV1 {
        self.operator
    }
    /// Returns the exact KIR left operand.
    pub const fn kernel_ir_left(&self) -> ValueId {
        self.kernel_ir_left
    }
    /// Returns the exact KIR right operand.
    pub const fn kernel_ir_right(&self) -> ValueId {
        self.kernel_ir_right
    }
    /// Returns the exact KIR result.
    pub const fn kernel_ir_result(&self) -> ValueId {
        self.kernel_ir_result
    }
}

/// Authority-free evidence connecting checked semantics to one live production lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertMirKirScalarRefinementEvidenceV1 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    model_identity: [u8; 32],
    semantic_sha256: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    correspondence_v4_identity: Option<[u8; 32]>,
    candidates: u32,
    certificates: Box<[MirKirScalarStepCertificateV1]>,
}

impl InertMirKirScalarRefinementEvidenceV1 {
    /// Derives semantic certificates from a revalidated production owner.
    pub fn from_live_owner(
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<Self, MirKirScalarRefinementErrorV1> {
        Self::derive(owner, None)
    }

    /// Derives certificates and binds them to existing canonical V4 correspondence evidence.
    pub fn from_live_owner_and_correspondence_v4(
        owner: &ProductionSemanticKirOwnerV1,
        correspondence: &InertCanonicalMirToKirCorrespondenceEvidenceV4,
    ) -> Result<Self, MirKirScalarRefinementErrorV1> {
        if correspondence.semantic_sha256()
            != owner.semantic().semantic().semantic_sha256().as_bytes()
            || correspondence.canonical_kernel_ir_identity() != owner.canonical_kernel_ir_identity()
        {
            return Err(MirKirScalarRefinementErrorV1::CorrespondenceIdentityMismatch);
        }
        Self::derive(owner, Some(*correspondence.identity()))
    }

    fn derive(
        owner: &ProductionSemanticKirOwnerV1,
        correspondence_v4_identity: Option<[u8; 32]>,
    ) -> Result<Self, MirKirScalarRefinementErrorV1> {
        owner
            .verify_equivalence()
            .map_err(|error| MirKirScalarRefinementErrorV1::LiveOwner(error.to_string()))?;
        let semantic = owner.semantic().semantic();
        let mut candidates = 0_u32;
        let mut certificates = Vec::new();
        for span in owner.correspondence().statement_operation_spans() {
            let Some(function) = semantic
                .functions()
                .get(span.semantic_function().index() as usize)
            else {
                return Err(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan);
            };
            let Some(block) = function
                .blocks()
                .get(span.semantic_block().index() as usize)
            else {
                return Err(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan);
            };
            let Some(statement) = block.statements().get(span.statement_ordinal() as usize) else {
                return Err(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan);
            };
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Binary { operation, .. } = assignment.value().kind() else {
                continue;
            };
            let Some(mir_operator) = MirKirScalarOperatorV1::from_mir(*operation) else {
                continue;
            };
            let Some(ty) = semantic
                .types()
                .get(assignment.value().result_type().index() as usize)
            else {
                return Err(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan);
            };
            if !matches!(
                ty.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                })
            ) {
                continue;
            }
            candidates = candidates
                .checked_add(1)
                .ok_or(MirKirScalarRefinementErrorV1::Overflow)?;
            let function_binding = owner
                .correspondence()
                .lowered_functions()
                .iter()
                .find(|binding| {
                    binding.correspondence_owner() == span.correspondence_owner()
                        && binding.semantic_function() == span.semantic_function()
                })
                .ok_or(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan)?;
            let kir_function = owner
                .module()
                .function(function_binding.kernel_ir_function())
                .ok_or(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan)?;
            let kir_block = kir_function
                .body
                .as_ref()
                .and_then(|body| {
                    body.blocks
                        .iter()
                        .find(|block| block.id == span.kernel_ir_block())
                })
                .ok_or(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan)?;
            let start = span.first_operation_ordinal() as usize;
            let end = start
                .checked_add(span.operation_count() as usize)
                .ok_or(MirKirScalarRefinementErrorV1::Overflow)?;
            let operations = kir_block
                .operations
                .get(start..end)
                .ok_or(MirKirScalarRefinementErrorV1::InvalidCorrespondenceSpan)?;
            let (relative, operation) = operations
                .iter()
                .enumerate()
                .rev()
                .find(|(_, operation)| mapped_kir_binary_v1(operation) == Some(mir_operator))
                .ok_or(MirKirScalarRefinementErrorV1::MissingMappedOperation)?;
            let OperationKind::Binary { lhs, rhs, .. } = operation.kind else {
                unreachable!()
            };
            let [result] = operation.results.as_slice() else {
                return Err(MirKirScalarRefinementErrorV1::MissingMappedOperation);
            };
            if result.ty != Type::Scalar(ScalarType::U32) || !operation.memory_effects().is_empty()
            {
                return Err(MirKirScalarRefinementErrorV1::MissingMappedOperation);
            }
            certificates.push(MirKirScalarStepCertificateV1 {
                correspondence_owner: span.correspondence_owner().index(),
                semantic_function: span.semantic_function().index(),
                semantic_block: span.semantic_block().index(),
                semantic_statement: span.statement_ordinal(),
                kernel_ir_block: span.kernel_ir_block().0,
                kernel_ir_operation: u32::try_from(start + relative)
                    .map_err(|_| MirKirScalarRefinementErrorV1::Overflow)?,
                operator: mir_operator,
                kernel_ir_left: lhs,
                kernel_ir_right: rhs,
                kernel_ir_result: result.id,
            });
        }
        if certificates.is_empty() {
            return Err(MirKirScalarRefinementErrorV1::NoCoveredSteps);
        }
        let model_identity = model_identity_v1();
        let semantic_sha256 = *semantic.semantic_sha256().as_bytes();
        let canonical_kernel_ir = owner.canonical_kernel_ir_identity();
        let canonical_bytes = encode_evidence_v1(
            model_identity,
            semantic_sha256,
            canonical_kernel_ir,
            correspondence_v4_identity,
            candidates,
            &certificates,
        );
        let identity = evidence_identity_v1(&canonical_bytes);
        let evidence = Self {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            identity,
            model_identity,
            semantic_sha256,
            canonical_kernel_ir,
            correspondence_v4_identity,
            candidates,
            certificates: certificates.into_boxed_slice(),
        };
        evidence.revalidate()?;
        Ok(evidence)
    }

    /// Revalidates the model identity, canonical encoding, and closed coverage count.
    pub fn revalidate(&self) -> Result<(), MirKirScalarRefinementErrorV1> {
        if self.model_identity != model_identity_v1()
            || self.candidates as usize != self.certificates.len()
            || self.certificates.is_empty()
            || self.semantic_sha256 == [0; 32]
            || self.canonical_kernel_ir.digest() == &[0; 32]
            || self.canonical_kernel_ir.canonical_length() == 0
            || self
                .certificates
                .windows(2)
                .any(|window| window[0] >= window[1])
        {
            return Err(MirKirScalarRefinementErrorV1::NonCanonicalEvidence);
        }
        let encoded = encode_evidence_v1(
            self.model_identity,
            self.semantic_sha256,
            self.canonical_kernel_ir,
            self.correspondence_v4_identity,
            self.candidates,
            &self.certificates,
        );
        if encoded.as_slice() != self.canonical_bytes.as_ref()
            || evidence_identity_v1(&encoded) != self.identity
        {
            return Err(MirKirScalarRefinementErrorV1::NonCanonicalEvidence);
        }
        Ok(())
    }

    /// Returns the deterministic evidence bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Returns the domain-separated evidence identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    /// Returns the executable semantic-model identity.
    pub const fn model_identity(&self) -> &[u8; 32] {
        &self.model_identity
    }
    /// Returns the exact admitted MIR identity.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    /// Returns the exact canonical KIR identity.
    pub const fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.canonical_kernel_ir
    }
    /// Returns the optional nested V4 correspondence identity.
    pub const fn correspondence_v4_identity(&self) -> Option<&[u8; 32]> {
        self.correspondence_v4_identity.as_ref()
    }
    /// Returns the number of supported semantic candidates encountered.
    pub const fn candidate_count(&self) -> u32 {
        self.candidates
    }
    /// Returns every exact certified production step.
    pub fn certificates(&self) -> &[MirKirScalarStepCertificateV1] {
        &self.certificates
    }
    /// Evidence custody grants no artifact, LLVM, runtime, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl InertCanonicalMirToKirCorrespondenceEvidenceV4 {
    /// Extends this structural V4 custody with checked scalar semantic refinement.
    pub fn scalar_semantic_refinement_v1(
        &self,
        owner: &ProductionSemanticKirOwnerV1,
    ) -> Result<InertMirKirScalarRefinementEvidenceV1, MirKirScalarRefinementErrorV1> {
        InertMirKirScalarRefinementEvidenceV1::from_live_owner_and_correspondence_v4(owner, self)
    }
}

/// Failure while deriving production-connected scalar refinement evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirKirScalarRefinementErrorV1 {
    /// The live owner failed its full replay check.
    LiveOwner(String),
    /// Existing V4 evidence names different MIR or KIR bytes.
    CorrespondenceIdentityMismatch,
    /// A retained source-to-target span was invalid.
    InvalidCorrespondenceSpan,
    /// A supported MIR candidate lacked its exact mapped KIR operation.
    MissingMappedOperation,
    /// The owner contained no step in the theorem's bounded subset.
    NoCoveredSteps,
    /// A bounded count or ordinal overflowed.
    Overflow,
    /// Inert evidence was not the unique encoding of its semantic contents.
    NonCanonicalEvidence,
}

impl fmt::Display for MirKirScalarRefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveOwner(error) => write!(formatter, "live MIR-to-KIR owner failed: {error}"),
            Self::CorrespondenceIdentityMismatch => {
                formatter.write_str("V4 correspondence identity differs from the live owner")
            }
            Self::InvalidCorrespondenceSpan => {
                formatter.write_str("MIR-to-KIR correspondence span is invalid")
            }
            Self::MissingMappedOperation => {
                formatter.write_str("supported MIR scalar step lacks its exact KIR operation")
            }
            Self::NoCoveredSteps => {
                formatter.write_str("owner has no u32 scalar step covered by refinement model V1")
            }
            Self::Overflow => {
                formatter.write_str("scalar refinement evidence exceeded a bounded integer")
            }
            Self::NonCanonicalEvidence => {
                formatter.write_str("scalar refinement evidence is not canonical")
            }
        }
    }
}

impl Error for MirKirScalarRefinementErrorV1 {}

fn mapped_kir_binary_v1(operation: &Operation) -> Option<MirKirScalarOperatorV1> {
    let OperationKind::Binary { op, .. } = operation.kind else {
        return None;
    };
    MirKirScalarOperatorV1::from_kir(op)
}

fn model_identity_v1() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MODEL_DOMAIN_V1);
    hash.update(MIR_KIR_SCALAR_REFINEMENT_MODEL_VERSION_V1.to_le_bytes());
    hash.update(MIR_KIR_SCALAR_REFINEMENT_POLICY_V1.to_le_bytes());
    hash.update(MIR_KIR_SCALAR_REFINEMENT_THEOREM_V1.as_bytes());
    hash.finalize().into()
}

fn evidence_identity_v1(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN_V1);
    hash.update(bytes);
    hash.finalize().into()
}

fn encode_evidence_v1(
    model_identity: [u8; 32],
    semantic_sha256: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    correspondence_v4_identity: Option<[u8; 32]>,
    candidates: u32,
    certificates: &[MirKirScalarStepCertificateV1],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(160 + certificates.len() * 40);
    bytes.extend_from_slice(b"F2MKS1\0\0");
    bytes.extend_from_slice(&MIR_KIR_SCALAR_REFINEMENT_MODEL_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&MIR_KIR_SCALAR_REFINEMENT_POLICY_V1.to_le_bytes());
    bytes.extend_from_slice(&model_identity);
    bytes.extend_from_slice(&semantic_sha256);
    bytes.extend_from_slice(canonical_kernel_ir.digest());
    bytes.extend_from_slice(&canonical_kernel_ir.canonical_length().to_le_bytes());
    bytes.push(match canonical_kernel_ir.version() {
        crate::ProductionCanonicalKernelIrVersionV1::V8 => 8,
        crate::ProductionCanonicalKernelIrVersionV1::V9 => 9,
    });
    bytes.extend_from_slice(&correspondence_v4_identity.unwrap_or([0; 32]));
    bytes.extend_from_slice(&candidates.to_le_bytes());
    bytes.extend_from_slice(&(certificates.len() as u32).to_le_bytes());
    for certificate in certificates {
        for value in [
            certificate.correspondence_owner,
            certificate.semantic_function,
            certificate.semantic_block,
            certificate.semantic_statement,
            certificate.kernel_ir_block,
            certificate.kernel_ir_operation,
            certificate.kernel_ir_left.0,
            certificate.kernel_ir_right.0,
            certificate.kernel_ir_result.0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(certificate.operator as u8);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::ValueDef;

    #[test]
    fn executable_mir_and_kir_semantics_refine_at_wrapping_boundaries() {
        for operator in [
            MirKirScalarOperatorV1::Add,
            MirKirScalarOperatorV1::Subtract,
            MirKirScalarOperatorV1::Multiply,
            MirKirScalarOperatorV1::BitAnd,
            MirKirScalarOperatorV1::BitOr,
            MirKirScalarOperatorV1::BitXor,
        ] {
            for (left, right) in [(0, 0), (u32::MAX, 1), (0x8000_0000, 3)] {
                assert!(mir_kir_scalar_element_refines_v1(
                    operator,
                    operator,
                    MirScalarElementStateV1 {
                        left,
                        right,
                        destination: 7
                    },
                    KirScalarElementStateV1 {
                        left,
                        right,
                        destination: 7
                    },
                ));
            }
        }
    }

    #[test]
    fn hostile_operator_operand_and_effect_mutations_do_not_refine() {
        let mir = MirScalarElementStateV1 {
            left: u32::MAX,
            right: 1,
            destination: 4,
        };
        assert!(!mir_kir_scalar_element_refines_v1(
            MirKirScalarOperatorV1::Add,
            MirKirScalarOperatorV1::Subtract,
            mir,
            KirScalarElementStateV1 {
                left: mir.left,
                right: mir.right,
                destination: mir.destination
            },
        ));
        assert!(!mir_kir_scalar_element_refines_v1(
            MirKirScalarOperatorV1::Add,
            MirKirScalarOperatorV1::Add,
            mir,
            KirScalarElementStateV1 {
                left: mir.left,
                right: 2,
                destination: mir.destination
            },
        ));
        let mut mutated = execute_kir_scalar_element_v1(
            MirKirScalarOperatorV1::Add,
            KirScalarElementStateV1 {
                left: mir.left,
                right: mir.right,
                destination: mir.destination,
            },
        );
        mutated.effects[2] = MirKirScalarEffectV1::Write {
            destination: 9,
            value: mutated.output,
        };
        assert_ne!(
            execute_mir_scalar_element_v1(MirKirScalarOperatorV1::Add, mir),
            mutated
        );
    }

    #[test]
    fn production_operation_classifier_rejects_operator_and_type_mutations() {
        let operation = Operation::effect_free(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        );
        assert_eq!(
            mapped_kir_binary_v1(&operation),
            Some(MirKirScalarOperatorV1::Add)
        );
        let wrong = Operation::effect_free(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::Subtract,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        );
        assert_ne!(
            mapped_kir_binary_v1(&wrong),
            Some(MirKirScalarOperatorV1::Add)
        );
    }

    #[test]
    fn inert_evidence_revalidation_rejects_identity_and_certificate_mutation() {
        let model_identity = model_identity_v1();
        let semantic_sha256 = [3; 32];
        let canonical_kernel_ir = ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
            crate::ProductionCanonicalKernelIrVersionV1::V8,
            [4; 32],
            128,
        );
        let certificates = vec![MirKirScalarStepCertificateV1 {
            correspondence_owner: 0,
            semantic_function: 0,
            semantic_block: 0,
            semantic_statement: 0,
            kernel_ir_block: 0,
            kernel_ir_operation: 2,
            operator: MirKirScalarOperatorV1::Add,
            kernel_ir_left: ValueId(0),
            kernel_ir_right: ValueId(1),
            kernel_ir_result: ValueId(2),
        }];
        let canonical_bytes = encode_evidence_v1(
            model_identity,
            semantic_sha256,
            canonical_kernel_ir,
            None,
            1,
            &certificates,
        );
        let mut evidence = InertMirKirScalarRefinementEvidenceV1 {
            identity: evidence_identity_v1(&canonical_bytes),
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            model_identity,
            semantic_sha256,
            canonical_kernel_ir,
            correspondence_v4_identity: None,
            candidates: 1,
            certificates: certificates.into_boxed_slice(),
        };
        evidence.revalidate().unwrap();
        evidence.certificates[0].operator = MirKirScalarOperatorV1::Subtract;
        assert_eq!(
            evidence.revalidate(),
            Err(MirKirScalarRefinementErrorV1::NonCanonicalEvidence)
        );
        evidence.certificates[0].operator = MirKirScalarOperatorV1::Add;
        evidence.identity[0] ^= 1;
        assert_eq!(
            evidence.revalidate(),
            Err(MirKirScalarRefinementErrorV1::NonCanonicalEvidence)
        );
    }
}
