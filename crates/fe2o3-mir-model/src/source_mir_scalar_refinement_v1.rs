//! Bounded source-expression to semantic-MIR refinement for `u32` binary operations.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticBinaryOpV1, SemanticFunctionIdV1, SemanticLocalIdV1,
    SemanticOperandV1, SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticSourceOriginV1,
    SemanticSourceProvenanceV1, SemanticStatementKindV1, SemanticTypeShapeV1,
};

/// Semantic-model version for the first source-to-MIR theorem.
pub const SOURCE_MIR_SCALAR_REFINEMENT_MODEL_VERSION_V1: u16 = 1;
/// Closed validator policy for the first source-to-MIR theorem.
pub const SOURCE_MIR_SCALAR_REFINEMENT_POLICY_V1: u16 = 1;
/// Stable Verus theorem name for the checked source-to-MIR relation.
pub const SOURCE_MIR_SCALAR_REFINEMENT_THEOREM_V1: &str = "fe2o3_source_mir_u32_element_refines_v1";
/// SHA-256 of the exact positive Verus theorem source.
pub const SOURCE_MIR_SCALAR_REFINEMENT_PROOF_SHA256_HEX_V1: &str =
    "d3eb7a0ee4182ac34d1b9324626243422420b9d43abbfdd63bbbe652ed44223d";
/// SHA-256 of the pinned Verus executable used by the proof lane.
pub const SOURCE_MIR_SCALAR_REFINEMENT_VERUS_SHA256_HEX_V1: &str =
    "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd";
/// Maximum source expressions certified in one admitted semantic module.
pub const MAX_SOURCE_MIR_SCALAR_CERTIFICATES_V1: usize = 16_384;

const MODEL_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-TO-MIR/U32-ELEMENT-REFINEMENT-MODEL/V1\0";
const EVIDENCE_DOMAIN_V1: &[u8] = b"FE2O3/SOURCE-TO-MIR/U32-ELEMENT-REFINEMENT-EVIDENCE/V1\0";

/// Closed binary operator subset shared by source expressions and semantic MIR.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SourceMirScalarOperatorV1 {
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

impl SourceMirScalarOperatorV1 {
    fn from_semantic(operation: SemanticBinaryOpV1) -> Option<Self> {
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

/// Executable state for one accepted source expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceScalarElementStateV1 {
    /// Source left operand value.
    pub left: u32,
    /// Source right operand value.
    pub right: u32,
    /// Abstract source result-binding identity.
    pub destination: u32,
}

/// Executable state for the corresponding semantic-MIR operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirScalarElementStateV1 {
    /// MIR left operand value.
    pub left: u32,
    /// MIR right operand value.
    pub right: u32,
    /// Abstract MIR destination identity.
    pub destination: u32,
}

/// Observable effect of the source/MIR selected-element semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMirScalarEffectV1 {
    /// Read the selected left operand.
    ReadLeft(u32),
    /// Read the selected right operand.
    ReadRight(u32),
    /// Publish a value to an abstract destination.
    Write {
        /// Abstract destination identity.
        destination: u32,
        /// Published `u32` value.
        value: u32,
    },
}

/// Output and ordered effect trace of an executable source/MIR step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMirScalarObservationV1 {
    /// Produced `u32` value.
    pub output: u32,
    /// Ordered abstract read/read/write trace.
    pub effects: [SourceMirScalarEffectV1; 3],
}

/// Executes the accepted source-expression semantics.
pub fn execute_source_scalar_element_v1(
    operator: SourceMirScalarOperatorV1,
    state: SourceScalarElementStateV1,
) -> SourceMirScalarObservationV1 {
    let output = operator.evaluate(state.left, state.right);
    SourceMirScalarObservationV1 {
        output,
        effects: [
            SourceMirScalarEffectV1::ReadLeft(state.left),
            SourceMirScalarEffectV1::ReadRight(state.right),
            SourceMirScalarEffectV1::Write {
                destination: state.destination,
                value: output,
            },
        ],
    }
}

