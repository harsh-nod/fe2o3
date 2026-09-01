//! Executable and formally checked semantic refinement for the first scalar MIR-to-KIR slice.

use std::{collections::BTreeMap, error::Error, fmt};

use fe2o3_kernel_ir::{BinaryOp, Constant, Operation, OperationKind, ScalarType, Type, ValueId};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticConstantValueV1, SemanticOperandV1, SemanticRvalueKindV1,
    SemanticScalarTypeV1, SemanticStatementKindV1, SemanticTypeDeclV1, SemanticTypeShapeV1,
};
use sha2::{Digest, Sha256};

use crate::{
    InertCanonicalMirToKirCorrespondenceEvidenceV4, ProductionCanonicalKernelIrIdentityV1,
    ProductionSemanticKirOwnerV1,
};

/// Wire and semantic-model version for the first scalar MIR-to-KIR theorem.
pub const MIR_KIR_SCALAR_REFINEMENT_MODEL_VERSION_V1: u16 = 2;
/// Closed policy version for live-owner certificate derivation.
pub const MIR_KIR_SCALAR_REFINEMENT_POLICY_V1: u16 = 2;
/// Stable name of the Verus theorem checked by the proof lane.
pub const MIR_KIR_SCALAR_REFINEMENT_THEOREM_V1: &str = "fe2o3_mir_kir_u32_element_refines_v1";
/// SHA-256 of the exact Verus theorem source accepted by the proof lane.
pub const MIR_KIR_SCALAR_REFINEMENT_PROOF_SHA256_V1: [u8; 32] = [
    0x73, 0x5c, 0x4c, 0x77, 0xf7, 0x8a, 0x90, 0x38, 0x5d, 0x20, 0x0b, 0xb4, 0x3d, 0xb5, 0xe0, 0x71,
    0xbc, 0x1a, 0x64, 0x1a, 0x5a, 0xe5, 0x61, 0x90, 0xa0, 0x90, 0x23, 0xbc, 0xb2, 0xf2, 0xfe, 0x8e,
];
/// SHA-256 of the pinned Verus executable accepted by the compiler proof lane.
pub const MIR_KIR_SCALAR_REFINEMENT_VERUS_SHA256_V1: [u8; 32] = [
    0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80, 0xa1, 0xda,
    0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0, 0xc9, 0xf3, 0x82, 0xdd,
];
/// SHA-256 of the manifest pinning the complete Verus/vstd/Z3 release closure.
pub const MIR_KIR_SCALAR_REFINEMENT_VERUS_CLOSURE_SHA256_V1: [u8; 32] = [
    0xd2, 0x8d, 0xf3, 0xfb, 0x5e, 0x0d, 0x74, 0x76, 0x37, 0x54, 0x39, 0x33, 0xdf, 0xc3, 0x8c, 0xff,
    0x45, 0x57, 0x6d, 0xa9, 0xb9, 0x20, 0xd7, 0x55, 0xb4, 0xb7, 0xe9, 0x19, 0xe4, 0x7a, 0x60, 0x19,
];

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

