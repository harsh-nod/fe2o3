use crate::conditional_invocation_codec::ConditionalInvocationCursorV1;
use crate::conditional_invocation_codec::{self as codec, Input, ResultV1, View, pay};
use crate::conditional_invocation_rows_v1 as row;
use crate::conditional_invocation_v1::*;
use crate::conditional_invocation_v2::*;
use sha2::Sha256;

/// Complete canonical, borrowed, inert V2 view. No embedded receipt/key is accepted.
/// The full-frame CPU commitment must still be checked against actual retained
/// input by the consuming owner; decoding grants no production authority.
///
/// V2 is not a V1 descriptor view:
///
/// ```compile_fail,E0308
/// use fe2o3_kernel_descriptor::{ConditionalInvocationContractV1, ConditionalInvocationContractV2};
/// fn downgrade(view: ConditionalInvocationContractV2<'_>) -> ConditionalInvocationContractV1<'_> {
///     view
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_descriptor::ConditionalInvocationContractV2;
/// fn forge() -> ConditionalInvocationContractV2<'static> { Default::default() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_descriptor::{ConditionalInvocationContractV2, decode_conditional_invocation_contract_v2};
/// fn escape(bytes: Vec<u8>) -> ConditionalInvocationContractV2<'static> {
///     decode_conditional_invocation_contract_v2(&bytes, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
pub struct ConditionalInvocationContractV2<'wire> {
    inner: View<'wire, ConditionalTheoremV2>,
}

/// Caller prepays these typed extents, plus live input/output backing. Numeric
/// declarations and callbacks are resource plumbing, never reservation authority.
pub const CONDITIONAL_INVOCATION_VIEW_STORAGE_V2: usize =
    size_of::<ConditionalInvocationContractV2<'static>>();
/// Query scratch extent; the unchanged row/cursor grammar has the V1 extent.
pub const CONDITIONAL_INVOCATION_QUERY_STORAGE_V2: usize =
    crate::conditional_invocation_codec_v1::CONDITIONAL_INVOCATION_QUERY_STORAGE_V1;
/// Caller-prepaid codec scratch plus live input/output backing, not authority.
pub const CONDITIONAL_INVOCATION_CODEC_STORAGE_V2: usize = CONDITIONAL_INVOCATION_VIEW_STORAGE_V2
    + CONDITIONAL_INVOCATION_QUERY_STORAGE_V2
    + size_of::<Sha256>()
    + size_of::<ConditionalInvocationContractInputV2<'static>>()
    + 512;

impl<'wire> ConditionalInvocationContractV2<'wire> {
    /// Borrow exact canonical V2 bytes.
    pub const fn canonical_bytes(&self) -> &'wire [u8] {
        self.inner.bytes
    }
    /// Return the domain-separated inert V2 content identity.
    pub const fn identity(&self) -> ConditionalInvocationIdentityV2 {
        ConditionalInvocationIdentityV2::from_untrusted_bytes(self.inner.identity)
    }
    /// Return the supported numerical domain.
    pub const fn numerical_domain(&self) -> ConditionalNumericalDomainV1 {
        ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
    }
    /// Borrow the complete projected subjects.
    pub const fn subjects(&self) -> &ConditionalSubjectsV1 {
        &self.inner.subjects
    }
    /// Borrow the distinct V2 theorem, including the full-frame CPU commitment.
    pub const fn theorem(&self) -> &ConditionalTheoremV2 {
        &self.inner.theorem
    }
    /// Return the sole output occurrence.
    pub const fn output(&self) -> ConditionalOutputV1 {
        self.inner.output
    }
    /// Return the number of argument bindings.
    pub const fn argument_count(&self) -> usize {
        self.inner.counts[1]
    }
    /// Return the number of read occurrences.
    pub const fn read_count(&self) -> usize {
        self.inner.counts[2]
    }
    /// Return the number of ordered runtime premises.
    pub const fn premise_count(&self) -> usize {
        self.inner.counts[3]
    }
    /// Return the number of typed roots.
    pub const fn typed_root_count(&self) -> usize {
        self.inner.counts[0]
    }

    /// Borrow a charged cursor over argument bindings.
    pub fn arguments(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalArgumentBindingV1> {
        self.inner.cursor(1, row::ARGUMENT, row::argument)
    }
    /// Borrow a charged cursor over read occurrences.
    pub fn reads(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalReadOccurrenceV1> {
        self.inner.cursor(2, row::READ, row::read)
    }
    /// Borrow a charged cursor over the complete premise roster.
    pub fn premises(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalRuntimePremiseV1> {
        self.inner.cursor(3, row::PREMISE, row::premise)
    }
    /// Borrow a charged cursor over typed roots.
    pub fn typed_roots(&self) -> ConditionalInvocationCursorV1<'wire, [u64; 4]> {
        self.inner.cursor(0, 32, row::root)
    }
    /// Read one argument after the same V1 query debit.
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
        expected: ConditionalInvocationIdentityV2,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV1<(), E> {
        pay(charge, 32)?;
        if self.inner.identity != *expected.as_bytes() {
            return Err(invalid("identity mismatch").into());
        }
        Ok(())
    }
}

impl Input for ConditionalInvocationContractInputV2<'_> {
    type Theorem = ConditionalTheoremV2;
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

/// Precharge and validate the complete V2 input, returning its exact byte length.
pub fn encoded_conditional_invocation_contract_v2_len<E>(
    input: &ConditionalInvocationContractInputV2<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<usize, E> {
    codec::encoded_len(input, charge)
}

/// All fallible checks/charges precede the first write. Failure preserves output.
pub fn encode_conditional_invocation_contract_v2<E>(
    input: &ConditionalInvocationContractInputV2<'_>,
    output: &mut [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<(), E> {
    codec::encode(input, output, charge)
}

/// Decode only V2 framing, after bounded precharge, without allocation or authority.
pub fn decode_conditional_invocation_contract_v2<'wire, E>(
    bytes: &'wire [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<ConditionalInvocationContractV2<'wire>, E> {
    Ok(ConditionalInvocationContractV2 {
        inner: codec::decode(bytes, charge)?,
    })
}
