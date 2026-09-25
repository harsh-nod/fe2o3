//! Bounded, inert projection of the conditional D1 byte-memory implication.
//!
//! Canonical bytes, hashes and successful decoding do not authenticate a graph,
//! source mapping, typed root, receipt, compiler, or runtime premise. A consuming
//! owner must compare every projected field with its retained authenticated
//! subjects and exact receipt. This type cannot discharge or admit anything.

use std::{error::Error, fmt};

pub const CONDITIONAL_INVOCATION_MAGIC_V1: [u8; 8] = *b"FE2O3CI\0";
pub const CONDITIONAL_INVOCATION_VERSION_V1: u16 = 1;
pub const CONDITIONAL_INVOCATION_DOMAIN_V1: &[u8] = b"FE2O3/CONDITIONAL-INVOCATION/V1\0";
/// Exact existing verifier preimage domain, including the terminal NUL.
pub const CONDITIONAL_MEMORY_THEOREM_DOMAIN_V1: &[u8] =
    b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V1/LE/SHARED-IEEE\0";
pub const MAX_CONDITIONAL_INVOCATION_BYTES_V1: usize = 64 * 1024;
pub const MAX_CONDITIONAL_ARGUMENTS_V1: usize = 64;
pub const MAX_CONDITIONAL_READS_V1: usize = 128;
pub const MAX_CONDITIONAL_ROOTS_V1: usize = 256;
pub const MAX_CONDITIONAL_PREMISES_V1: usize = 4 + 3 * MAX_CONDITIONAL_READS_V1;

/// No machine/ISA numerical-refinement alternative exists in this wire version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ConditionalNumericalDomainV1 {
    LittleEndianSharedIeeeV1 = 1,
}