/// Executes the semantic-MIR operation semantics.
pub fn execute_mir_scalar_element_v1(
    operator: SourceMirScalarOperatorV1,
    state: MirScalarElementStateV1,
) -> SourceMirScalarObservationV1 {
    let output = operator.evaluate(state.left, state.right);
    SourceMirScalarObservationV1 {
        output,
        effects: [
            SourceMirScalarEffectV1::ReadLeft(state.left),
            SourceMirScalarEffectV1::ReadRight(state.right),
            SourceMirScalarEffectV1::Write {
                destination: state.destination,
                value: output,
            },
        ],
    }
}

/// Checks the explicit value/destination relation and executable refinement.
pub fn source_mir_scalar_element_refines_v1(
    source_operator: SourceMirScalarOperatorV1,
    mir_operator: SourceMirScalarOperatorV1,
    source: SourceScalarElementStateV1,
    mir: MirScalarElementStateV1,
) -> bool {
    source_operator == mir_operator
        && source.left == mir.left
        && source.right == mir.right
        && source.destination == mir.destination
        && execute_source_scalar_element_v1(source_operator, source)
            == execute_mir_scalar_element_v1(mir_operator, mir)
}

/// Same-session source binding, raw-MIR local, and semantic-local correspondence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceMirLocalBindingV1 {
    source_binding_sha256: [u8; 32],
    rustc_mir_local: u32,
    semantic_local: SemanticLocalIdV1,
    semantic_local_identity: [u8; 32],
}

impl SourceMirLocalBindingV1 {
    /// Constructs one frontend-observed binding axis.
    pub const fn new(
        source_binding_sha256: [u8; 32],
        rustc_mir_local: u32,
        semantic_local: SemanticLocalIdV1,
        semantic_local_identity: [u8; 32],
    ) -> Self {
        Self {
            source_binding_sha256,
            rustc_mir_local,
            semantic_local,
            semantic_local_identity,
        }
    }

    /// Returns the exact same-session HIR binding identity.
    pub const fn source_binding_sha256(&self) -> &[u8; 32] {
        &self.source_binding_sha256
    }
    /// Returns the raw rustc MIR local ordinal.
    pub const fn rustc_mir_local(&self) -> u32 {
        self.rustc_mir_local
    }
    /// Returns the admitted semantic local.
    pub const fn semantic_local(&self) -> SemanticLocalIdV1 {
        self.semantic_local
    }
    /// Returns the canonical semantic-local identity.
    pub const fn semantic_local_identity(&self) -> &[u8; 32] {
        &self.semantic_local_identity
    }
}

/// Rustc-owned HIR/raw-MIR observation submitted to the independent validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcSourceMirScalarObservationV1 {
    /// Stable same-session HIR owner identity.
    pub rustc_hir_owner_sha256: [u8; 32],
    /// Stable same-session HIR expression identity.
    pub source_expression_sha256: [u8; 32],
    /// Exact monomorphized raw-MIR body identity.
    pub rustc_mir_body_sha256: [u8; 32],
    /// Exact source provenance shared by HIR and raw MIR.
    pub source: SemanticSourceProvenanceV1,
    /// Admitted semantic function index.
    pub semantic_function: SemanticFunctionIdV1,
    /// Admitted semantic block index.
    pub semantic_block: u32,
    /// Admitted semantic statement ordinal.
    pub semantic_statement: u32,
    /// HIR and raw-MIR operator classified by the frontend validator.
    pub operator: SourceMirScalarOperatorV1,
    /// Left source/raw-MIR/semantic binding.
    pub left: SourceMirLocalBindingV1,
    /// Right source/raw-MIR/semantic binding.
    pub right: SourceMirLocalBindingV1,
    /// Source-result/raw-MIR/semantic destination binding.
    pub destination: SourceMirLocalBindingV1,
}

