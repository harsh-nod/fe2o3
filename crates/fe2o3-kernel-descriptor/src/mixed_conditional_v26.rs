//! Lossless, bounded compiler/runtime data for mixed conditional accesses.
//!
//! Decoding authenticates no source, native owner, descriptor, proof, artifact,
//! or executable. Semantic type identity and descriptor layout identity are
//! different fields and are never substituted for one another. Formation and
//! dereference envelopes remain independent, even when their values coincide.

use crate::decode::Reader;
use crate::nominal_v3::Output;
use sha2::{Digest, Sha256};

pub const MAX_MIXED_ARGUMENTS_V26: usize = 64;
pub const MAX_MIXED_OCCURRENCES_V26: usize = 256;
pub const MAX_MIXED_CONTRACT_BYTES_V26: usize = 128 * 1024;
pub const MIXED_CONTRACT_MAGIC_V26: [u8; 8] = *b"FE2O3M26";
pub const MIXED_CONTRACT_DOMAIN_V26: &[u8] = b"FE2O3/CONDITIONAL-MIXED-CONTRACT/V26\0";
const PREFIX: usize = 20;

#[derive(Debug, Eq, PartialEq)]
pub enum MixedContractErrorV26<E> {
    Resource(E),
    Invalid(&'static str),
}
impl<E: std::fmt::Display> std::fmt::Display for MixedContractErrorV26<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Invalid(e) => f.write_str(e),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for MixedContractErrorV26<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}
type Format<T> = Result<T, &'static str>;
type ResultV26<T, E> = Result<T, MixedContractErrorV26<E>>;