/// Exact semantic origin whose value is related to one KIR SSA operand.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirKirScalarSemanticOperandV1 {
    /// A source `u32` constant checked against its exact KIR constant definition.
    Constant(u32),
    /// An unprojected source local mapped by an earlier certificate in the same block.
    Local(u32),
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
    semantic_left: MirKirScalarSemanticOperandV1,
    semantic_right: MirKirScalarSemanticOperandV1,
    semantic_destination: u32,
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
    /// Returns the exact source origin mapped to the KIR left operand.
    pub const fn semantic_left(&self) -> MirKirScalarSemanticOperandV1 {
        self.semantic_left
    }
    /// Returns the exact source origin mapped to the KIR right operand.
    pub const fn semantic_right(&self) -> MirKirScalarSemanticOperandV1 {
        self.semantic_right
    }
    /// Returns the unprojected source local mapped to the KIR result.
    pub const fn semantic_destination(&self) -> u32 {
        self.semantic_destination
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
        let mut scalar_locals = BTreeMap::<(u32, u32, u32), ValueId>::new();
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
            if let SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) = statement.kind()
            {
                scalar_locals.remove(&(
                    span.correspondence_owner().index(),
                    span.semantic_function().index(),
                    local.index(),
                ));
                continue;
            }
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            let destination_local = destination
                .projections()
                .is_empty()
                .then(|| destination.local().index());
            let SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } = assignment.value().kind()
            else {
                if let Some(destination) = destination_local {
                    scalar_locals.remove(&(
                        span.correspondence_owner().index(),
                        span.semantic_function().index(),
                        destination,
                    ));
                }
                continue;
            };
            let Some(mir_operator) = MirKirScalarOperatorV1::from_mir(*operation) else {
                if let Some(destination) = destination_local {
                    scalar_locals.remove(&(
                        span.correspondence_owner().index(),
                        span.semantic_function().index(),
                        destination,
                    ));
                }
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
                if let Some(destination) = destination_local {
                    scalar_locals.remove(&(
                        span.correspondence_owner().index(),
                        span.semantic_function().index(),
                        destination,
                    ));
                }
                continue;
            }
            candidates = candidates
                .checked_add(1)
                .ok_or(MirKirScalarRefinementErrorV1::Overflow)?;
            if function.blocks().len() != 1 {
                return Err(MirKirScalarRefinementErrorV1::UnsupportedInputRelation);
            }
            let destination_local =
                destination_local.ok_or(MirKirScalarRefinementErrorV1::UnsupportedInputRelation)?;
            let semantic_left = semantic_u32_operand_v1(semantic.types(), left)?;
            let semantic_right = semantic_u32_operand_v1(semantic.types(), right)?;
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
            let relation_key = (
                span.correspondence_owner().index(),
                span.semantic_function().index(),
            );
            let checked = check_exact_kir_step_v1(
                mir_operator,
                semantic_left,
                semantic_right,
                relation_key,
                &scalar_locals,
                operations,
            )?;
            certificates.push(MirKirScalarStepCertificateV1 {
                correspondence_owner: span.correspondence_owner().index(),
                semantic_function: span.semantic_function().index(),
                semantic_block: span.semantic_block().index(),
                semantic_statement: span.statement_ordinal(),
                kernel_ir_block: span.kernel_ir_block().0,
                kernel_ir_operation: u32::try_from(start + checked.binary_ordinal)
                    .map_err(|_| MirKirScalarRefinementErrorV1::Overflow)?,
                operator: mir_operator,
                semantic_left,
                semantic_right,
                semantic_destination: destination_local,
                kernel_ir_left: checked.left,
                kernel_ir_right: checked.right,
                kernel_ir_result: checked.result,
            });
            scalar_locals.insert(
                (relation_key.0, relation_key.1, destination_local),
                checked.result,
            );
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
    /// A source operand or destination is outside the closed input-relation fragment.
    UnsupportedInputRelation,
    /// KIR operand, result, or statement-span structure violates the exact input relation.
    InputRelationMismatch,
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
            Self::UnsupportedInputRelation => formatter.write_str(
                "MIR-to-KIR scalar step is outside the straight-line constant-rooted input relation",
            ),
            Self::InputRelationMismatch => formatter.write_str(
                "MIR-to-KIR scalar step does not preserve the exact operand/result relation",
            ),
            Self::NoCoveredSteps => {
                formatter.write_str("owner has no u32 scalar step covered by the refinement model")
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedKirScalarStepV1 {
    binary_ordinal: usize,
    left: ValueId,
    right: ValueId,
    result: ValueId,
}

fn semantic_u32_operand_v1(
    types: &[SemanticTypeDeclV1],
    operand: &SemanticOperandV1,
) -> Result<MirKirScalarSemanticOperandV1, MirKirScalarRefinementErrorV1> {
    let ty = match operand {
        SemanticOperandV1::Copy(place) if place.projections().is_empty() => place.ty(),
        SemanticOperandV1::Constant(constant) => constant.ty(),
        SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_) => {
            return Err(MirKirScalarRefinementErrorV1::UnsupportedInputRelation);
        }
    };
    if !matches!(
        types
            .get(ty.index() as usize)
            .map(|declaration| declaration.shape()),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }))
    ) {
        return Err(MirKirScalarRefinementErrorV1::UnsupportedInputRelation);
    }
    match operand {
        SemanticOperandV1::Copy(place) => {
            Ok(MirKirScalarSemanticOperandV1::Local(place.local().index()))
        }
        SemanticOperandV1::Constant(constant) => {
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Err(MirKirScalarRefinementErrorV1::UnsupportedInputRelation);
            };
            if value.size_bytes() != 4 {
                return Err(MirKirScalarRefinementErrorV1::UnsupportedInputRelation);
            }
            let value = u32::try_from(value.bits())
                .map_err(|_| MirKirScalarRefinementErrorV1::UnsupportedInputRelation)?;
            Ok(MirKirScalarSemanticOperandV1::Constant(value))
        }
        SemanticOperandV1::Move(_) => Err(MirKirScalarRefinementErrorV1::UnsupportedInputRelation),
    }
}