/// Canonical certificate for one independently checked source-to-MIR step.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceMirScalarStepCertificateV1 {
    source_expression_sha256: [u8; 32],
    semantic_function: u32,
    semantic_block: u32,
    semantic_statement: u32,
    operator: SourceMirScalarOperatorV1,
    left: SourceMirLocalBindingV1,
    right: SourceMirLocalBindingV1,
    destination: SourceMirLocalBindingV1,
}

impl SourceMirScalarStepCertificateV1 {
    /// Returns the exact HIR expression identity.
    pub const fn source_expression_sha256(&self) -> &[u8; 32] {
        &self.source_expression_sha256
    }
    /// Returns the admitted semantic function index.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }
    /// Returns the admitted semantic block index.
    pub const fn semantic_block(&self) -> u32 {
        self.semantic_block
    }
    /// Returns the admitted semantic statement ordinal.
    pub const fn semantic_statement(&self) -> u32 {
        self.semantic_statement
    }
    /// Returns the checked operator.
    pub const fn operator(&self) -> SourceMirScalarOperatorV1 {
        self.operator
    }
    /// Returns the checked left binding.
    pub const fn left(&self) -> SourceMirLocalBindingV1 {
        self.left
    }
    /// Returns the checked right binding.
    pub const fn right(&self) -> SourceMirLocalBindingV1 {
        self.right
    }
    /// Returns the checked destination binding.
    pub const fn destination(&self) -> SourceMirLocalBindingV1 {
        self.destination
    }
}

/// Authority-free source/HIR/raw-MIR to admitted semantic-MIR evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertSourceMirScalarRefinementEvidenceV1 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    model_identity: [u8; 32],
    rustc_hir_owner_sha256: [u8; 32],
    rustc_mir_body_sha256: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    source: SemanticSourceProvenanceV1,
    certificates: Box<[SourceMirScalarStepCertificateV1]>,
}

impl InertSourceMirScalarRefinementEvidenceV1 {
    /// Independently validates rustc observations against exact admitted semantic MIR.
    pub fn from_rustc_observations(
        semantic: &AdmittedInertSemanticMirV1,
        observations: Vec<RustcSourceMirScalarObservationV1>,
    ) -> Result<Self, SourceMirScalarRefinementErrorV1> {
        if observations.is_empty() || observations.len() > MAX_SOURCE_MIR_SCALAR_CERTIFICATES_V1 {
            return Err(SourceMirScalarRefinementErrorV1::CertificateCount);
        }
        let first = &observations[0];
        if first.rustc_hir_owner_sha256 == [0; 32]
            || first.rustc_mir_body_sha256 == [0; 32]
            || first.source_expression_sha256 == [0; 32]
            || first.source.expansion().is_none()
            || first.source.call_site().is_none()
        {
            return Err(SourceMirScalarRefinementErrorV1::InvalidSourceIdentity);
        }
        let rustc_hir_owner_sha256 = first.rustc_hir_owner_sha256;
        let rustc_mir_body_sha256 = first.rustc_mir_body_sha256;
        let source = first.source;
        let mut certificates = Vec::with_capacity(observations.len());
        for observation in observations {
            if observation.rustc_hir_owner_sha256 != rustc_hir_owner_sha256
                || observation.rustc_mir_body_sha256 != rustc_mir_body_sha256
                || observation.source != source
            {
                return Err(SourceMirScalarRefinementErrorV1::MixedSourceOwner);
            }
            if observation.source_expression_sha256 == [0; 32] {
                return Err(SourceMirScalarRefinementErrorV1::InvalidSourceIdentity);
            }
            certificates.push(validate_observation_v1(semantic, observation)?);
        }
        certificates.sort();
        if certificates.windows(2).any(|window| window[0] == window[1]) {
            return Err(SourceMirScalarRefinementErrorV1::DuplicateCertificate);
        }
        let model_identity = source_mir_scalar_model_identity_v1();
        let semantic_mir_sha256 = *semantic.semantic_sha256().as_bytes();
        let canonical_bytes = encode_v1(
            model_identity,
            rustc_hir_owner_sha256,
            rustc_mir_body_sha256,
            semantic_mir_sha256,
            source,
            &certificates,
        )?;
        let identity = evidence_identity_v1(&canonical_bytes);
        let evidence = Self {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            identity,
            model_identity,
            rustc_hir_owner_sha256,
            rustc_mir_body_sha256,
            semantic_mir_sha256,
            source,
            certificates: certificates.into_boxed_slice(),
        };
        evidence.revalidate()?;
        Ok(evidence)
    }

