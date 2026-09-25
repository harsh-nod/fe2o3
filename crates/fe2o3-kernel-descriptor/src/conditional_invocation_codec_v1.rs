use crate::conditional_invocation_rows_v1 as row;
use crate::conditional_invocation_v1::*;
use crate::conditional_invocation_validate_v1::{length, validate};
use crate::decode::Reader;
use crate::nominal_v3::Output as Writer;
use sha2::{Digest, Sha256};

type ResultV1<T, E> = Result<T, ConditionalInvocationWireErrorV1<E>>;
type Parse<T> = for<'a> fn(&mut Reader<'a>) -> FormatResult<T>;

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
    bytes: &'wire [u8],
    identity: ConditionalInvocationIdentityV1,
    subjects: ConditionalSubjectsV1,
    theorem: ConditionalTheoremV1,
    output: ConditionalOutputV1,
    counts: [usize; 4],
    starts: [usize; 4],
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

fn pay<E>(charge: &mut impl FnMut(usize) -> Result<(), E>, n: usize) -> ResultV1<(), E> {
    charge(n).map_err(ConditionalInvocationWireErrorV1::Work)
}
fn prepay<E>(n: usize, charge: &mut impl FnMut(usize) -> Result<(), E>) -> ResultV1<(), E> {
    // Covers bounded pairwise mapping/site checks, repeated record decoding and
    // hashes before any traversal or output mutation; no fresh resource ledger.
    let work = n
        .checked_mul(256)
        .and_then(|n| n.checked_add(4096))
        .ok_or(invalid("work overflow"))?;
    pay(charge, work)
}

