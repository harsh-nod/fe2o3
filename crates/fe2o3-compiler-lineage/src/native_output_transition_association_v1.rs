//! Borrowed inert output-transition framing; protected proof admission is separate.

use crate::{
    InertLineageContentIdentityV3, InertNativeNeutralSubjectV1,
    InertProofBindingAssociationErrorV3, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3, NativeNeutralSubjectErrorV1,
};
use std::{error::Error, fmt};

/// Distinct output-transition preimage in the existing ProofBinding receipt slot.
pub const NATIVE_OUTPUT_TRANSITION_ASSOCIATION_MAGIC_V1: [u8; 8] = *b"F2NOUT1\0";
/// Canonical output-association schema, not a compiler refinement theorem.
pub const NATIVE_OUTPUT_TRANSITION_ASSOCIATION_VERSION_V1: u16 = 1;
/// Association-only policy. No legacy proof policy is reinterpreted as output proof.
pub const NATIVE_OUTPUT_TRANSITION_ASSOCIATION_POLICY_V1: u16 = 1;
/// Fixed header before semantic-root rows and four nested byte fields.
pub const NATIVE_OUTPUT_TRANSITION_ASSOCIATION_HEADER_BYTES_V1: usize = 348;
const HEADER: usize = NATIVE_OUTPUT_TRANSITION_ASSOCIATION_HEADER_BYTES_V1;
const ROOT_BYTES: usize = 16;

/// Four inert root coordinates. Only the last three axes are bounded permutations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeOutputTransitionRootV1 {
    semantic_root: u32,
    descriptor_ordinal: u32,
    input_kernel_ordinal: u32,
    output_kernel_ordinal: u32,
}
impl NativeOutputTransitionRootV1 {
    /// Constructs inert coordinates, not an authenticated source/output relation.
    pub const fn new(
        semantic_root: u32,
        descriptor_ordinal: u32,
        input_kernel_ordinal: u32,
        output_kernel_ordinal: u32,
    ) -> Self {
        Self {
            semantic_root,
            descriptor_ordinal,
            input_kernel_ordinal,
            output_kernel_ordinal,
        }
    }
    /// Actual semantic function ID; it need not be contiguous or less than root count.
    pub const fn semantic_root(self) -> u32 {
        self.semantic_root
    }
    /// Descriptor-canonical ordinal.
    pub const fn descriptor_ordinal(self) -> u32 {
        self.descriptor_ordinal
    }
    /// Kernel ordinal in the historical input graph.
    pub const fn input_kernel_ordinal(self) -> u32 {
        self.input_kernel_ordinal
    }
    /// Kernel ordinal in the optimized output graph.
    pub const fn output_kernel_ordinal(self) -> u32 {
        self.output_kernel_ordinal
    }
    fn words(self) -> [u32; 4] {
        [
            self.semantic_root,
            self.descriptor_ordinal,
            self.input_kernel_ordinal,
            self.output_kernel_ordinal,
        ]
    }
}

/// Borrowed contents of an inert output-transition association.
#[derive(Clone, Copy, Debug)]
pub struct NativeOutputTransitionAssociationInputsV1<'a> {
    /// Full original input graph-plus-catalog subject.
    pub input_subject: InertNativeNeutralSubjectV1,
    /// Full actual optimized output graph-plus-catalog subject.
    pub output_subject: InertNativeNeutralSubjectV1,
    /// Original input ProofBinding receipt identity, not the new wrapper identity.
    pub input_proof_binding: InertLineageContentIdentityV3,
    /// Existing outer KernelIr receipt identity for the output native envelope.
    pub output_kernel_ir: InertLineageContentIdentityV3,
    /// Existing outer FormalMemory receipt identity for the fresh output roster.
    pub output_formal_memory: InertLineageContentIdentityV3,
    /// Semantic-root-ordered four-axis rows, minimum one.
    pub roots: &'a [NativeOutputTransitionRootV1],
    /// Exact unchanged input V4 association preimage.
    pub input_association: &'a [u8],
    /// Original subject, historical input graph bytes and original catalog envelope.
    pub input_kernel_ir: &'a [u8],
    /// Exact unchanged input formal roster preimage.
    pub input_formal_memory: &'a [u8],
    /// Exact canonical nine-slice transition receipt.
    pub transition: &'a [u8],
}