    /// Revalidates the unique evidence encoding and model identity.
    pub fn revalidate(&self) -> Result<(), SourceMirScalarRefinementErrorV1> {
        if self.model_identity != source_mir_scalar_model_identity_v1()
            || self.rustc_hir_owner_sha256 == [0; 32]
            || self.rustc_mir_body_sha256 == [0; 32]
            || self.semantic_mir_sha256 == [0; 32]
            || self.source.expansion().is_none()
            || self.source.call_site().is_none()
            || self.certificates.is_empty()
            || self.certificates.len() > MAX_SOURCE_MIR_SCALAR_CERTIFICATES_V1
            || self
                .certificates
                .windows(2)
                .any(|window| window[0] >= window[1])
        {
            return Err(SourceMirScalarRefinementErrorV1::NonCanonicalEvidence);
        }
        let bytes = encode_v1(
            self.model_identity,
            self.rustc_hir_owner_sha256,
            self.rustc_mir_body_sha256,
            self.semantic_mir_sha256,
            self.source,
            &self.certificates,
        )?;
        if bytes.as_slice() != self.canonical_bytes.as_ref()
            || evidence_identity_v1(&bytes) != self.identity
        {
            return Err(SourceMirScalarRefinementErrorV1::NonCanonicalEvidence);
        }
        Ok(())
    }

    /// Returns deterministic evidence bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Returns the domain-separated evidence identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    /// Returns the executable/formal semantic-model identity.
    pub const fn model_identity(&self) -> &[u8; 32] {
        &self.model_identity
    }
    /// Returns the exact HIR owner identity.
    pub const fn rustc_hir_owner_sha256(&self) -> &[u8; 32] {
        &self.rustc_hir_owner_sha256
    }
    /// Returns the exact monomorphized raw-MIR identity.
    pub const fn rustc_mir_body_sha256(&self) -> &[u8; 32] {
        &self.rustc_mir_body_sha256
    }
    /// Returns the exact admitted semantic-MIR identity.
    pub const fn semantic_mir_sha256(&self) -> &[u8; 32] {
        &self.semantic_mir_sha256
    }
    /// Returns the exact source provenance shared by HIR, raw MIR, and semantic MIR.
    pub const fn source(&self) -> SemanticSourceProvenanceV1 {
        self.source
    }
    /// Returns every independently validated expression certificate.
    pub fn certificates(&self) -> &[SourceMirScalarStepCertificateV1] {
        &self.certificates
    }
    /// This evidence grants no rustc, artifact, LLVM, runtime, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Returns the domain-separated identity of the executable and formal model.
pub fn source_mir_scalar_model_identity_v1() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(MODEL_DOMAIN_V1);
    hash.update(SOURCE_MIR_SCALAR_REFINEMENT_MODEL_VERSION_V1.to_le_bytes());
    hash.update(SOURCE_MIR_SCALAR_REFINEMENT_POLICY_V1.to_le_bytes());
    hash.update(SOURCE_MIR_SCALAR_REFINEMENT_THEOREM_V1.as_bytes());
    hash.update(SOURCE_MIR_SCALAR_REFINEMENT_PROOF_SHA256_HEX_V1.as_bytes());
    hash.update(SOURCE_MIR_SCALAR_REFINEMENT_VERUS_SHA256_HEX_V1.as_bytes());
    hash.finalize().into()
}