/// This profile carries the existing MIR-to-MIR reference subjects only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConditionalReferenceKindV1 {
    Mir = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalSubjectsV1 {
    pub kernel_id: [u8; 32],
    pub exact_graph_identity: [u8; 32],
    pub aggregate_statement_identity: [u8; 32],
    pub source_semantic_identity: [u8; 32],
    pub reference_kind: ConditionalReferenceKindV1,
    pub safe_reference_identity: [u8; 32],
    /// Zero for the current MIR-only subject. Not blanket nonzero-validated.
    pub safe_reference_source_hash: [u8; 32],
    pub safe_reference_mir_hash: [u8; 32],
    pub kernel_subject_identity: [u8; 32],
    pub kernel_mir_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalTheoremV1 {
    pub statement_identity: [u8; 32],
    pub generated_source_identity: [u8; 32],
    pub execution_identity: [u8; 32],
    pub receipt_identity: [u8; 32],
    pub staging_receipt_identity: [u8; 32],
    pub staging_obligation_identity: [u8; 32],
    pub staging_signer_identity: [u8; 32],
    pub staging_execution_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ConditionalArgumentRoleV1 {
    Input = 0,
    Output = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalArgumentBindingV1 {
    pub canonical_parameter: u32,
    pub source_argument: u32,
    pub adjusted_argument: u32,
    pub semantic_local: u32,
    pub semantic_type: u32,
    /// Generated logical field ordinal, not a physical ABI component ordinal.
    pub generated_field: u16,
    pub role: ConditionalArgumentRoleV1,
    pub source_type_identity: [u8; 32],
    pub device_layout_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalCanonicalLocationV1 {
    pub block: u32,
    pub operation: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalRankedLocationV1 {
    pub block: u32,
    pub operation: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalRankedValueV1 {
    Argument(u32),
    BlockArgument { block: u32, argument: u32 },
    Local(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ConditionalAddressDomainV1 {
    GuardedOutput = 0,
    GlobalLaunch = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalOutputV1 {
    pub argument: u16,
    pub canonical_store: ConditionalCanonicalLocationV1,
    pub ranked_store: ConditionalRankedLocationV1,
    pub ranked_effect: ConditionalRankedLocationV1,
    pub element_bytes: u64,
    pub alignment: u32,
    pub address_domain: ConditionalAddressDomainV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalReadOccurrenceV1 {
    pub argument: u16,
    pub canonical: ConditionalCanonicalLocationV1,
    pub slice: u32,
    pub pointer: u32,
    pub index: u32,
    pub value: u32,
    pub ranked: ConditionalRankedLocationV1,
    pub ranked_view: ConditionalRankedValueV1,
    pub ranked_index: ConditionalRankedValueV1,
    pub access_domain: ConditionalAddressDomainV1,
    pub address_domain: ConditionalAddressDomainV1,
    pub element_bytes: u64,
    pub alignment: u32,
}

/// Matches ProductionConditionalRuntimePremiseV1, including full logical-span
/// separation and address formation independent of readable allocation extents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalRuntimePremiseV1 {
    D1Launch,
    OutputWithinGlobalX {
        parameter: u32,
    },
    WritableOutput {
        parameter: u32,
    },
    ReadableInput {
        parameter: u32,
        domain: ConditionalAddressDomainV1,
    },
    SeparateInputOutput {
        input: u32,
        output: u32,
    },
    RepresentableAddress {
        parameter: u32,
        domain: ConditionalAddressDomainV1,
        element_bytes: u64,
        alignment: u32,
    },
}

/// Caller data. Even successful encoding does not authenticate its provenance.
pub struct ConditionalInvocationContractInputV1<'a> {
    pub numerical_domain: ConditionalNumericalDomainV1,
    pub subjects: ConditionalSubjectsV1,
    pub theorem: ConditionalTheoremV1,
    pub typed_roots: &'a [[u64; 4]],
    /// Strict canonical-parameter order. Other ordinal spaces are independent.
    pub arguments: &'a [ConditionalArgumentBindingV1],
    pub output: ConditionalOutputV1,
    /// Preserve producer occurrence order; never deduplicate by parameter.
    pub reads: &'a [ConditionalReadOccurrenceV1],
    /// Exact producer order; no synthesized or omitted host obligations.
    pub premises: &'a [ConditionalRuntimePremiseV1],
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ConditionalInvocationIdentityV1([u8; 32]);
impl ConditionalInvocationIdentityV1 {
    pub const fn from_untrusted_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug)]
pub enum ConditionalInvocationContractErrorV1 {
    Decode(crate::DecodeError),
    Invalid(&'static str),
}
impl From<crate::DecodeError> for ConditionalInvocationContractErrorV1 {
    fn from(value: crate::DecodeError) -> Self {
        Self::Decode(value)
    }
}
impl fmt::Display for ConditionalInvocationContractErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "conditional contract wire: {e}"),
            Self::Invalid(field) => write!(f, "invalid conditional contract {field}"),
        }
    }
}
impl Error for ConditionalInvocationContractErrorV1 {}

#[derive(Debug)]
pub enum ConditionalInvocationWireErrorV1<E> {
    Contract(ConditionalInvocationContractErrorV1),
    Work(E),
    OutputLength { expected: usize, actual: usize },
}
impl<E> From<ConditionalInvocationContractErrorV1> for ConditionalInvocationWireErrorV1<E> {
    fn from(e: ConditionalInvocationContractErrorV1) -> Self {
        Self::Contract(e)
    }
}
impl<E: fmt::Display> fmt::Display for ConditionalInvocationWireErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(e) => e.fmt(f),
            Self::Work(e) => write!(f, "conditional contract work refused: {e}"),
            Self::OutputLength { expected, actual } => {
                write!(f, "conditional output length {actual}, expected {expected}")
            }
        }
    }
}
impl<E: Error + 'static> Error for ConditionalInvocationWireErrorV1<E> {}

pub(crate) type FormatResult<T> = Result<T, ConditionalInvocationContractErrorV1>;
pub(crate) fn invalid(field: &'static str) -> ConditionalInvocationContractErrorV1 {
    ConditionalInvocationContractErrorV1::Invalid(field)
}