/// Strict borrowed outer framing, never a checked graph or proof owner.
///
/// Nested fields remain opaque until their actual typed decoders and replay
/// checkers run. Framing and equal inert identities cannot authorize substitution.
/// Existing legacy protected proof consumers deliberately reject this schema.
/// No heap allocation occurs. Native subject decoding includes its existing
/// fixed-size identity hashes; opaque nested payloads are not hashed or admitted.
///
/// ```compile_fail
/// use fe2o3_compiler_lineage::NativeOutputTransitionAssociationRefV1;
/// fn escape(bytes: Vec<u8>) -> NativeOutputTransitionAssociationRefV1<'static> {
///     NativeOutputTransitionAssociationRefV1::decode(&bytes).unwrap()
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct NativeOutputTransitionAssociationRefV1<'a> {
    bytes: &'a [u8],
    input_subject: InertNativeNeutralSubjectV1,
    output_subject: InertNativeNeutralSubjectV1,
    identities: [InertLineageContentIdentityV3; 3],
    roots: &'a [u8],
    nested: [&'a [u8]; 4],
}
impl<'a> NativeOutputTransitionAssociationRefV1<'a> {
    /// Validates exact outer framing and root-axis partitions only.
    /// The aggregate cap applies to the entire wrapper, not separately to fields.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, NativeOutputTransitionAssociationErrorV1> {
        if !(HEADER..=MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3).contains(&bytes.len()) {
            return Err(ErrorV1::Length);
        }
        let mut reader = Reader { bytes, offset: 0 };
        if reader.fixed::<8>()? != NATIVE_OUTPUT_TRANSITION_ASSOCIATION_MAGIC_V1
            || reader.u16()? != NATIVE_OUTPUT_TRANSITION_ASSOCIATION_VERSION_V1
            || reader.u16()? != NATIVE_OUTPUT_TRANSITION_ASSOCIATION_POLICY_V1
        {
            return Err(ErrorV1::Header);
        }
        if usize::try_from(reader.u32()?).ok() != Some(bytes.len()) {
            return Err(ErrorV1::Length);
        }
        let input_subject =
            InertNativeNeutralSubjectV1::decode(reader.take(96)?).map_err(ErrorV1::Subject)?;
        let output_subject =
            InertNativeNeutralSubjectV1::decode(reader.take(96)?).map_err(ErrorV1::Subject)?;
        let identities = [reader.identity()?, reader.identity()?, reader.identity()?];
        let count = usize::try_from(reader.u32()?).map_err(|_| ErrorV1::Length)?;
        let lengths = [
            reader.usize()?,
            reader.usize()?,
            reader.usize()?,
            reader.usize()?,
        ];
        if extent(count, lengths)? != bytes.len() {
            return Err(ErrorV1::Length);
        }
        let roots = reader.take(count.checked_mul(ROOT_BYTES).ok_or(ErrorV1::Length)?)?;
        validate_roots(count, (0..count).map(|index| decode_root(roots, index)))?;
        let nested = [
            reader.take(lengths[0])?,
            reader.take(lengths[1])?,
            reader.take(lengths[2])?,
            reader.take(lengths[3])?,
        ];
        if reader.offset != bytes.len() {
            return Err(ErrorV1::Length);
        }
        Ok(Self {
            bytes,
            input_subject,
            output_subject,
            identities,
            roots,
            nested,
        })
    }
    /// Exact borrowed wrapper preimage, not its ProofBinding receipt identity.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Original input subject with unchanged import meaning.
    pub const fn input_subject(&self) -> &InertNativeNeutralSubjectV1 {
        &self.input_subject
    }
    /// Claimed output subject, requiring actual output admission.
    pub const fn output_subject(&self) -> &InertNativeNeutralSubjectV1 {
        &self.output_subject
    }
    /// Original nested input ProofBinding receipt identity.
    pub const fn input_proof_binding(&self) -> InertLineageContentIdentityV3 {
        self.identities[0]
    }
    /// Claimed exact outer output KernelIr receipt identity.
    pub const fn output_kernel_ir(&self) -> InertLineageContentIdentityV3 {
        self.identities[1]
    }
    /// Claimed exact outer output FormalMemory receipt identity.
    pub const fn output_formal_memory(&self) -> InertLineageContentIdentityV3 {
        self.identities[2]
    }
    /// Number of semantic-root rows, including a singleton.
    pub const fn root_count(&self) -> usize {
        self.roots.len() / ROOT_BYTES
    }
    /// Decodes one fixed row from this exact borrowed frame.
    pub fn root(&self, index: usize) -> Option<NativeOutputTransitionRootV1> {
        decode_root(self.roots, index).ok()
    }
    /// Unchanged nested input V4 bytes; not an output proof.
    pub const fn input_association(&self) -> &'a [u8] {
        self.nested[0]
    }
    /// Historical input envelope; not executable custody.
    pub const fn input_kernel_ir(&self) -> &'a [u8] {
        self.nested[1]
    }
    /// Unchanged input formal roster; never fresh output verification.
    pub const fn input_formal_memory(&self) -> &'a [u8] {
        self.nested[2]
    }
    /// Inert nine-slice bytes requiring actual endpoint inventory checking.
    pub const fn transition(&self) -> &'a [u8] {
        self.nested[3]
    }
    /// Neither outer framing nor any contained hash grants authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Returns the exact bounded encoded extent without allocating or parsing nested fields.
