//! Versioned mixed-access data with an honest explicit-predicate alternative.
//!
//! A decoded row grants no source, native, artifact, or launch authority.
//! Formation and access bounds are independent mandatory claims. V26's wire
//! format and CFG-only consumers remain separate and unchanged.

use super::mixed_conditional_v26::{
    MixedArgumentV26, MixedContractSubjectsV26, MixedDefinitionV26, MixedEdgeV26,
    MixedGuardPathV26, MixedIndexEnvelopeV26, MixedMemorySpaceV26, MixedOperationV26, Wire,
    wire_struct,
};
use crate::decode::Reader;
use crate::nominal_v3::Output;
use sha2::{Digest, Sha256};

pub const MAX_MIXED_ARGUMENTS_V86: usize = 64;
pub const MAX_MIXED_OCCURRENCES_V86: usize = 256;
pub const MAX_MIXED_CONTRACT_BYTES_V86: usize = 128 * 1024;
pub const MIXED_CONTRACT_MAGIC_V86: [u8; 8] = *b"FE2O3M86";
pub const MIXED_CONTRACT_DOMAIN_V86: &[u8] = b"FE2O3/CONDITIONAL-MIXED-CONTRACT/V86\0";
const PREFIX: usize = 20;

#[derive(Debug, Eq, PartialEq)]
pub enum MixedContractErrorV86<E> {
    Resource(E),
    Invalid(&'static str),
}
impl<E: std::fmt::Display> std::fmt::Display for MixedContractErrorV86<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Invalid(e) => f.write_str(e),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for MixedContractErrorV86<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}
type Format<T> = Result<T, &'static str>;
type ResultV86<T, E> = Result<T, MixedContractErrorV86<E>>;

include!("mixed_conditional_guard_v86.rs");

wire_struct! {
    /// Every occurrence is retained in producer order, including repeated arguments.
    MixedOccurrenceV86 {
        argument: u16,
        original_instance: u32,
        original_operation: MixedOperationV26,
        output_operation: MixedOperationV26,
        original_formation: MixedOperationV26,
        output_formation: MixedOperationV26,
        output_address_index: MixedDefinitionV26,
        output_guard: MixedAccessGuardV86,
        slice_value: u32,
        pointer_value: u32,
        index_value: u32,
        guard_index_value: u32,
        length_value: u32,
        predicate_value: u32,
        path: MixedGuardPathV26,
        element_bytes: u64,
        alignment: u32,
        address_space: MixedMemorySpaceV26,
        writing: bool,
        volatile: bool,
        /// 0..=2 is an actual direct Global invocation projection; 255 is absent.
        invocation_axis: u8,
        /// The exact body-local ValueId; zero padding when projection is absent.
        invocation_value: u32,
        access_envelope: MixedIndexEnvelopeV26,
        formation_envelope: MixedIndexEnvelopeV26
    }
}

pub struct MixedContractInputV86<'a> {
    pub subjects: MixedContractSubjectsV26,
    /// Strict original argument order, independently of physical parameter ordinals.
    pub arguments: &'a [MixedArgumentV26],
    pub occurrences: &'a [MixedOccurrenceV86],
}

/// A borrowed canonical data view, not a compiler or verification certificate.
pub struct MixedContractV86<'a> {
    bytes: &'a [u8],
    identity: [u8; 32],
    subjects: MixedContractSubjectsV26,
    arguments: usize,
    occurrences: usize,
}
pub const MIXED_CONTRACT_CODEC_STORAGE_V86: usize = size_of::<MixedContractV86<'static>>()
    + size_of::<MixedContractInputV86<'static>>()
    + size_of::<MixedContractSubjectsV26>() * 2
    + size_of::<MixedArgumentV26>() * 2
    + size_of::<MixedOccurrenceV86>() * 2
    + size_of::<[[u32; 2]; MAX_MIXED_ARGUMENTS_V86]>()
    + size_of::<Reader<'static>>() * 2
    + size_of::<Output<'static>>()
    + size_of::<Sha256>()
    + 1024;