fn check_exact_kir_step_v1(
    operator: MirKirScalarOperatorV1,
    semantic_left: MirKirScalarSemanticOperandV1,
    semantic_right: MirKirScalarSemanticOperandV1,
    relation_key: (u32, u32),
    scalar_locals: &BTreeMap<(u32, u32, u32), ValueId>,
    operations: &[Operation],
) -> Result<CheckedKirScalarStepV1, MirKirScalarRefinementErrorV1> {
    let mut cursor = 0_usize;
    let mut map_operand = |operand| match operand {
        MirKirScalarSemanticOperandV1::Constant(expected) => {
            let operation = operations
                .get(cursor)
                .ok_or(MirKirScalarRefinementErrorV1::InputRelationMismatch)?;
            let [result] = operation.results.as_slice() else {
                return Err(MirKirScalarRefinementErrorV1::InputRelationMismatch);
            };
            if operation.kind != OperationKind::Constant(Constant::U32(expected))
                || result.ty != Type::Scalar(ScalarType::U32)
                || !operation.memory_effects().is_empty()
            {
                return Err(MirKirScalarRefinementErrorV1::InputRelationMismatch);
            }
            cursor = cursor
                .checked_add(1)
                .ok_or(MirKirScalarRefinementErrorV1::Overflow)?;
            Ok(result.id)
        }
        MirKirScalarSemanticOperandV1::Local(local) => scalar_locals
            .get(&(relation_key.0, relation_key.1, local))
            .copied()
            .ok_or(MirKirScalarRefinementErrorV1::UnsupportedInputRelation),
    };
    let left = map_operand(semantic_left)?;
    let right = map_operand(semantic_right)?;
    let binary_ordinal = cursor;
    let operation = operations
        .get(cursor)
        .ok_or(MirKirScalarRefinementErrorV1::MissingMappedOperation)?;
    let OperationKind::Binary { op, lhs, rhs } = operation.kind else {
        return Err(MirKirScalarRefinementErrorV1::MissingMappedOperation);
    };
    let [result] = operation.results.as_slice() else {
        return Err(MirKirScalarRefinementErrorV1::MissingMappedOperation);
    };
    if MirKirScalarOperatorV1::from_kir(op) != Some(operator)
        || lhs != left
        || rhs != right
        || result.ty != Type::Scalar(ScalarType::U32)
        || !operation.memory_effects().is_empty()
        || cursor.checked_add(1) != Some(operations.len())
    {
        return Err(MirKirScalarRefinementErrorV1::InputRelationMismatch);
    }
    Ok(CheckedKirScalarStepV1 {
        binary_ordinal,
        left,
        right,
        result: result.id,
    })
}