pub fn native_output_transition_association_length_v1(
    inputs: NativeOutputTransitionAssociationInputsV1<'_>,
) -> Result<usize, NativeOutputTransitionAssociationErrorV1> {
    extent(inputs.roots.len(), fields(inputs).map(<[u8]>::len))
}

/// Encodes an inert wrapper with one exact-capacity allocation.
///
/// Callers account for this Vec header and returned capacity before calling.
/// This dependency-light codec has fixed aggregate/root limits, not a shared work
/// ledger. Nested canonicality and all semantic replay remain consumer checks.
pub fn encode_native_output_transition_association_v1(
    inputs: NativeOutputTransitionAssociationInputsV1<'_>,
) -> Result<Vec<u8>, NativeOutputTransitionAssociationErrorV1> {
    let length = native_output_transition_association_length_v1(inputs)?;
    validate_roots(inputs.roots.len(), inputs.roots.iter().copied().map(Ok))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ErrorV1::Allocation)?;
    if bytes.capacity() != length {
        return Err(ErrorV1::Allocation);
    }
    bytes.extend_from_slice(&NATIVE_OUTPUT_TRANSITION_ASSOCIATION_MAGIC_V1);
    bytes.extend_from_slice(&NATIVE_OUTPUT_TRANSITION_ASSOCIATION_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&NATIVE_OUTPUT_TRANSITION_ASSOCIATION_POLICY_V1.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| ErrorV1::Length)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(inputs.input_subject.canonical_bytes());
    bytes.extend_from_slice(inputs.output_subject.canonical_bytes());
    for identity in [
        inputs.input_proof_binding,
        inputs.output_kernel_ir,
        inputs.output_formal_memory,
    ] {
        bytes.extend_from_slice(&identity.sha256());
        bytes.extend_from_slice(&identity.byte_len().to_le_bytes());
    }
    bytes.extend_from_slice(
        &u32::try_from(inputs.roots.len())
            .map_err(|_| ErrorV1::Length)?
            .to_le_bytes(),
    );
    for field in fields(inputs) {
        bytes.extend_from_slice(
            &u32::try_from(field.len())
                .map_err(|_| ErrorV1::Length)?
                .to_le_bytes(),
        );
    }
    for row in inputs.roots {
        for word in row.words() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    for field in fields(inputs) {
        bytes.extend_from_slice(field);
    }
    if bytes.len() != length {
        return Err(ErrorV1::Length);
    }
    Ok(bytes)
}