trait Rows {
    fn subjects(&self) -> &MixedContractSubjectsV26;
    fn argument_count(&self) -> usize;
    fn occurrence_count(&self) -> usize;
    fn argument(&self, i: usize) -> Format<MixedArgumentV26>;
    fn occurrence(&self, i: usize) -> Format<MixedOccurrenceV86>;
}
impl Rows for MixedContractInputV86<'_> {
    fn subjects(&self) -> &MixedContractSubjectsV26 {
        &self.subjects
    }
    fn argument_count(&self) -> usize {
        self.arguments.len()
    }
    fn occurrence_count(&self) -> usize {
        self.occurrences.len()
    }
    fn argument(&self, i: usize) -> Format<MixedArgumentV26> {
        self.arguments
            .get(i)
            .copied()
            .ok_or("mixed argument ordinal")
    }
    fn occurrence(&self, i: usize) -> Format<MixedOccurrenceV86> {
        self.occurrences
            .get(i)
            .copied()
            .ok_or("mixed occurrence ordinal")
    }
}
impl Rows for MixedContractV86<'_> {
    fn subjects(&self) -> &MixedContractSubjectsV26 {
        &self.subjects
    }
    fn argument_count(&self) -> usize {
        self.arguments
    }
    fn occurrence_count(&self) -> usize {
        self.occurrences
    }
    fn argument(&self, i: usize) -> Format<MixedArgumentV26> {
        if i >= self.arguments {
            return Err("mixed argument ordinal");
        }
        read_at(
            self.bytes,
            PREFIX + MixedContractSubjectsV26::BYTES + i * MixedArgumentV26::BYTES,
        )
    }
    fn occurrence(&self, i: usize) -> Format<MixedOccurrenceV86> {
        if i >= self.occurrences {
            return Err("mixed occurrence ordinal");
        }
        read_at(
            self.bytes,
            PREFIX
                + MixedContractSubjectsV26::BYTES
                + self.arguments * MixedArgumentV26::BYTES
                + i * MixedOccurrenceV86::BYTES,
        )
    }
}
fn read_at<T: Wire>(bytes: &[u8], at: usize) -> Format<T> {
    let row = bytes.get(at..at + T::BYTES).ok_or("mixed row bounds")?;
    T::read(&mut Reader::new(row))
}
fn frame_len(arguments: usize, occurrences: usize) -> Format<usize> {
    if arguments > MAX_MIXED_ARGUMENTS_V86 || occurrences > MAX_MIXED_OCCURRENCES_V86 {
        return Err("mixed roster limit");
    }
    let bytes = PREFIX
        + MixedContractSubjectsV26::BYTES
        + arguments * MixedArgumentV26::BYTES
        + occurrences * MixedOccurrenceV86::BYTES;
    if bytes > MAX_MIXED_CONTRACT_BYTES_V86 {
        return Err("mixed frame limit");
    }
    Ok(bytes)
}
fn charge<E>(bytes: usize, pay: &mut impl FnMut(usize) -> Result<(), E>) -> ResultV86<(), E> {
    // Bounded rows: two full structural visits, scalar tags, row copies, length
    // checks, canonical output and SHA input. No hidden sort or pairwise scan.
    pay(4096 + 32 * bytes).map_err(MixedContractErrorV86::Resource)
}
fn validate(rows: &impl Rows) -> Format<()> {
    let s = rows.subjects();
    if !(1..=3).contains(&s.source_rank)
        || !matches!(s.index_width, 32 | 64)
        || !s.kernarg_alignment.is_power_of_two()
        || s.explicit_argument_bytes > 64 * 1024
        || s.exact_grid.contains(&0)
        || s.exact_grid[usize::from(s.source_rank)..]
            .iter()
            .any(|n| *n != 1)
        || (rows.argument_count() == 0 && rows.occurrence_count() != 0)
    {
        return Err("mixed launch or ABI header");
    }
    let mut previous = None;
    let mut counts = [[0u32; 2]; MAX_MIXED_ARGUMENTS_V86];
    for i in 0..rows.argument_count() {
        let a = rows.argument(i)?;
        if previous.is_some_and(|old| old >= a.source_argument)
            || a.source_argument >= s.source_argument_count
            || u32::from(a.generated_field) >= s.generated_field_count
            || a.reads
                .checked_add(a.writes)
                .filter(|n| *n <= MAX_MIXED_OCCURRENCES_V86 as u32)
                .is_none()
            || (a.writes != 0 && !a.source_exclusive)
            || !a.pointer_offset.is_multiple_of(8)
            || !a.length_offset.is_multiple_of(8)
            || a.pointer_offset == a.length_offset
            || a.pointer_offset
                .checked_add(8)
                .is_none_or(|n| n > s.explicit_argument_bytes)
            || a.length_offset
                .checked_add(8)
                .is_none_or(|n| n > s.explicit_argument_bytes)
        {
            return Err("mixed source argument or ABI row");
        }
        previous = Some(a.source_argument);
    }
    for i in 0..rows.occurrence_count() {
        let o = rows.occurrence(i)?;
        let a = rows.argument(usize::from(o.argument))?;
        if o.output_operation.function != s.output_function
            || o.output_formation.function != s.output_function
            || o.output_address_index.function() != s.output_function
            || o.original_formation.function != o.original_operation.function
            || o.element_bytes != a.scalar.element_bytes()
            || !o.alignment.is_power_of_two()
            || !o.element_bytes.is_multiple_of(u64::from(o.alignment))
            || o.volatile
            || (o.invocation_axis != 255 && o.invocation_axis >= s.source_rank)
            || (o.invocation_axis == 255 && (o.invocation_value != 0 || o.writing))
        {
            return Err("mixed occurrence shape or projection");
        }
        o.output_guard.validate(s.output_function, o.path)?;
        for envelope in [o.access_envelope, o.formation_envelope] {
            match envelope {
                MixedIndexEnvelopeV26::LogicalExtent { argument }
                    if usize::from(argument) < rows.argument_count() =>
                {
                    ()
                }
                MixedIndexEnvelopeV26::InvocationAxis { axis } if axis < s.source_rank => (),
                MixedIndexEnvelopeV26::UnsignedWidth { bits }
                    if (1..=s.index_width).contains(&bits) =>
                {
                    ()
                }
                _ => return Err("mixed envelope range"),
            }
        }
        counts[usize::from(o.argument)][usize::from(o.writing)] += 1;
    }
    for (i, count) in counts.iter().take(rows.argument_count()).enumerate() {
        let a = rows.argument(i)?;
        if *count != [a.reads, a.writes] {
            return Err("mixed complete occurrence census");
        }
    }
    Ok(())
}