/// Failure while validating or revalidating source-to-MIR evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceMirScalarRefinementErrorV1 {
    /// The observation count is zero or exceeds the fixed bound.
    CertificateCount,
    /// A source, HIR, raw-MIR, or semantic identity is absent.
    InvalidSourceIdentity,
    /// One evidence object mixed distinct HIR owners, MIR bodies, or source spans.
    MixedSourceOwner,
    /// A semantic function, block, or statement locator is invalid.
    InvalidSemanticLocator,
    /// Source and semantic operators differ or are outside the closed subset.
    OperatorMismatch,
    /// A result or operand is not an unprojected `u32` local.
    TypeOrOperandMismatch,
    /// Source, raw-MIR, and semantic local identities do not agree.
    LocalBindingMismatch,
    /// The semantic statement provenance differs from the HIR/raw-MIR source span.
    SourceSpanMismatch,
    /// Two observations produced the same exact certificate.
    DuplicateCertificate,
    /// Evidence bytes or ordering are not canonical.
    NonCanonicalEvidence,
    /// A canonical length or coordinate exceeded its fixed integer representation.
    Overflow,
}

impl fmt::Display for SourceMirScalarRefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CertificateCount => "source-to-MIR certificate count is outside its bound",
            Self::InvalidSourceIdentity => "source-to-MIR observation has an absent identity",
            Self::MixedSourceOwner => "source-to-MIR observations mix source owners",
            Self::InvalidSemanticLocator => "source-to-MIR semantic locator is invalid",
            Self::OperatorMismatch => "source and semantic MIR operators differ",
            Self::TypeOrOperandMismatch => {
                "source-to-MIR operand or type is outside the u32 subset"
            }
            Self::LocalBindingMismatch => "source/raw-MIR/semantic local binding differs",
            Self::SourceSpanMismatch => "source and semantic MIR spans differ",
            Self::DuplicateCertificate => "source-to-MIR certificate is duplicated",
            Self::NonCanonicalEvidence => "source-to-MIR evidence is not canonical",
            Self::Overflow => "source-to-MIR evidence encoding overflowed",
        })
    }
}

impl Error for SourceMirScalarRefinementErrorV1 {}

fn validate_observation_v1(
    semantic: &AdmittedInertSemanticMirV1,
    observation: RustcSourceMirScalarObservationV1,
) -> Result<SourceMirScalarStepCertificateV1, SourceMirScalarRefinementErrorV1> {
    let function = semantic
        .functions()
        .get(observation.semantic_function.index() as usize)
        .ok_or(SourceMirScalarRefinementErrorV1::InvalidSemanticLocator)?;
    let block = function
        .blocks()
        .get(observation.semantic_block as usize)
        .ok_or(SourceMirScalarRefinementErrorV1::InvalidSemanticLocator)?;
    let statement = block
        .statements()
        .get(observation.semantic_statement as usize)
        .ok_or(SourceMirScalarRefinementErrorV1::InvalidSemanticLocator)?;
    if statement.source() != observation.source {
        return Err(SourceMirScalarRefinementErrorV1::SourceSpanMismatch);
    }
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(SourceMirScalarRefinementErrorV1::TypeOrOperandMismatch);
    };
    let SemanticRvalueKindV1::Binary {
        operation,
        left,
        right,
    } = assignment.value().kind()
    else {
        return Err(SourceMirScalarRefinementErrorV1::OperatorMismatch);
    };
    if SourceMirScalarOperatorV1::from_semantic(*operation) != Some(observation.operator) {
        return Err(SourceMirScalarRefinementErrorV1::OperatorMismatch);
    }
    let result_ty = assignment.value().result_type();
    if !is_u32_v1(semantic, result_ty)
        || assignment.destination().ty() != result_ty
        || !assignment.destination().projections().is_empty()
        || assignment.destination().local() != observation.destination.semantic_local
        || !operand_matches_binding_v1(semantic, left, observation.left)
        || !operand_matches_binding_v1(semantic, right, observation.right)
    {
        return Err(SourceMirScalarRefinementErrorV1::TypeOrOperandMismatch);
    }
    for binding in [observation.left, observation.right, observation.destination] {
        let local = function
            .locals()
            .get(binding.semantic_local.index() as usize)
            .ok_or(SourceMirScalarRefinementErrorV1::LocalBindingMismatch)?;
        if local.identity().as_bytes() != &binding.semantic_local_identity
            || !is_u32_v1(semantic, local.ty())
            || binding.source_binding_sha256 == [0; 32]
        {
            return Err(SourceMirScalarRefinementErrorV1::LocalBindingMismatch);
        }
    }
    Ok(SourceMirScalarStepCertificateV1 {
        source_expression_sha256: observation.source_expression_sha256,
        semantic_function: observation.semantic_function.index(),
        semantic_block: observation.semantic_block,
        semantic_statement: observation.semantic_statement,
        operator: observation.operator,
        left: observation.left,
        right: observation.right,
        destination: observation.destination,
    })
}