fn fields(inputs: NativeOutputTransitionAssociationInputsV1<'_>) -> [&[u8]; 4] {
    [
        inputs.input_association,
        inputs.input_kernel_ir,
        inputs.input_formal_memory,
        inputs.transition,
    ]
}
fn extent(count: usize, lengths: [usize; 4]) -> Result<usize, ErrorV1> {
    if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&count) {
        return Err(ErrorV1::RootAxes);
    }
    let mut length = count
        .checked_mul(ROOT_BYTES)
        .and_then(|n| HEADER.checked_add(n))
        .ok_or(ErrorV1::Length)?;
    for field in lengths {
        if field == 0 {
            return Err(ErrorV1::Length);
        }
        length = length.checked_add(field).ok_or(ErrorV1::Length)?;
    }
    if length > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
        return Err(ErrorV1::Length);
    }
    Ok(length)
}
fn validate_roots(
    count: usize,
    rows: impl Iterator<Item = Result<NativeOutputTransitionRootV1, ErrorV1>>,
) -> Result<(), ErrorV1> {
    let mut seen = [[false; MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3]; 3];
    let mut previous = None;
    let mut visited = 0_usize;
    for row in rows {
        let row = row?;
        if previous.is_some_and(|previous| row.semantic_root <= previous) {
            return Err(ErrorV1::RootAxes);
        }
        previous = Some(row.semantic_root);
        for (axis, ordinal) in seen.iter_mut().zip(row.words().into_iter().skip(1)) {
            let ordinal = usize::try_from(ordinal).map_err(|_| ErrorV1::RootAxes)?;
            if ordinal >= count || axis[ordinal] {
                return Err(ErrorV1::RootAxes);
            }
            axis[ordinal] = true;
        }
        visited = visited.checked_add(1).ok_or(ErrorV1::RootAxes)?;
    }
    if visited != count {
        return Err(ErrorV1::RootAxes);
    }
    Ok(())
}
fn decode_root(bytes: &[u8], index: usize) -> Result<NativeOutputTransitionRootV1, ErrorV1> {
    let start = index.checked_mul(ROOT_BYTES).ok_or(ErrorV1::Length)?;
    let mut reader = Reader {
        bytes: bytes
            .get(start..start.checked_add(ROOT_BYTES).ok_or(ErrorV1::Length)?)
            .ok_or(ErrorV1::Length)?,
        offset: 0,
    };
    Ok(NativeOutputTransitionRootV1::new(
        reader.u32()?,
        reader.u32()?,
        reader.u32()?,
        reader.u32()?,
    ))
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], ErrorV1> {
        let end = self.offset.checked_add(length).ok_or(ErrorV1::Length)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(ErrorV1::Length)?;
        self.offset = end;
        Ok(bytes)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ErrorV1> {
        self.take(N)?.try_into().map_err(|_| ErrorV1::Length)
    }
    fn u16(&mut self) -> Result<u16, ErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }
    fn u32(&mut self) -> Result<u32, ErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn usize(&mut self) -> Result<usize, ErrorV1> {
        usize::try_from(self.u32()?).map_err(|_| ErrorV1::Length)
    }
    fn identity(&mut self) -> Result<InertLineageContentIdentityV3, ErrorV1> {
        InertLineageContentIdentityV3::new(self.fixed()?, u64::from_le_bytes(self.fixed()?))
            .map_err(ErrorV1::Identity)
    }
}

/// Outer framing error only; it is not a result from any proof or graph checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOutputTransitionAssociationErrorV1 {
    /// Magic, schema or association policy differs.
    Header,
    /// Empty field, arithmetic overflow, aggregate bound, truncation or trailing data.
    Length,
    /// Semantic ordering or one of the three ordinal permutations is invalid.
    RootAxes,
    /// A full native subject has invalid canonical framing.
    Subject(NativeNeutralSubjectErrorV1),
    /// An inert receipt identity is zero or malformed.
    Identity(InertProofBindingAssociationErrorV3),
    /// Exact-capacity fallible allocation failed.
    Allocation,
}
type ErrorV1 = NativeOutputTransitionAssociationErrorV1;
impl fmt::Display for ErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header => f.write_str("native output association header differs"),
            Self::Length => f.write_str("native output association extent differs"),
            Self::RootAxes => f.write_str("native output association root axes differ"),
            Self::Subject(e) => write!(f, "native output association subject: {e}"),
            Self::Identity(e) => write!(f, "native output association identity: {e}"),
            Self::Allocation => f.write_str("native output association allocation failed"),
        }
    }
}
impl Error for ErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Subject(e) => Some(e),
            Self::Identity(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "native_output_transition_association_v1_tests.rs"]
mod tests;
