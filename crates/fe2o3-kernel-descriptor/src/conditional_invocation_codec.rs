//! Private common codec. Only the two internal theorem implementations select a schema.
use crate::conditional_invocation_rows_v1 as row;
use crate::conditional_invocation_v1::*;
use crate::conditional_invocation_validate_v1::{Records, length, validate};
use crate::decode::Reader;
use crate::nominal_v3::Output as Writer;
use sha2::{Digest, Sha256};

pub(crate) type ResultV1<T, E> = Result<T, ConditionalInvocationWireErrorV1<E>>;
type Parse<T> = for<'a> fn(&mut Reader<'a>) -> FormatResult<T>;

pub(crate) trait Theorem: Copy {
    const MAGIC: [u8; 8];
    const VERSION: u16;
    const DOMAIN: &'static [u8];
    const FIXED: usize;
    fn read(r: &mut Reader<'_>) -> FormatResult<Self>;
    fn write(self, w: &mut Writer<'_>);
    fn matches(self, subjects: ConditionalSubjectsV1) -> bool;
}

pub(crate) trait Input: Records {
    type Theorem: Theorem;
    fn numerical_domain(&self) -> ConditionalNumericalDomainV1;
    fn subjects(&self) -> ConditionalSubjectsV1;
    fn theorem(&self) -> Self::Theorem;
    fn output(&self) -> ConditionalOutputV1;
    fn typed_roots(&self) -> &[[u64; 4]];
    fn arguments(&self) -> &[ConditionalArgumentBindingV1];
    fn reads(&self) -> &[ConditionalReadOccurrenceV1];
    fn premises(&self) -> &[ConditionalRuntimePremiseV1];
    fn counts(&self) -> [usize; 4] {
        [
            self.typed_roots().len(),
            self.arguments().len(),
            self.reads().len(),
            self.premises().len(),
        ]
    }
}

pub(crate) struct View<'wire, T> {
    pub(crate) bytes: &'wire [u8],
    pub(crate) identity: [u8; 32],
    pub(crate) subjects: ConditionalSubjectsV1,
    pub(crate) theorem: T,
    pub(crate) output: ConditionalOutputV1,
    pub(crate) counts: [usize; 4],
    pub(crate) starts: [usize; 4],
}

pub(crate) fn pay<E>(charge: &mut impl FnMut(usize) -> Result<(), E>, n: usize) -> ResultV1<(), E> {
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

impl<'wire, T> View<'wire, T> {
    pub(crate) fn cursor<R>(
        &self,
        group: usize,
        stride: usize,
        parse: Parse<R>,
    ) -> ConditionalInvocationCursorV1<'wire, R> {
        let start = self.starts[group];
        ConditionalInvocationCursorV1 {
            bytes: &self.bytes[start..start + self.counts[group] * stride],
            stride,
            parse,
        }
    }
    pub(crate) fn record<R>(
        &self,
        group: usize,
        i: usize,
        stride: usize,
        parse: Parse<R>,
    ) -> FormatResult<R> {
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

pub(crate) fn encoded_len<I: Input, E>(
    input: &I,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<usize, E> {
    pay(charge, 1)?;
    let counts = input.counts();
    let n = length(counts, I::Theorem::FIXED)?;
    prepay(n, charge)?;
    validate(
        input,
        counts,
        input.subjects(),
        input.theorem(),
        input.output(),
    )?;
    Ok(n)
}

/// All fallible checks/charges precede the first write. Failure preserves output.
pub(crate) fn encode<I: Input, E>(
    input: &I,
    output: &mut [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<(), E> {
    let n = encoded_len(input, charge)?;
    if output.len() != n {
        return Err(ConditionalInvocationWireErrorV1::OutputLength {
            expected: n,
            actual: output.len(),
        });
    }
    pay(charge, n * 2 + 1)?;
    let mut w = Writer::for_slice(output);
    w.bytes(&I::Theorem::MAGIC);
    w.u16(I::Theorem::VERSION);
    w.u16(0);
    w.u32(n as u32);
    w.u16(input.numerical_domain() as u16);
    for count in input.counts() {
        w.u16(count as u16);
    }
    w.u16(0);
    row::put_subjects(&mut w, input.subjects());
    input.theorem().write(&mut w);
    row::put_output(&mut w, input.output());
    for root in input.typed_roots() {
        for n in root {
            w.u64(*n);
        }
    }
    for a in input.arguments() {
        row::put_argument(&mut w, *a);
    }
    for r in input.reads() {
        row::put_read(&mut w, *r);
    }
    for p in input.premises() {
        row::put_premise(&mut w, *p);
    }
    Ok(())
}

pub(crate) fn decode<'wire, T: Theorem, E>(
    bytes: &'wire [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV1<View<'wire, T>, E> {
    pay(charge, 1)?;
    if bytes.len() > MAX_CONDITIONAL_INVOCATION_BYTES_V1 {
        return Err(invalid("byte limit").into());
    }
    prepay(bytes.len(), charge)?;
    let view = parse::<T>(bytes)?;
    validate(&view, view.counts, view.subjects, view.theorem, view.output)?;
    Ok(view)
}

fn parse<T: Theorem>(bytes: &[u8]) -> FormatResult<View<'_, T>> {
    let mut r = Reader::new(bytes);
    if r.fixed::<8>()? != T::MAGIC {
        return Err(invalid("magic"));
    }
    if r.u16()? != T::VERSION {
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
    if length(counts, T::FIXED)? != bytes.len() {
        return Err(invalid("record length/trailing bytes"));
    }
    let subjects = row::subjects(&mut r)?;
    let theorem = T::read(&mut r)?;
    let output = row::output(&mut r)?;
    let mut starts = [T::FIXED; 4];
    for (i, stride) in [32, row::ARGUMENT, row::READ].into_iter().enumerate() {
        starts[i + 1] = starts[i] + counts[i] * stride;
    }
    let mut hash = Sha256::new();
    hash.update(T::DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    Ok(View {
        bytes,
        identity: hash.finalize().into(),
        subjects,
        theorem,
        output,
        counts,
        starts,
    })
}