trait Wire: Sized {
    const BYTES: usize;
    fn read(r: &mut Reader<'_>) -> Format<Self>;
    fn write(&self, w: &mut Output<'_>);
}
macro_rules! integer {
    ($ty:ty, $method:ident) => {
        impl Wire for $ty {
            const BYTES: usize = size_of::<Self>();
            fn read(r: &mut Reader<'_>) -> Format<Self> {
                r.$method().map_err(|_| "truncated mixed row")
            }
            fn write(&self, w: &mut Output<'_>) {
                w.$method(*self);
            }
        }
    };
}
integer!(u8, u8);
integer!(u16, u16);
integer!(u32, u32);
integer!(u64, u64);
impl<const N: usize> Wire for [u8; N] {
    const BYTES: usize = N;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        r.fixed().map_err(|_| "truncated mixed identity")
    }
    fn write(&self, w: &mut Output<'_>) {
        w.bytes(self);
    }
}
impl Wire for [u64; 3] {
    const BYTES: usize = 24;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        Ok([u64::read(r)?, u64::read(r)?, u64::read(r)?])
    }
    fn write(&self, w: &mut Output<'_>) {
        for value in self {
            value.write(w);
        }
    }
}
impl Wire for bool {
    const BYTES: usize = 1;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        match u8::read(r)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("mixed boolean"),
        }
    }
    fn write(&self, w: &mut Output<'_>) {
        u8::from(*self).write(w);
    }
}
macro_rules! wire_struct {
    ($(#[$meta:meta])* $name:ident { $($(#[$field_meta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name { $($(#[$field_meta])* pub $field: $ty),* }
        impl Wire for $name {
            const BYTES: usize = 0 $(+ <$ty as Wire>::BYTES)*;
            fn read(r: &mut Reader<'_>) -> Format<Self> {
                Ok(Self { $($field: <$ty as Wire>::read(r)?),* })
            }
            fn write(&self, w: &mut Output<'_>) { $(self.$field.write(w);)* }
        }
    };
}

wire_struct! {
    /// One kernel root; identities are claims until matched by an owning consumer.
    MixedContractSubjectsV26 {
        kernel_id: [u8; 32],
        source_semantic_identity: [u8; 32],
        original_graph_identity: [u8; 32],
        output_graph_identity: [u8; 32],
        /// Exact nominal V3 descriptor subject, with only its final code-object
        /// digest slot normalized. Final artifact authority binds that separately.
        descriptor_identity: [u8; 32],
        original_root: u32,
        output_function: u32,
        source_rank: u8,
        index_width: u8,
        exact_grid: [u64; 3],
        source_argument_count: u32,
        generated_field_count: u32,
        explicit_argument_bytes: u32,
        kernarg_alignment: u32
    }
}
wire_struct! {
    /// Stored ordinals in one exact graph, not raw block/value IDs.
    MixedOperationV26 { function: u32, block: u32, operation: u32 }
}
wire_struct! {
    /// The successor occurrence, not only its possibly repeated target block.
    MixedEdgeV26 { function: u32, block: u32, successor: u32 }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedDefinitionV26 {
    FunctionArgument {
        function: u32,
        argument: u32,
    },
    BlockArgument {
        function: u32,
        block: u32,
        argument: u32,
    },
    Result {
        operation: MixedOperationV26,
        result: u32,
    },
}
impl MixedDefinitionV26 {
    pub const fn function(self) -> u32 {
        match self {
            Self::FunctionArgument { function, .. } | Self::BlockArgument { function, .. } => {
                function
            }
            Self::Result { operation, .. } => operation.function,
        }
    }
}
impl Wire for MixedDefinitionV26 {
    const BYTES: usize = 17;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        let tag = u8::read(r)?;
        let function = u32::read(r)?;
        let a = u32::read(r)?;
        let b = u32::read(r)?;
        let c = u32::read(r)?;
        match (tag, b, c) {
            (0, 0, 0) => Ok(Self::FunctionArgument {
                function,
                argument: a,
            }),
            (1, _, 0) => Ok(Self::BlockArgument {
                function,
                block: a,
                argument: b,
            }),
            (2, _, _) => Ok(Self::Result {
                operation: MixedOperationV26 {
                    function,
                    block: a,
                    operation: b,
                },
                result: c,
            }),
            _ => Err("mixed definition tag or padding"),
        }
    }
    fn write(&self, w: &mut Output<'_>) {
        let (tag, function, a, b, c) = match *self {
            Self::FunctionArgument { function, argument } => (0u8, function, argument, 0, 0),
            Self::BlockArgument {
                function,
                block,
                argument,
            } => (1, function, block, argument, 0),
            Self::Result { operation, result } => (
                2,
                operation.function,
                operation.block,
                operation.operation,
                result,
            ),
        };
        tag.write(w);
        function.write(w);
        a.write(w);
        b.write(w);
        c.write(w);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedGuardPathV26 {
    ExplicitPredicate,
    /// Source/target are exact body-local BlockIds, independent of roster ordinals.
    TrueEdge {
        source: u32,
        successor: u64,
        target: u32,
    },
}
impl Wire for MixedGuardPathV26 {
    const BYTES: usize = 17;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        let tag = u8::read(r)?;
        let source = u32::read(r)?;
        let successor = u64::read(r)?;
        let target = u32::read(r)?;
        match (tag, source, successor, target) {
            (0, 0, 0, 0) => Ok(Self::ExplicitPredicate),
            (1, _, _, _) => Ok(Self::TrueEdge {
                source,
                successor,
                target,
            }),
            _ => Err("mixed guard path tag or padding"),
        }
    }
    fn write(&self, w: &mut Output<'_>) {
        let (tag, source, successor, target) = match *self {
            Self::ExplicitPredicate => (0u8, 0u32, 0u64, 0u32),
            Self::TrueEdge {
                source,
                successor,
                target,
            } => (1, source, successor, target),
        };
        tag.write(w);
        source.write(w);
        successor.write(w);
        target.write(w);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedIndexEnvelopeV26 {
    LogicalExtent { argument: u16 },
    InvocationAxis { axis: u8 },
    UnsignedWidth { bits: u8 },
}
impl Wire for MixedIndexEnvelopeV26 {
    const BYTES: usize = 3;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        let tag = u8::read(r)?;
        let value = u16::read(r)?;
        match tag {
            0 => Ok(Self::LogicalExtent { argument: value }),
            1 if value < 3 => Ok(Self::InvocationAxis { axis: value as u8 }),
            2 if (1..=64).contains(&value) => Ok(Self::UnsignedWidth { bits: value as u8 }),
            _ => Err("mixed envelope tag or range"),
        }
    }
    fn write(&self, w: &mut Output<'_>) {
        let (tag, value) = match *self {
            Self::LogicalExtent { argument } => (0u8, argument),
            Self::InvocationAxis { axis } => (1, u16::from(axis)),
            Self::UnsignedWidth { bits } => (2, u16::from(bits)),
        };
        tag.write(w);
        value.write(w);
    }
}

/// Exact canonical scalar kind, including kinds not admitted by the old host ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedScalarV26 {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    Index,
    F16,
    Bf16,
    F32,
    F64,
}
impl MixedScalarV26 {
    pub const fn element_bytes(self) -> u64 {
        match self {
            Self::Bool | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 | Self::F16 | Self::Bf16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
            Self::I128 | Self::U128 => 16,
            // This ABI has 64-bit pointers/Index. Formal index_width limits
            // admissible runtime index arithmetic, not scalar storage layout.
            Self::Index => 8,
        }
    }
}
const SCALARS: [MixedScalarV26; 16] = [
    MixedScalarV26::Bool,
    MixedScalarV26::I8,
    MixedScalarV26::I16,
    MixedScalarV26::I32,
    MixedScalarV26::I64,
    MixedScalarV26::I128,
    MixedScalarV26::U8,
    MixedScalarV26::U16,
    MixedScalarV26::U32,
    MixedScalarV26::U64,
    MixedScalarV26::U128,
    MixedScalarV26::Index,
    MixedScalarV26::F16,
    MixedScalarV26::Bf16,
    MixedScalarV26::F32,
    MixedScalarV26::F64,
];
impl Wire for MixedScalarV26 {
    const BYTES: usize = 1;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        SCALARS
            .get(usize::from(u8::read(r)?))
            .copied()
            .ok_or("mixed scalar tag")
    }
    fn write(&self, w: &mut Output<'_>) {
        // Closed enum and explicit stable roster; never reuse an IR discriminant.
        (SCALARS.iter().position(|value| value == self).unwrap() as u8).write(w);
    }
}

wire_struct! {
    MixedArgumentV26 {
        source_argument: u32,
        generated_field: u16,
        physical_parameter: u32,
        semantic_type: u32,
        semantic_type_identity: [u8; 32],
        descriptor_type_identity: [u8; 32],
        device_layout_identity: [u8; 32],
        scalar: MixedScalarV26,
        pointer_offset: u32,
        length_offset: u32,
        reads: u32,
        writes: u32,
        source_exclusive: bool
    }
}

/// Exact physical access representation, distinct from the Global allocation
/// domain. Generic is never relabeled Global and does not prove its origin.
/// Older decoders reject the new Generic tag instead of interpreting it as 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedMemorySpaceV26 {
    Global,
    Generic,
}
impl Wire for MixedMemorySpaceV26 {
    const BYTES: usize = 1;
    fn read(r: &mut Reader<'_>) -> Format<Self> {
        match u8::read(r)? {
            1 => Ok(Self::Global),
            2 => Ok(Self::Generic),
            _ => Err("mixed memory space"),
        }
    }
    fn write(&self, w: &mut Output<'_>) {
        match self {
            Self::Global => 1u8,
            Self::Generic => 2u8,
        }
        .write(w);
    }
}
wire_struct! {
    /// Every occurrence is retained in producer order, including repeated arguments.
    MixedOccurrenceV26 {
        argument: u16,
        original_instance: u32,
        original_operation: MixedOperationV26,
        output_operation: MixedOperationV26,
        original_formation: MixedOperationV26,
        output_formation: MixedOperationV26,
        output_address_index: MixedDefinitionV26,
        output_guard_edge: MixedEdgeV26,
        output_guard_condition: MixedDefinitionV26,
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

pub struct MixedContractInputV26<'a> {
    pub subjects: MixedContractSubjectsV26,
    /// Strict original argument order, independently of physical parameter ordinals.
    pub arguments: &'a [MixedArgumentV26],
    pub occurrences: &'a [MixedOccurrenceV26],
}

/// A borrowed canonical data view, not a compiler or verification certificate.
pub struct MixedContractV26<'a> {
    bytes: &'a [u8],
    identity: [u8; 32],
    subjects: MixedContractSubjectsV26,
    arguments: usize,
    occurrences: usize,
}
pub const MIXED_CONTRACT_CODEC_STORAGE_V26: usize = size_of::<MixedContractV26<'static>>()
    + size_of::<MixedContractInputV26<'static>>()
    + size_of::<MixedContractSubjectsV26>() * 2
    + size_of::<MixedArgumentV26>() * 2
    + size_of::<MixedOccurrenceV26>() * 2
    + size_of::<[[u32; 2]; MAX_MIXED_ARGUMENTS_V26]>()
    + size_of::<Reader<'static>>() * 2
    + size_of::<Output<'static>>()
    + size_of::<Sha256>()
    + 1024;

trait Rows {
    fn subjects(&self) -> &MixedContractSubjectsV26;
    fn argument_count(&self) -> usize;
    fn occurrence_count(&self) -> usize;
    fn argument(&self, i: usize) -> Format<MixedArgumentV26>;
    fn occurrence(&self, i: usize) -> Format<MixedOccurrenceV26>;
}
impl Rows for MixedContractInputV26<'_> {
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
    fn occurrence(&self, i: usize) -> Format<MixedOccurrenceV26> {
        self.occurrences
            .get(i)
            .copied()
            .ok_or("mixed occurrence ordinal")
    }
}
impl Rows for MixedContractV26<'_> {
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
    fn occurrence(&self, i: usize) -> Format<MixedOccurrenceV26> {
        if i >= self.occurrences {
            return Err("mixed occurrence ordinal");
        }
        read_at(
            self.bytes,
            PREFIX
                + MixedContractSubjectsV26::BYTES
                + self.arguments * MixedArgumentV26::BYTES
                + i * MixedOccurrenceV26::BYTES,
        )
    }
}
fn read_at<T: Wire>(bytes: &[u8], at: usize) -> Format<T> {
    let row = bytes.get(at..at + T::BYTES).ok_or("mixed row bounds")?;
    T::read(&mut Reader::new(row))
}
fn frame_len(arguments: usize, occurrences: usize) -> Format<usize> {
    if arguments > MAX_MIXED_ARGUMENTS_V26 || occurrences > MAX_MIXED_OCCURRENCES_V26 {
        return Err("mixed roster limit");
    }
    let bytes = PREFIX
        + MixedContractSubjectsV26::BYTES
        + arguments * MixedArgumentV26::BYTES
        + occurrences * MixedOccurrenceV26::BYTES;
    if bytes > MAX_MIXED_CONTRACT_BYTES_V26 {
        return Err("mixed frame limit");
    }
    Ok(bytes)
}
fn charge<E>(bytes: usize, pay: &mut impl FnMut(usize) -> Result<(), E>) -> ResultV26<(), E> {
    // Bounded rows: two full structural visits, scalar tags, row copies, length
    // checks, canonical output and SHA input. No hidden sort or pairwise scan.
    pay(4096 + 32 * bytes).map_err(MixedContractErrorV26::Resource)
}
fn validate(rows: &impl Rows) -> Format<()> {
    let s = rows.subjects();
    if !(1..=3).contains(&s.source_rank)
        || !matches!(s.index_width, 32 | 64)
        || !s.kernarg_alignment.is_power_of_two()
        || s.explicit_argument_bytes > 64 * 1024
        || s.exact_grid
            .iter()
            .any(|n| *n == 0 || *n > u64::from(u32::MAX))
        || s.exact_grid[usize::from(s.source_rank)..]
            .iter()
            .any(|n| *n != 1)
        || (rows.argument_count() == 0 && rows.occurrence_count() != 0)
    {
        return Err("mixed launch or ABI header");
    }
    let mut previous = None;
    let mut counts = [[0u32; 2]; MAX_MIXED_ARGUMENTS_V26];
    for i in 0..rows.argument_count() {
        let a = rows.argument(i)?;
        if previous.is_some_and(|old| old >= a.source_argument)
            || a.source_argument >= s.source_argument_count
            || u32::from(a.generated_field) >= s.generated_field_count
            || a.reads
                .checked_add(a.writes)
                .filter(|n| *n <= MAX_MIXED_OCCURRENCES_V26 as u32)
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
            || o.output_guard_edge.function != s.output_function
            || o.output_guard_condition.function() != s.output_function
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

pub fn encoded_mixed_contract_v26_len<E>(
    input: &MixedContractInputV26<'_>,
    pay: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV26<usize, E> {
    let len = frame_len(input.arguments.len(), input.occurrences.len())
        .map_err(MixedContractErrorV26::Invalid)?;
    charge(len, pay)?;
    validate(input).map_err(MixedContractErrorV26::Invalid)?;
    Ok(len)
}

/// Commits every nominal descriptor byte except its final code-object digest.
/// The latter is filled by finalization and must not make the source contract
/// self-referential. This content identity grants no descriptor/proof authority.
/// Caller prepays the codec scratch and retains the actual borrowed table.
pub fn mixed_descriptor_subject_v26<E>(
    table: &crate::DeviceDescriptorTableV3<'_>,
    pay: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV26<[u8; 32], E> {
    let bytes = table.canonical_bytes();
    pay(bytes.len() + 256).map_err(MixedContractErrorV26::Resource)?;
    let at = crate::CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-MIXED-DESCRIPTOR-SUBJECT/V26\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(&bytes[..at]);
    hash.update([0u8; 32]);
    hash.update(&bytes[at + 32..]);
    Ok(hash.finalize().into())
}
/// No writes occur before all validation and resource callbacks have succeeded.
pub fn encode_mixed_contract_v26<E>(
    input: &MixedContractInputV26<'_>,
    output: &mut [u8],
    pay: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV26<(), E> {
    let len = encoded_mixed_contract_v26_len(input, pay)?;
    if output.len() != len {
        return Err(MixedContractErrorV26::Invalid("mixed output extent"));
    }
    let mut w = Output::for_slice(output);
    MIXED_CONTRACT_MAGIC_V26.write(&mut w);
    26u16.write(&mut w);
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
pub fn decode_mixed_contract_v26<'a, E>(
    bytes: &'a [u8],
    pay: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV26<MixedContractV26<'a>, E> {
    if bytes.len() > MAX_MIXED_CONTRACT_BYTES_V26 {
        return Err(MixedContractErrorV26::Invalid("mixed frame limit"));
    }
    charge(bytes.len(), pay)?;
    let parse = || -> Format<_> {
        let mut r = Reader::new(bytes);
        if <[u8; 8]>::read(&mut r)? != MIXED_CONTRACT_MAGIC_V26 || u16::read(&mut r)? != 26 {
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
        let view = MixedContractV26 {
            bytes,
            identity: [0; 32],
            subjects,
            arguments,
            occurrences,
        };
        validate(&view)?;
        Ok(view)
    };
    let mut view = parse().map_err(MixedContractErrorV26::Invalid)?;
    let mut hash = Sha256::new();
    hash.update(MIXED_CONTRACT_DOMAIN_V26);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    view.identity = hash.finalize().into();
    Ok(view)
}
impl<'a> MixedContractV26<'a> {
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
    ) -> ResultV26<MixedArgumentV26, E> {
        pay(64 + 16 * MixedArgumentV26::BYTES).map_err(MixedContractErrorV26::Resource)?;
        Rows::argument(self, i).map_err(MixedContractErrorV26::Invalid)
    }
    pub fn occurrence<E>(
        &self,
        i: usize,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV26<MixedOccurrenceV26, E> {
        pay(64 + 16 * MixedOccurrenceV26::BYTES).map_err(MixedContractErrorV26::Resource)?;
        Rows::occurrence(self, i).map_err(MixedContractErrorV26::Invalid)
    }
    /// Commits an exact source/ABI argument row, including a zero-access row.
    /// This is data identity only; it cannot assert that an actual graph is unused.
    pub fn argument_identity<E>(
        &self,
        i: usize,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV26<[u8; 32], E> {
        pay(256 + 16 * MixedArgumentV26::BYTES).map_err(MixedContractErrorV26::Resource)?;
        if i >= self.arguments {
            return Err(MixedContractErrorV26::Invalid("mixed argument ordinal"));
        }
        let at = PREFIX + MixedContractSubjectsV26::BYTES + i * MixedArgumentV26::BYTES;
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-MIXED-ARGUMENT/V26\0");
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
    ) -> ResultV26<[u8; 32], E> {
        pay(256 + 16 * MixedOccurrenceV26::BYTES).map_err(MixedContractErrorV26::Resource)?;
        if i >= self.occurrences {
            return Err(MixedContractErrorV26::Invalid("mixed occurrence ordinal"));
        }
        let at = PREFIX
            + MixedContractSubjectsV26::BYTES
            + self.arguments * MixedArgumentV26::BYTES
            + i * MixedOccurrenceV26::BYTES;
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-MIXED-OCCURRENCE/V26\0");
        hash.update(self.identity);
        hash.update((i as u64).to_le_bytes());
        hash.update(&self.bytes[at..at + MixedOccurrenceV26::BYTES]);
        Ok(hash.finalize().into())
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "mixed_conditional_v26_tests.rs"]
mod tests;