fn operand_matches_binding_v1(
    semantic: &AdmittedInertSemanticMirV1,
    operand: &SemanticOperandV1,
    binding: SourceMirLocalBindingV1,
) -> bool {
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
        return false;
    };
    place.projections().is_empty()
        && place.local() == binding.semantic_local
        && is_u32_v1(semantic, place.ty())
}

fn is_u32_v1(
    semantic: &AdmittedInertSemanticMirV1,
    ty: crate::semantic_mir_v1::SemanticTypeIdV1,
) -> bool {
    matches!(
        semantic
            .types()
            .get(ty.index() as usize)
            .map(|ty| ty.shape()),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        }))
    )
}

fn encode_v1(
    model_identity: [u8; 32],
    rustc_hir_owner_sha256: [u8; 32],
    rustc_mir_body_sha256: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    source: SemanticSourceProvenanceV1,
    certificates: &[SourceMirScalarStepCertificateV1],
) -> Result<Vec<u8>, SourceMirScalarRefinementErrorV1> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"F2S2M1\0\0");
    bytes.extend_from_slice(&SOURCE_MIR_SCALAR_REFINEMENT_MODEL_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&SOURCE_MIR_SCALAR_REFINEMENT_POLICY_V1.to_le_bytes());
    bytes.extend_from_slice(&model_identity);
    bytes.extend_from_slice(&rustc_hir_owner_sha256);
    bytes.extend_from_slice(&rustc_mir_body_sha256);
    bytes.extend_from_slice(&semantic_mir_sha256);
    encode_source_v1(&mut bytes, source)?;
    bytes.extend_from_slice(
        &u32::try_from(certificates.len())
            .map_err(|_| SourceMirScalarRefinementErrorV1::Overflow)?
            .to_le_bytes(),
    );
    for certificate in certificates {
        bytes.extend_from_slice(&certificate.source_expression_sha256);
        for value in [
            certificate.semantic_function,
            certificate.semantic_block,
            certificate.semantic_statement,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(certificate.operator as u8);
        for binding in [certificate.left, certificate.right, certificate.destination] {
            bytes.extend_from_slice(&binding.source_binding_sha256);
            bytes.extend_from_slice(&binding.rustc_mir_local.to_le_bytes());
            bytes.extend_from_slice(&binding.semantic_local.index().to_le_bytes());
            bytes.extend_from_slice(&binding.semantic_local_identity);
        }
    }
    Ok(bytes)
}

fn encode_source_v1(
    bytes: &mut Vec<u8>,
    source: SemanticSourceProvenanceV1,
) -> Result<(), SourceMirScalarRefinementErrorV1> {
    for origin in [source.expansion(), source.call_site()] {
        let origin = origin.ok_or(SourceMirScalarRefinementErrorV1::InvalidSourceIdentity)?;
        encode_origin_v1(bytes, origin);
    }
    Ok(())
}

