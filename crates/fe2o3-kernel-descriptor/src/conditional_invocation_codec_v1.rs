pub use crate::conditional_invocation_codec::ConditionalInvocationCursorV1;
use crate::conditional_invocation_codec::{self as codec, Input, ResultV1, View, pay};
use crate::conditional_invocation_rows_v1 as row;
use crate::conditional_invocation_v1::*;
use crate::decode::Reader;
use sha2::Sha256;

/// Complete canonical, borrowed, inert view. No embedded receipt/key is accepted.
///
/// ```compile_fail
/// use fe2o3_kernel_descriptor::ConditionalInvocationContractV1;
/// fn forge() -> ConditionalInvocationContractV1<'static> { Default::default() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_descriptor::{ConditionalInvocationContractV1, decode_conditional_invocation_contract_v1};
/// fn escape(bytes: Vec<u8>) -> ConditionalInvocationContractV1<'static> {
///     decode_conditional_invocation_contract_v1(&bytes, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
pub struct ConditionalInvocationContractV1<'wire> {
    inner: View<'wire, ConditionalTheoremV1>,
}

/// Caller prepays these typed extents, plus live input/output backing. Numeric
/// declarations and callbacks are resource plumbing, never reservation authority.
pub const CONDITIONAL_INVOCATION_VIEW_STORAGE_V1: usize =
    size_of::<ConditionalInvocationContractV1<'static>>();
pub const CONDITIONAL_INVOCATION_QUERY_STORAGE_V1: usize = size_of::<Reader<'static>>()
    + size_of::<ConditionalReadOccurrenceV1>() * 2
    + size_of::<ConditionalArgumentBindingV1>() * 2
    + size_of::<ConditionalRuntimePremiseV1>() * 4
    + size_of::<ConditionalInvocationCursorV1<'static, ConditionalReadOccurrenceV1>>()
    + size_of::<[u8; 80]>();
pub const CONDITIONAL_INVOCATION_CODEC_STORAGE_V1: usize = CONDITIONAL_INVOCATION_VIEW_STORAGE_V1
    + CONDITIONAL_INVOCATION_QUERY_STORAGE_V1
    + size_of::<Sha256>()
    + size_of::<ConditionalInvocationContractInputV1<'static>>()
    + 512;

impl<'wire> ConditionalInvocationContractV1<'wire> {
    pub const fn canonical_bytes(&self) -> &'wire [u8] {
        self.inner.bytes
    }
    pub const fn identity(&self) -> ConditionalInvocationIdentityV1 {
        ConditionalInvocationIdentityV1::from_untrusted_bytes(self.inner.identity)
    }
    pub const fn numerical_domain(&self) -> ConditionalNumericalDomainV1 {
        ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
    }
    pub const fn subjects(&self) -> &ConditionalSubjectsV1 {
        &self.inner.subjects
    }
    pub const fn theorem(&self) -> &ConditionalTheoremV1 {
        &self.inner.theorem
    }
    pub const fn output(&self) -> ConditionalOutputV1 {
        self.inner.output
    }
    pub const fn argument_count(&self) -> usize {
        self.inner.counts[1]
    }
    pub const fn read_count(&self) -> usize {
        self.inner.counts[2]
    }
    pub const fn premise_count(&self) -> usize {
        self.inner.counts[3]
    }
    pub const fn typed_root_count(&self) -> usize {
        self.inner.counts[0]
    }

    pub fn arguments(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalArgumentBindingV1> {
        self.inner.cursor(1, row::ARGUMENT, row::argument)
    }
    pub fn reads(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalReadOccurrenceV1> {
        self.inner.cursor(2, row::READ, row::read)
    }
    pub fn premises(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalRuntimePremiseV1> {
        self.inner.cursor(3, row::PREMISE, row::premise)
    }
    pub fn typed_roots(&self) -> ConditionalInvocationCursorV1<'wire, [u64; 4]> {
        self.inner.cursor(0, 32, row::root)
    }
    pub fn argument<E>(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV1<ConditionalArgumentBindingV1, E> {
        pay(charge, 4 * row::ARGUMENT + 1)?;
        Ok(self.inner.record(1, index, row::ARGUMENT, row::argument)?)
    }
    /// Exact digest equality only; an expected digest supplied by a caller does
    /// not acquire trusted origin or establish receipt/graph membership.
    pub fn require_identity<E>(
        &self,
        expected: ConditionalInvocationIdentityV1,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV1<(), E> {
        pay(charge, 32)?;
        if self.inner.identity != *expected.as_bytes() {
            return Err(invalid("identity mismatch").into());
        }
        Ok(())
    }
}

impl Input for ConditionalInvocationContractInputV1<'_> {
    type Theorem = ConditionalTheoremV1;
    fn numerical_domain(&self) -> ConditionalNumericalDomainV1 {
        self.numerical_domain
    }
    fn subjects(&self) -> ConditionalSubjectsV1 {
        self.subjects
    }
    fn theorem(&self) -> Self::Theorem {
        self.theorem
    }
    fn output(&self) -> ConditionalOutputV1 {
        self.output
    }
    fn typed_roots(&self) -> &[[u64; 4]] {
        self.typed_roots
    }
    fn arguments(&self) -> &[ConditionalArgumentBindingV1] {
        self.arguments
    }
    fn reads(&self) -> &[ConditionalReadOccurrenceV1] {
        self.reads
    }
    fn premises(&self) -> &[ConditionalRuntimePremiseV1] {
        self.premises
    }
}

pub fn encoded_conditional_invocation_contract_v1_len<E>(
    input: &ConditionalInvocationContractInputV1<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<usize, E> {
    codec::encoded_len(input, charge)
}

/// All fallible checks/charges precede the first write. Failure preserves output.
pub fn encode_conditional_invocation_contract_v1<E>(
    input: &ConditionalInvocationContractInputV1<'_>,
    output: &mut [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<(), E> {
    codec::encode(input, output, charge)
}

pub fn decode_conditional_invocation_contract_v1<'wire, E>(
    bytes: &'wire [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<ConditionalInvocationContractV1<'wire>, E> {
    Ok(ConditionalInvocationContractV1 {
        inner: codec::decode(bytes, charge)?,
    })
}