impl<'wire> ConditionalInvocationContractV1<'wire> {
    pub const fn canonical_bytes(&self) -> &'wire [u8] {
        self.bytes
    }
    pub const fn identity(&self) -> ConditionalInvocationIdentityV1 {
        self.identity
    }
    pub const fn numerical_domain(&self) -> ConditionalNumericalDomainV1 {
        ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
    }
    pub const fn subjects(&self) -> &ConditionalSubjectsV1 {
        &self.subjects
    }
    pub const fn theorem(&self) -> &ConditionalTheoremV1 {
        &self.theorem
    }
    pub const fn output(&self) -> ConditionalOutputV1 {
        self.output
    }
    pub const fn argument_count(&self) -> usize {
        self.counts[1]
    }
    pub const fn read_count(&self) -> usize {
        self.counts[2]
    }
    pub const fn premise_count(&self) -> usize {
        self.counts[3]
    }
    pub const fn typed_root_count(&self) -> usize {
        self.counts[0]
    }

    pub fn arguments(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalArgumentBindingV1> {
        self.cursor(1, row::ARGUMENT, row::argument)
    }
    pub fn reads(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalReadOccurrenceV1> {
        self.cursor(2, row::READ, row::read)
    }
    pub fn premises(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalRuntimePremiseV1> {
        self.cursor(3, row::PREMISE, row::premise)
    }
    pub fn typed_roots(&self) -> ConditionalInvocationCursorV1<'wire, [u64; 4]> {
        self.cursor(0, 32, row::root)
    }
    pub fn argument<E>(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV1<ConditionalArgumentBindingV1, E> {
        pay(charge, 4 * row::ARGUMENT + 1)?;
        Ok(self.record(1, index, row::ARGUMENT, row::argument)?)
    }
    /// Exact digest equality only; an expected digest supplied by a caller does
    /// not acquire trusted origin or establish receipt/graph membership.
    pub fn require_identity<E>(
        &self,
        expected: ConditionalInvocationIdentityV1,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV1<(), E> {
        pay(charge, 32)?;
        if self.identity != expected {
            return Err(invalid("identity mismatch").into());
        }
        Ok(())
    }
    fn cursor<T>(
        &self,
        group: usize,
        stride: usize,
        parse: Parse<T>,
    ) -> ConditionalInvocationCursorV1<'wire, T> {
        let start = self.starts[group];
        ConditionalInvocationCursorV1 {
            bytes: &self.bytes[start..start + self.counts[group] * stride],
            stride,
            parse,
        }
    }
    pub(crate) fn record<T>(
        &self,
        group: usize,
        i: usize,
        stride: usize,
        parse: Parse<T>,
    ) -> FormatResult<T> {
        if i >= self.counts[group] {
            return Err(invalid("record reference"));
        }
        let start = self.starts[group] + i * stride;
        parse(&mut Reader::new(&self.bytes[start..start + stride]))
    }
}

pub struct ConditionalInvocationCursorV1<'wire, T> {
    bytes: &'wire [u8],
    stride: usize,
    parse: Parse<T>,
}
impl<T> ConditionalInvocationCursorV1<'_, T> {
    pub fn next<E>(
        &mut self,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV1<Option<T>, E> {
        pay(charge, 1)?;
        if self.bytes.is_empty() {
            return Ok(None);
        }
        pay(charge, self.stride * 4)?;
        let result = (self.parse)(&mut Reader::new(&self.bytes[..self.stride]))?;
        self.bytes = &self.bytes[self.stride..];
        Ok(Some(result))
    }
}

fn input_counts(input: &ConditionalInvocationContractInputV1<'_>) -> [usize; 4] {
    [
        input.typed_roots.len(),
        input.arguments.len(),
        input.reads.len(),
        input.premises.len(),
    ]
}
pub fn encoded_conditional_invocation_contract_v1_len<E>(
    input: &ConditionalInvocationContractInputV1<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<usize, E> {
    pay(charge, 1)?;
    let counts = input_counts(input);
    let n = length(counts)?;
    prepay(n, charge)?;
    validate(input, counts, input.subjects, input.theorem, input.output)?;
    Ok(n)
}

/// All fallible checks/charges precede the first write. Failure preserves output.
pub fn encode_conditional_invocation_contract_v1<E>(
    input: &ConditionalInvocationContractInputV1<'_>,
    output: &mut [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<(), E> {
    let n = encoded_conditional_invocation_contract_v1_len(input, charge)?;
    if output.len() != n {
        return Err(ConditionalInvocationWireErrorV1::OutputLength {
            expected: n,
            actual: output.len(),
        });
    }
    pay(charge, n * 2 + 1)?;
    let mut w = Writer::for_slice(output);
    w.bytes(&CONDITIONAL_INVOCATION_MAGIC_V1);
    w.u16(CONDITIONAL_INVOCATION_VERSION_V1);
    w.u16(0);
    w.u32(n as u32);
    w.u16(input.numerical_domain as u16);
    for count in input_counts(input) {
        w.u16(count as u16);
    }
    w.u16(0);
    row::put_subjects(&mut w, input.subjects);
    row::put_theorem(&mut w, input.theorem);
    row::put_output(&mut w, input.output);
    for root in input.typed_roots {
        for n in root {
            w.u64(*n);
        }
    }
    for a in input.arguments {
        row::put_argument(&mut w, *a);
    }
    for r in input.reads {
        row::put_read(&mut w, *r);
    }
    for p in input.premises {
        row::put_premise(&mut w, *p);
    }
    Ok(())
}

pub fn decode_conditional_invocation_contract_v1<'wire, E>(
    bytes: &'wire [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<ConditionalInvocationContractV1<'wire>, E> {
    pay(charge, 1)?;
    if bytes.len() > MAX_CONDITIONAL_INVOCATION_BYTES_V1 {
        return Err(invalid("byte limit").into());
    }
    prepay(bytes.len(), charge)?;
    let view = decode(bytes)?;
    validate(&view, view.counts, view.subjects, view.theorem, view.output)?;
    Ok(view)
}

fn decode(bytes: &[u8]) -> FormatResult<ConditionalInvocationContractV1<'_>> {
    let mut r = Reader::new(bytes);
    if r.fixed::<8>()? != CONDITIONAL_INVOCATION_MAGIC_V1 {
        return Err(invalid("magic"));
    }
    if r.u16()? != CONDITIONAL_INVOCATION_VERSION_V1 {
        return Err(invalid("version"));
    }
    if r.u16()? != 0 {
        return Err(invalid("flags"));
    }
    if r.u32()? as usize != bytes.len() {
        return Err(invalid("declared length"));
    }
    if r.u16()? != ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1 as u16 {
        return Err(invalid("numerical domain/version"));
    }
    let counts = [
        usize::from(r.u16()?),
        usize::from(r.u16()?),
        usize::from(r.u16()?),
        usize::from(r.u16()?),
    ];
    if r.u16()? != 0 {
        return Err(invalid("header reserved"));
    }
    if length(counts)? != bytes.len() {
        return Err(invalid("record length/trailing bytes"));
    }
    let subjects = row::subjects(&mut r)?;
    let theorem = row::theorem(&mut r)?;
    let output = row::output(&mut r)?;
    let mut starts = [row::FIXED; 4];
    for (i, stride) in [32, row::ARGUMENT, row::READ].into_iter().enumerate() {
        starts[i + 1] = starts[i] + counts[i] * stride;
    }
    let mut hash = Sha256::new();
    hash.update(CONDITIONAL_INVOCATION_DOMAIN_V1);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    Ok(ConditionalInvocationContractV1 {
        bytes,
        identity: ConditionalInvocationIdentityV1::from_untrusted_bytes(hash.finalize().into()),
        subjects,
        theorem,
        output,
        counts,
        starts,
    })
}