fn model_identity_v1() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MODEL_DOMAIN_V1);
    hash.update(MIR_KIR_SCALAR_REFINEMENT_MODEL_VERSION_V1.to_le_bytes());
    hash.update(MIR_KIR_SCALAR_REFINEMENT_POLICY_V1.to_le_bytes());
    hash.update(MIR_KIR_SCALAR_REFINEMENT_THEOREM_V1.as_bytes());
    hash.update(MIR_KIR_SCALAR_REFINEMENT_PROOF_SHA256_V1);
    hash.update(MIR_KIR_SCALAR_REFINEMENT_VERUS_SHA256_V1);
    hash.update(MIR_KIR_SCALAR_REFINEMENT_VERUS_CLOSURE_SHA256_V1);
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
    let mut bytes = Vec::with_capacity(160 + certificates.len() * 56);
    bytes.extend_from_slice(b"F2MKS2\0\0");
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
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(certificate.operator as u8);
        encode_semantic_operand_v1(&mut bytes, certificate.semantic_left);
        encode_semantic_operand_v1(&mut bytes, certificate.semantic_right);
        bytes.extend_from_slice(&certificate.semantic_destination.to_le_bytes());
        for value in [
            certificate.kernel_ir_left.0,
            certificate.kernel_ir_right.0,
            certificate.kernel_ir_result.0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

fn encode_semantic_operand_v1(bytes: &mut Vec<u8>, operand: MirKirScalarSemanticOperandV1) {
    match operand {
        MirKirScalarSemanticOperandV1::Constant(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        MirKirScalarSemanticOperandV1::Local(local) => {
            bytes.push(1);
            bytes.extend_from_slice(&local.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::ValueDef;

    #[test]
    fn model_identity_binds_the_exact_theorem_source() {
        let actual: [u8; 32] =
            Sha256::digest(include_bytes!("../verus/mir_kir_scalar_refinement_v1.rs")).into();
        assert_eq!(actual, MIR_KIR_SCALAR_REFINEMENT_PROOF_SHA256_V1);
        let closure: [u8; 32] =
            Sha256::digest(include_bytes!("../verus/pins/VERUS_CLOSURE_MANIFEST")).into();
        assert_eq!(closure, MIR_KIR_SCALAR_REFINEMENT_VERUS_CLOSURE_SHA256_V1);
        assert_ne!(model_identity_v1(), [0; 32]);
    }

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

    fn u32_constant(id: u32, value: u32) -> Operation {
        Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(value)),
        )
    }

    fn u32_binary(id: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
        Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    }

    #[test]
    fn exact_constant_rooted_local_chain_establishes_operand_and_destination_mapping() {
        let key = (3, 5);
        let mut locals = BTreeMap::new();
        let first = check_exact_kir_step_v1(
            MirKirScalarOperatorV1::Subtract,
            MirKirScalarSemanticOperandV1::Constant(9),
            MirKirScalarSemanticOperandV1::Constant(2),
            key,
            &locals,
            &[
                u32_constant(10, 9),
                u32_constant(11, 2),
                u32_binary(12, BinaryOp::Subtract, 10, 11),
            ],
        )
        .unwrap();
        locals.insert((key.0, key.1, 7), first.result);
        let second = check_exact_kir_step_v1(
            MirKirScalarOperatorV1::Add,
            MirKirScalarSemanticOperandV1::Local(7),
            MirKirScalarSemanticOperandV1::Constant(1),
            key,
            &locals,
            &[u32_constant(13, 1), u32_binary(14, BinaryOp::Add, 12, 13)],
        )
        .unwrap();
        assert_eq!(
            (second.left, second.right, second.result),
            (ValueId(12), ValueId(13), ValueId(14))
        );
    }

    #[test]
    fn hostile_swapped_noncommutative_operands_are_rejected() {
        let error = check_exact_kir_step_v1(
            MirKirScalarOperatorV1::Subtract,
            MirKirScalarSemanticOperandV1::Constant(9),
            MirKirScalarSemanticOperandV1::Constant(2),
            (0, 0),
            &BTreeMap::new(),
            &[
                u32_constant(10, 9),
                u32_constant(11, 2),
                u32_binary(12, BinaryOp::Subtract, 11, 10),
            ],
        )
        .unwrap_err();
        assert_eq!(error, MirKirScalarRefinementErrorV1::InputRelationMismatch);
    }

    #[test]
    fn hostile_destination_result_substitution_is_rejected() {
        let key = (3, 5);
        let locals = BTreeMap::from([((key.0, key.1, 7), ValueId(12))]);
        let error = check_exact_kir_step_v1(
            MirKirScalarOperatorV1::Add,
            MirKirScalarSemanticOperandV1::Local(7),
            MirKirScalarSemanticOperandV1::Constant(1),
            key,
            &locals,
            &[u32_constant(13, 1), u32_binary(14, BinaryOp::Add, 99, 13)],
        )
        .unwrap_err();
        assert_eq!(error, MirKirScalarRefinementErrorV1::InputRelationMismatch);
    }

    #[test]
    fn unsupported_unmapped_source_local_fails_closed() {
        let error = check_exact_kir_step_v1(
            MirKirScalarOperatorV1::Add,
            MirKirScalarSemanticOperandV1::Local(7),
            MirKirScalarSemanticOperandV1::Constant(1),
            (3, 5),
            &BTreeMap::new(),
            &[u32_constant(13, 1), u32_binary(14, BinaryOp::Add, 12, 13)],
        )
        .unwrap_err();
        assert_eq!(
            error,
            MirKirScalarRefinementErrorV1::UnsupportedInputRelation
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
            semantic_left: MirKirScalarSemanticOperandV1::Constant(1),
            semantic_right: MirKirScalarSemanticOperandV1::Constant(2),
            semantic_destination: 3,
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
        evidence.certificates[0].semantic_destination = 4;
        assert_eq!(
            evidence.revalidate(),
            Err(MirKirScalarRefinementErrorV1::NonCanonicalEvidence)
        );
        evidence.certificates[0].semantic_destination = 3;
        evidence.identity[0] ^= 1;
        assert_eq!(
            evidence.revalidate(),
            Err(MirKirScalarRefinementErrorV1::NonCanonicalEvidence)
        );
    }
}