fn encode_origin_v1(bytes: &mut Vec<u8>, origin: SemanticSourceOriginV1) {
    bytes.extend_from_slice(origin.file().as_bytes());
    let (start, end) = origin.byte_range();
    bytes.extend_from_slice(&start.to_le_bytes());
    bytes.extend_from_slice(&end.to_le_bytes());
    let (line_start, column_start) = origin.start_coordinate();
    let (line_end, column_end) = origin.end_coordinate();
    for value in [line_start, column_start, line_end, column_end] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn evidence_identity_v1(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN_V1);
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_mir_v1::*;

    const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);

    fn bytes(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn source(tag: u8) -> SemanticSourceProvenanceV1 {
        let origin = SemanticSourceOriginV1::new(
            SemanticSourceFileIdentityV1::from_sha256(bytes(tag)),
            10,
            18,
            2,
            4,
            2,
            12,
        )
        .unwrap();
        SemanticSourceProvenanceV1::new(Some(origin), Some(origin))
    }

    fn direct_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    }

    fn admitted(
        operation: SemanticBinaryOpV1,
        bits: u16,
        statement_source: SemanticSourceProvenanceV1,
    ) -> AdmittedInertSemanticMirV1 {
        let size = u64::from(bits / 8);
        let maximum = if bits == 128 {
            u128::MAX
        } else {
            (1_u128 << bits) - 1
        };
        let ty = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(1)),
            SemanticLayoutIdentityV1::from_sha256(bytes(2)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, bits, size),
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        );
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(3)),
            SemanticLayoutIdentityV1::from_sha256(bytes(4)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct_value(U32), direct_value(U32)],
            direct_value(U32),
        )
        .unwrap();
        let locals = [
            SemanticLocalRoleV1::Return,
            SemanticLocalRoleV1::Argument(0),
            SemanticLocalRoleV1::Argument(1),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(10 + index as u8)),
                U32,
                role,
                statement_source,
            )
        })
        .collect::<Vec<_>>();
        let place = |local| SemanticPlaceV1::new(local, vec![], U32).unwrap();
        let statement = SemanticStatementV1::new(
            statement_source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(SemanticLocalIdV1::from_index(0)),
                SemanticRvalueV1::new(
                    U32,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left: SemanticOperandV1::Copy(place(SemanticLocalIdV1::from_index(1))),
                        right: SemanticOperandV1::Copy(place(SemanticLocalIdV1::from_index(2))),
                    },
                ),
            )),
        );
        let block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(bytes(20)),
            statement_source,
            vec![statement],
            SemanticTerminatorV1::new(statement_source, SemanticTerminatorKindV1::Return),
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(21)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(22)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(23)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(24)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(25)),
            statement_source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![block],
        )
        .unwrap();
        InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(30))),
            vec![ty],
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit(SemanticMirLimitsV1::default())
        .unwrap()
    }

    fn observation(
        semantic: &AdmittedInertSemanticMirV1,
        operator: SourceMirScalarOperatorV1,
        observation_source: SemanticSourceProvenanceV1,
    ) -> RustcSourceMirScalarObservationV1 {
        let function = &semantic.functions()[0];
        let binding = |tag: u8, raw: u32, semantic_local: u32| {
            let local = &function.locals()[semantic_local as usize];
            SourceMirLocalBindingV1::new(
                bytes(tag),
                raw,
                SemanticLocalIdV1::from_index(semantic_local),
                *local.identity().as_bytes(),
            )
        };
        RustcSourceMirScalarObservationV1 {
            rustc_hir_owner_sha256: bytes(40),
            source_expression_sha256: bytes(41),
            rustc_mir_body_sha256: bytes(42),
            source: observation_source,
            semantic_function: SemanticFunctionIdV1::from_index(0),
            semantic_block: 0,
            semantic_statement: 0,
            operator,
            left: binding(43, 1, 1),
            right: binding(44, 2, 2),
            destination: binding(45, 0, 0),
        }
    }

    #[test]
    fn executable_source_and_mir_semantics_refine_and_mutations_fail() {
        let source = SourceScalarElementStateV1 {
            left: u32::MAX,
            right: 1,
            destination: 9,
        };
        let mir = MirScalarElementStateV1 {
            left: source.left,
            right: source.right,
            destination: source.destination,
        };
        assert!(source_mir_scalar_element_refines_v1(
            SourceMirScalarOperatorV1::Add,
            SourceMirScalarOperatorV1::Add,
            source,
            mir,
        ));
        assert!(!source_mir_scalar_element_refines_v1(
            SourceMirScalarOperatorV1::Add,
            SourceMirScalarOperatorV1::Subtract,
            source,
            mir,
        ));
        assert!(!source_mir_scalar_element_refines_v1(
            SourceMirScalarOperatorV1::Add,
            SourceMirScalarOperatorV1::Add,
            source,
            MirScalarElementStateV1 {
                destination: 10,
                ..mir
            },
        ));
    }

    #[test]
    fn evidence_validator_is_non_vacuous_and_rejects_axis_substitutions() {
        let proof_sha: [u8; 32] = Sha256::digest(include_bytes!(
            "../verus/source_mir_scalar_refinement_v1.rs"
        ))
        .into();
        let proof_hex = proof_sha
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(proof_hex, SOURCE_MIR_SCALAR_REFINEMENT_PROOF_SHA256_HEX_V1);
        let span = source(1);
        let semantic = admitted(SemanticBinaryOpV1::BitXor, 32, span);
        let exact = observation(&semantic, SourceMirScalarOperatorV1::BitXor, span);
        let evidence = InertSourceMirScalarRefinementEvidenceV1::from_rustc_observations(
            &semantic,
            vec![exact.clone()],
        )
        .unwrap();
        assert_eq!(evidence.certificates().len(), 1);
        assert!(!evidence.grants_authority());
        evidence.revalidate().unwrap();

        let mut wrong_operator = exact.clone();
        wrong_operator.operator = SourceMirScalarOperatorV1::BitOr;
        assert_eq!(
            InertSourceMirScalarRefinementEvidenceV1::from_rustc_observations(
                &semantic,
                vec![wrong_operator],
            )
            .unwrap_err(),
            SourceMirScalarRefinementErrorV1::OperatorMismatch,
        );

        let mut wrong_binding = exact.clone();
        wrong_binding.left =
            SourceMirLocalBindingV1::new(bytes(43), 1, SemanticLocalIdV1::from_index(1), bytes(99));
        assert_eq!(
            InertSourceMirScalarRefinementEvidenceV1::from_rustc_observations(
                &semantic,
                vec![wrong_binding],
            )
            .unwrap_err(),
            SourceMirScalarRefinementErrorV1::LocalBindingMismatch,
        );

        let wrong_source = observation(&semantic, SourceMirScalarOperatorV1::BitXor, source(2));
        assert_eq!(
            InertSourceMirScalarRefinementEvidenceV1::from_rustc_observations(
                &semantic,
                vec![wrong_source],
            )
            .unwrap_err(),
            SourceMirScalarRefinementErrorV1::SourceSpanMismatch,
        );

        let semantic_u64 = admitted(SemanticBinaryOpV1::BitXor, 64, span);
        let wrong_type = observation(&semantic_u64, SourceMirScalarOperatorV1::BitXor, span);
        assert_eq!(
            InertSourceMirScalarRefinementEvidenceV1::from_rustc_observations(
                &semantic_u64,
                vec![wrong_type],
            )
            .unwrap_err(),
            SourceMirScalarRefinementErrorV1::TypeOrOperandMismatch,
        );
    }
}