pub fn encoded_mixed_contract_v86_len<E>(
    input: &MixedContractInputV86<'_>,
    pay: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV86<usize, E> {
    let len = frame_len(input.arguments.len(), input.occurrences.len())
        .map_err(MixedContractErrorV86::Invalid)?;
    charge(len, pay)?;
    validate(input).map_err(MixedContractErrorV86::Invalid)?;
    Ok(len)
}

/// No writes occur before all validation and resource callbacks have succeeded.
pub fn encode_mixed_contract_v86<E>(
    input: &MixedContractInputV86<'_>,
    output: &mut [u8],
    pay: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV86<(), E> {
    let len = encoded_mixed_contract_v86_len(input, pay)?;
    if output.len() != len {
        return Err(MixedContractErrorV86::Invalid("mixed output extent"));
    }
    let mut w = Output::for_slice(output);
    MIXED_CONTRACT_MAGIC_V86.write(&mut w);
    86u16.write(&mut w);
    (input.arguments.len() as u16).write(&mut w);
    (input.occurrences.len() as u16).write(&mut w);
    0u16.write(&mut w);
    (len as u32).write(&mut w);
    input.subjects.write(&mut w);
    for row in input.arguments {
        row.write(&mut w);
    }
    for row in input.occurrences {
        row.write(&mut w);
    }
    debug_assert_eq!(w.position, len);
    Ok(())
}
pub fn decode_mixed_contract_v86<'a, E>(
    bytes: &'a [u8],
    pay: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV86<MixedContractV86<'a>, E> {
    if bytes.len() > MAX_MIXED_CONTRACT_BYTES_V86 {
        return Err(MixedContractErrorV86::Invalid("mixed frame limit"));
    }
    charge(bytes.len(), pay)?;
    let parse = || -> Format<_> {
        let mut r = Reader::new(bytes);
        if <[u8; 8]>::read(&mut r)? != MIXED_CONTRACT_MAGIC_V86 || u16::read(&mut r)? != 86 {
            return Err("mixed family/version");
        }
        let arguments = usize::from(u16::read(&mut r)?);
        let occurrences = usize::from(u16::read(&mut r)?);
        if u16::read(&mut r)? != 0
            || u32::read(&mut r)? as usize != bytes.len()
            || frame_len(arguments, occurrences)? != bytes.len()
        {
            return Err("mixed frame extent or flags");
        }
        let subjects = MixedContractSubjectsV26::read(&mut r)?;
        let view = MixedContractV86 {
            bytes,
            identity: [0; 32],
            subjects,
            arguments,
            occurrences,
        };
        validate(&view)?;
        Ok(view)
    };
    let mut view = parse().map_err(MixedContractErrorV86::Invalid)?;
    let mut hash = Sha256::new();
    hash.update(MIXED_CONTRACT_DOMAIN_V86);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    view.identity = hash.finalize().into();
    Ok(view)
}
impl<'a> MixedContractV86<'a> {
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn subjects(&self) -> &MixedContractSubjectsV26 {
        &self.subjects
    }
    pub const fn argument_count(&self) -> usize {
        self.arguments
    }
    pub const fn occurrence_count(&self) -> usize {
        self.occurrences
    }
    pub fn argument<E>(
        &self,
        i: usize,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV86<MixedArgumentV26, E> {
        pay(64 + 16 * MixedArgumentV26::BYTES).map_err(MixedContractErrorV86::Resource)?;
        Rows::argument(self, i).map_err(MixedContractErrorV86::Invalid)
    }
    pub fn occurrence<E>(
        &self,
        i: usize,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV86<MixedOccurrenceV86, E> {
        pay(64 + 16 * MixedOccurrenceV86::BYTES).map_err(MixedContractErrorV86::Resource)?;
        Rows::occurrence(self, i).map_err(MixedContractErrorV86::Invalid)
    }
    /// Commits an exact source/ABI argument row, including a zero-access row.
    /// This is data identity only; it cannot assert that an actual graph is unused.
    pub fn argument_identity<E>(
        &self,
        i: usize,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV86<[u8; 32], E> {
        pay(256 + 16 * MixedArgumentV26::BYTES).map_err(MixedContractErrorV86::Resource)?;
        if i >= self.arguments {
            return Err(MixedContractErrorV86::Invalid("mixed argument ordinal"));
        }
        let at = PREFIX + MixedContractSubjectsV26::BYTES + i * MixedArgumentV26::BYTES;
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-MIXED-ARGUMENT/V86\0");
        hash.update(self.identity);
        hash.update((i as u64).to_le_bytes());
        hash.update(&self.bytes[at..at + MixedArgumentV26::BYTES]);
        Ok(hash.finalize().into())
    }
    /// Binds the complete exact row and contract, never just a parameter/count.
    pub fn occurrence_identity<E>(
        &self,
        i: usize,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV86<[u8; 32], E> {
        pay(256 + 16 * MixedOccurrenceV86::BYTES).map_err(MixedContractErrorV86::Resource)?;
        if i >= self.occurrences {
            return Err(MixedContractErrorV86::Invalid("mixed occurrence ordinal"));
        }
        let at = PREFIX
            + MixedContractSubjectsV26::BYTES
            + self.arguments * MixedArgumentV26::BYTES
            + i * MixedOccurrenceV86::BYTES;
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-MIXED-OCCURRENCE/V86\0");
        hash.update(self.identity);
        hash.update((i as u64).to_le_bytes());
        hash.update(&self.bytes[at..at + MixedOccurrenceV86::BYTES]);
        Ok(hash.finalize().into())
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "mixed_conditional_v86_tests.rs"]
mod tests;
