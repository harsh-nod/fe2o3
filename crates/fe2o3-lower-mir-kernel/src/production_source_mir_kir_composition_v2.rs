//! Independently checked composition of source-to-MIR and MIR-to-KIR scalar evidence.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::ValueId;
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
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_MODEL_VERSION_V2: u16 = 2;
/// Closed independently checked production join policy.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_POLICY_V2: u16 = 2;
/// Stable name of the Verus composition theorem.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_THEOREM_V2: &str =
    "fe2o3_source_mir_kir_u32_element_refines_v2";
/// SHA-256 of the exact Verus composition source.
pub const SOURCE_MIR_KIR_SCALAR_COMPOSITION_PROOF_SHA256_V2: [u8; 32] = [
    0xa1, 0x40, 0xe6, 0x10, 0xd6, 0x79, 0x25, 0xcf, 0x32, 0x93, 0x1e, 0xc2, 0x2e, 0x7a, 0x7f, 0x58,
    0xb0, 0xc4, 0x5a, 0xae, 0x33, 0x92, 0xd0, 0x4f, 0x38, 0x69, 0x7b, 0x9b, 0x76, 0x4e, 0xf2, 0x6b,
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

const MODEL_DOMAIN_V2: &[u8] = b"FE2O3/SOURCE-MIR-KIR/U32-COMPOSITION-MODEL/V2\0";
const EVIDENCE_DOMAIN_V2: &[u8] = b"FE2O3/SOURCE-MIR-KIR/U32-COMPOSITION-EVIDENCE/V2\0";

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
        if source.semantic_mir_sha256() != semantic.semantic_sha256().as_bytes() {
            return Err(SourceMirKirScalarCompositionErrorV2::SemanticIdentityMismatch);
        }
        let mir_kir = InertMirKirScalarRefinementEvidenceV1::from_live_owner(owner)
            .map_err(|error| SourceMirKirScalarCompositionErrorV2::MirKir(error.to_string()))?;
        if mir_kir.semantic_sha256() != source.semantic_mir_sha256()
            || mir_kir.canonical_kernel_ir_identity() != owner.canonical_kernel_ir_identity()
        {
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
            steps.push(SourceMirKirScalarCompositionStepV2 {
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
            });
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
            || self.steps.windows(2).any(|window| window[0] >= window[1])
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
    steps: &[SourceMirKirScalarCompositionStepV2],
) -> Result<Vec<u8>, SourceMirKirScalarCompositionErrorV2> {
    let count =
        u32::try_from(steps.len()).map_err(|_| SourceMirKirScalarCompositionErrorV2::Overflow)?;
    let mut bytes = Vec::with_capacity(320 + steps.len() * 260);
    bytes.extend_from_slice(b"F2SMKC2\0");
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
        assert_ne!(
            source_mir_kir_scalar_composition_model_identity_v2(),
            [0; 32]
        );
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
}
