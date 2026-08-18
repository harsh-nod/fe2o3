use std::collections::{HashMap, HashSet};
use std::fmt::{self, Write};

use crate::{MirAddressSpace, MirMutability};

const MAGIC: &[u8; 8] = b"F2MCONST";
const VERSION: u16 = 1;
const FLAGS: u16 = 0;

pub const MAX_CONSTANT_ALLOCATIONS: usize = 4_096;
pub const MAX_CONSTANT_ALLOCATION_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CONSTANT_TOTAL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_CONSTANT_RELOCATIONS: usize = 65_536;
pub const MAX_CONSTANT_IDENTITY_BYTES: usize = 1_024;
pub const MAX_CONSTANT_GRAPH_DEPTH: usize = 64;
pub const MAX_CONSTANT_WIRE_BYTES: usize = 80 * 1024 * 1024;

/// A pool of rustc constant allocations described without target inference.
///
/// This is validation data only. A valid pool grants no permission to lower,
/// load, dereference, or launch any represented value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSemanticConstantPool {
    pub allocations: Vec<MirConstantAllocation>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirAllocationId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirByteOffset(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirAlignment(pub u64);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirConstantIdentity(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirPromotedIdentity {
    pub owner: MirConstantIdentity,
    pub index: u32,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirStaticIdentity(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirMemoryIdentity(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirSymbolIdentity(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MirAllocationOrigin {
    Constant(MirConstantIdentity),
    Promoted(MirPromotedIdentity),
    Static(MirStaticIdentity),
    /// A named backing allocation reachable from a constant or static.
    Memory(MirMemoryIdentity),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirConstantRepresentation {
    Scalar,
    Aggregate,
}

/// Raw allocation bytes and rustc's initialized-byte evidence.
///
/// Relocated pointer bytes must be zero in `bytes`; the pointer addend lives in
/// `target_offset`. This removes an otherwise ambiguous second representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirConstantAllocation {
    pub id: MirAllocationId,
    pub origin: MirAllocationOrigin,
    pub representation: MirConstantRepresentation,
    pub bytes: Vec<u8>,
    pub initialized: MirInitializedMask,
    pub alignment: MirAlignment,
    pub address_space: MirAddressSpace,
    pub mutability: MirMutability,
    pub relocations: Vec<MirPointerRelocation>,
}

/// One bit per allocation byte, least-significant bit first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInitializedMask {
    pub byte_len: u64,
    pub bits: Vec<u8>,
}

impl MirInitializedMask {
    pub fn all(byte_len: usize) -> Self {
        let storage_len = byte_len.div_ceil(8);
        let mut bits = vec![u8::MAX; storage_len];
        if let Some(last) = bits.last_mut()
            && !byte_len.is_multiple_of(8)
        {
            *last = (1_u8 << (byte_len % 8)) - 1;
        }
        Self {
            byte_len: u64::try_from(byte_len).expect("usize must fit u64"),
            bits,
        }
    }

    pub fn none(byte_len: usize) -> Self {
        Self {
            byte_len: u64::try_from(byte_len).expect("usize must fit u64"),
            bits: vec![0; byte_len.div_ceil(8)],
        }
    }

    pub fn is_initialized(&self, offset: MirByteOffset) -> Option<bool> {
        if offset.0 >= self.byte_len {
            return None;
        }
        let index = usize::try_from(offset.0).ok()?;
        Some(self.bits[index / 8] & (1 << (index % 8)) != 0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirPointerWidth(pub u8);

/// Pointer provenance retained from rustc's allocation model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirPointerProvenance {
    Allocation(MirAllocationId),
    Static(MirStaticIdentity),
    Function(MirSymbolIdentity),
    VTable(MirSymbolIdentity),
    ThreadLocal(MirStaticIdentity),
    Unknown(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPointerRelocation {
    pub offset: MirByteOffset,
    pub width: MirPointerWidth,
    pub provenance: MirPointerProvenance,
    pub target_offset: MirByteOffset,
    pub address_space: MirAddressSpace,
    pub mutability: MirMutability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirConstantValidationError {
    path: String,
    reason: String,
}

impl MirConstantValidationError {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for MirConstantValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MirConstantValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirConstantDecodeError {
    InputTooLarge,
    UnexpectedEnd,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    UnknownTag { field: &'static str, tag: u8 },
    InvalidUtf8,
    LimitExceeded(&'static str),
    TrailingBytes,
    NonCanonical,
    Validation(MirConstantValidationError),
}

impl fmt::Display for MirConstantDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => formatter.write_str("constant wire input exceeds its bound"),
            Self::UnexpectedEnd => formatter.write_str("constant wire input ended unexpectedly"),
            Self::InvalidMagic => formatter.write_str("invalid constant wire magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unknown constant version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported constant flags {flags:#06x}")
            }
            Self::UnknownTag { field, tag } => write!(formatter, "unknown {field} tag {tag}"),
            Self::InvalidUtf8 => formatter.write_str("constant identity is not UTF-8"),
            Self::LimitExceeded(limit) => write!(formatter, "constant {limit} limit exceeded"),
            Self::TrailingBytes => formatter.write_str("trailing constant wire bytes"),
            Self::NonCanonical => formatter.write_str("constant wire input is not canonical"),
            Self::Validation(error) => write!(formatter, "invalid constant model: {error}"),
        }
    }
}

impl std::error::Error for MirConstantDecodeError {}

impl From<MirConstantValidationError> for MirConstantDecodeError {
    fn from(value: MirConstantValidationError) -> Self {
        Self::Validation(value)
    }
}

impl MirSemanticConstantPool {
    pub fn validate(&self) -> Result<(), MirConstantValidationError> {
        if self.allocations.len() > MAX_CONSTANT_ALLOCATIONS {
            return Err(MirConstantValidationError::new(
                "constants.allocations",
                "allocation count exceeds the resource bound",
            ));
        }

        let mut origins = HashSet::with_capacity(self.allocations.len());
        let mut statics = HashMap::new();
        let mut total_bytes = 0_usize;
        let mut total_relocations = 0_usize;

        for (index, allocation) in self.allocations.iter().enumerate() {
            let path = format!("constants.allocation[{index}]");
            let expected_id = u32::try_from(index).map_err(|_| {
                MirConstantValidationError::new(&path, "allocation index does not fit u32")
            })?;
            if allocation.id != MirAllocationId(expected_id) {
                return Err(MirConstantValidationError::new(
                    format!("{path}.id"),
                    "allocation IDs must be contiguous and ascending from zero",
                ));
            }
            validate_origin(&allocation.origin, &format!("{path}.origin"))?;
            let origin_key = canonical_origin_key(&allocation.origin);
            if !origins.insert(origin_key) {
                return Err(MirConstantValidationError::new(
                    format!("{path}.origin"),
                    "allocation origins must be unique",
                ));
            }
            if let MirAllocationOrigin::Static(identity) = &allocation.origin {
                statics.insert(identity.0.as_str(), allocation.id);
            }
            if allocation.bytes.len() > MAX_CONSTANT_ALLOCATION_BYTES {
                return Err(MirConstantValidationError::new(
                    format!("{path}.bytes"),
                    "allocation byte length exceeds the per-allocation bound",
                ));
            }
            total_bytes = total_bytes
                .checked_add(allocation.bytes.len())
                .ok_or_else(|| {
                    MirConstantValidationError::new(
                        "constants.allocations",
                        "total allocation byte length overflows usize",
                    )
                })?;
            if total_bytes > MAX_CONSTANT_TOTAL_BYTES {
                return Err(MirConstantValidationError::new(
                    "constants.allocations",
                    "total allocation byte length exceeds the resource bound",
                ));
            }
            total_relocations = total_relocations
                .checked_add(allocation.relocations.len())
                .ok_or_else(|| {
                    MirConstantValidationError::new(
                        "constants.allocations",
                        "total relocation count overflows usize",
                    )
                })?;
            if total_relocations > MAX_CONSTANT_RELOCATIONS {
                return Err(MirConstantValidationError::new(
                    "constants.allocations",
                    "total relocation count exceeds the resource bound",
                ));
            }
            validate_allocation_shape(allocation, &path)?;
        }

        let mut edges = vec![Vec::new(); self.allocations.len()];
        for (index, allocation) in self.allocations.iter().enumerate() {
            let path = format!("constants.allocation[{index}]");
            validate_relocations(self, allocation, &statics, &path, &mut edges[index])?;
        }
        validate_acyclic(&edges)
    }

    pub fn canonical_text(&self) -> Result<String, MirConstantValidationError> {
        self.validate()?;
        let mut output = String::from("mir.constants.v1{allocations=[");
        for (index, allocation) in self.allocations.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            write_allocation_text(&mut output, allocation);
        }
        output.push_str("]}");
        Ok(output)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, MirConstantValidationError> {
        self.validate()?;
        let mut writer = Writer::new();
        writer.bytes.extend_from_slice(MAGIC);
        writer.u16(VERSION);
        writer.u16(FLAGS);
        writer.u32(u32::try_from(self.allocations.len()).expect("bounded allocation count"));
        for allocation in &self.allocations {
            encode_allocation(&mut writer, allocation);
        }
        if writer.bytes.len() > MAX_CONSTANT_WIRE_BYTES {
            return Err(MirConstantValidationError::new(
                "constants",
                "canonical encoding exceeds the wire resource bound",
            ));
        }
        Ok(writer.bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MirConstantDecodeError> {
        if bytes.len() > MAX_CONSTANT_WIRE_BYTES {
            return Err(MirConstantDecodeError::InputTooLarge);
        }
        let mut reader = Reader::new(bytes);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(MirConstantDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != VERSION {
            return Err(MirConstantDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != FLAGS {
            return Err(MirConstantDecodeError::UnsupportedFlags(flags));
        }
        let count = reader.bounded_count(MAX_CONSTANT_ALLOCATIONS, "allocation count")?;
        let mut allocations = Vec::with_capacity(count);
        for _ in 0..count {
            allocations.push(decode_allocation(&mut reader)?);
        }
        if !reader.is_empty() {
            return Err(MirConstantDecodeError::TrailingBytes);
        }
        let pool = Self { allocations };
        pool.validate()?;
        if pool.to_bytes()? != bytes {
            return Err(MirConstantDecodeError::NonCanonical);
        }
        Ok(pool)
    }
}

fn validate_origin(
    origin: &MirAllocationOrigin,
    path: &str,
) -> Result<(), MirConstantValidationError> {
    match origin {
        MirAllocationOrigin::Constant(identity) => validate_identity(&identity.0, path),
        MirAllocationOrigin::Promoted(identity) => {
            validate_identity(&identity.owner.0, &format!("{path}.owner"))
        }
        MirAllocationOrigin::Static(identity) => validate_identity(&identity.0, path),
        MirAllocationOrigin::Memory(identity) => validate_identity(&identity.0, path),
    }
}

fn validate_identity(value: &str, path: &str) -> Result<(), MirConstantValidationError> {
    if value.is_empty() {
        return Err(MirConstantValidationError::new(
            path,
            "identity must not be empty",
        ));
    }
    if value.len() > MAX_CONSTANT_IDENTITY_BYTES {
        return Err(MirConstantValidationError::new(
            path,
            "identity exceeds its byte-length bound",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(MirConstantValidationError::new(
            path,
            "identity must not contain control characters",
        ));
    }
    Ok(())
}

fn canonical_origin_key(origin: &MirAllocationOrigin) -> String {
    match origin {
        MirAllocationOrigin::Constant(identity) => format!("c:{}", identity.0),
        MirAllocationOrigin::Promoted(identity) => {
            format!("p:{}:{}", identity.owner.0, identity.index)
        }
        MirAllocationOrigin::Static(identity) => format!("s:{}", identity.0),
        MirAllocationOrigin::Memory(identity) => format!("m:{}", identity.0),
    }
}

fn validate_allocation_shape(
    allocation: &MirConstantAllocation,
    path: &str,
) -> Result<(), MirConstantValidationError> {
    if allocation.alignment.0 == 0 || !allocation.alignment.0.is_power_of_two() {
        return Err(MirConstantValidationError::new(
            format!("{path}.alignment"),
            "alignment must be a nonzero power of two",
        ));
    }
    let byte_len = u64::try_from(allocation.bytes.len()).map_err(|_| {
        MirConstantValidationError::new(format!("{path}.bytes"), "byte length does not fit u64")
    })?;
    if allocation.initialized.byte_len != byte_len {
        return Err(MirConstantValidationError::new(
            format!("{path}.initialized"),
            "initialized mask byte length must equal the allocation byte length",
        ));
    }
    let storage_len = allocation.bytes.len().div_ceil(8);
    if allocation.initialized.bits.len() != storage_len {
        return Err(MirConstantValidationError::new(
            format!("{path}.initialized.bits"),
            "initialized mask storage length is not canonical",
        ));
    }
    if let Some(last) = allocation.initialized.bits.last()
        && !allocation.bytes.len().is_multiple_of(8)
    {
        let used = allocation.bytes.len() % 8;
        let unused_mask = u8::MAX << used;
        if last & unused_mask != 0 {
            return Err(MirConstantValidationError::new(
                format!("{path}.initialized.bits"),
                "unused initialized-mask bits must be zero",
            ));
        }
    }
    if allocation.representation == MirConstantRepresentation::Scalar {
        if allocation.bytes.is_empty() || allocation.bytes.len() > 16 {
            return Err(MirConstantValidationError::new(
                format!("{path}.representation"),
                "scalar allocation size must be 1..=16 bytes",
            ));
        }
        if (0..byte_len).any(|offset| {
            allocation.initialized.is_initialized(MirByteOffset(offset)) != Some(true)
        }) {
            return Err(MirConstantValidationError::new(
                format!("{path}.initialized"),
                "scalar allocation bytes must all be initialized",
            ));
        }
    }
    Ok(())
}

fn validate_relocations(
    pool: &MirSemanticConstantPool,
    allocation: &MirConstantAllocation,
    statics: &HashMap<&str, MirAllocationId>,
    path: &str,
    edges: &mut Vec<usize>,
) -> Result<(), MirConstantValidationError> {
    let source_len = u64::try_from(allocation.bytes.len()).map_err(|_| {
        MirConstantValidationError::new(format!("{path}.bytes"), "byte length does not fit u64")
    })?;
    let mut previous_end = 0_u64;
    for (index, relocation) in allocation.relocations.iter().enumerate() {
        let relocation_path = format!("{path}.relocation[{index}]");
        let width = match relocation.width.0 {
            4 | 8 | 16 => u64::from(relocation.width.0),
            _ => {
                return Err(MirConstantValidationError::new(
                    format!("{relocation_path}.width"),
                    "pointer width must be 4, 8, or 16 bytes",
                ));
            }
        };
        let end = relocation.offset.0.checked_add(width).ok_or_else(|| {
            MirConstantValidationError::new(
                format!("{relocation_path}.offset"),
                "relocation byte range overflows u64",
            )
        })?;
        if end > source_len {
            return Err(MirConstantValidationError::new(
                format!("{relocation_path}.offset"),
                "relocation extends beyond its source allocation",
            ));
        }
        if index != 0 && relocation.offset.0 < previous_end {
            return Err(MirConstantValidationError::new(
                format!("{relocation_path}.offset"),
                "relocations must be ordered by offset and must not overlap",
            ));
        }
        previous_end = end;
        for byte_offset in relocation.offset.0..end {
            let byte_index = usize::try_from(byte_offset).map_err(|_| {
                MirConstantValidationError::new(
                    format!("{relocation_path}.offset"),
                    "relocation byte offset does not fit usize",
                )
            })?;
            if allocation.bytes[byte_index] != 0 {
                return Err(MirConstantValidationError::new(
                    format!("{relocation_path}.offset"),
                    "relocated bytes must be zero; use target_offset for the addend",
                ));
            }
            if allocation
                .initialized
                .is_initialized(MirByteOffset(byte_offset))
                != Some(true)
            {
                return Err(MirConstantValidationError::new(
                    format!("{relocation_path}.offset"),
                    "relocated bytes must be initialized",
                ));
            }
        }

        let target_id = match &relocation.provenance {
            MirPointerProvenance::Allocation(id) => *id,
            MirPointerProvenance::Static(identity) => {
                validate_identity(&identity.0, &format!("{relocation_path}.provenance"))?;
                *statics.get(identity.0.as_str()).ok_or_else(|| {
                    MirConstantValidationError::new(
                        format!("{relocation_path}.provenance"),
                        "static provenance does not resolve to an allocation",
                    )
                })?
            }
            MirPointerProvenance::Function(identity) => {
                validate_identity(&identity.0, &format!("{relocation_path}.provenance"))?;
                return Err(MirConstantValidationError::new(
                    format!("{relocation_path}.provenance"),
                    "function relocations are not supported",
                ));
            }
            MirPointerProvenance::VTable(identity) => {
                validate_identity(&identity.0, &format!("{relocation_path}.provenance"))?;
                return Err(MirConstantValidationError::new(
                    format!("{relocation_path}.provenance"),
                    "vtable relocations are not supported",
                ));
            }
            MirPointerProvenance::ThreadLocal(identity) => {
                validate_identity(&identity.0, &format!("{relocation_path}.provenance"))?;
                return Err(MirConstantValidationError::new(
                    format!("{relocation_path}.provenance"),
                    "thread-local relocations are not supported",
                ));
            }
            MirPointerProvenance::Unknown(_) => {
                return Err(MirConstantValidationError::new(
                    format!("{relocation_path}.provenance"),
                    "unknown relocation provenance is not supported",
                ));
            }
        };
        let target_index = usize::try_from(target_id.0).map_err(|_| {
            MirConstantValidationError::new(
                format!("{relocation_path}.provenance"),
                "target allocation ID does not fit usize",
            )
        })?;
        let target = pool.allocations.get(target_index).ok_or_else(|| {
            MirConstantValidationError::new(
                format!("{relocation_path}.provenance"),
                "target allocation ID is out of range",
            )
        })?;
        if target.id != target_id {
            return Err(MirConstantValidationError::new(
                format!("{relocation_path}.provenance"),
                "target allocation ID is not canonical",
            ));
        }
        let target_len = u64::try_from(target.bytes.len()).map_err(|_| {
            MirConstantValidationError::new(
                format!("{relocation_path}.target_offset"),
                "target byte length does not fit u64",
            )
        })?;
        if relocation.target_offset.0 > target_len {
            return Err(MirConstantValidationError::new(
                format!("{relocation_path}.target_offset"),
                "target offset exceeds the target allocation (one-past is allowed)",
            ));
        }
        if relocation.address_space != target.address_space {
            return Err(MirConstantValidationError::new(
                format!("{relocation_path}.address_space"),
                "pointer and target address spaces differ",
            ));
        }
        if relocation.mutability == MirMutability::Mutable
            && target.mutability != MirMutability::Mutable
        {
            return Err(MirConstantValidationError::new(
                format!("{relocation_path}.mutability"),
                "mutable pointer provenance requires mutable target storage",
            ));
        }
        edges.push(target_index);
    }

    if allocation.representation == MirConstantRepresentation::Scalar
        && !allocation.relocations.is_empty()
        && (allocation.relocations.len() != 1
            || allocation.relocations[0].offset != MirByteOffset(0)
            || usize::from(allocation.relocations[0].width.0) != allocation.bytes.len())
    {
        return Err(MirConstantValidationError::new(
            format!("{path}.relocations"),
            "a relocated scalar must contain exactly one full-width relocation",
        ));
    }
    Ok(())
}

fn validate_acyclic(edges: &[Vec<usize>]) -> Result<(), MirConstantValidationError> {
    #[derive(Clone, Copy, Eq, PartialEq)]
    enum Visit {
        New,
        Active,
        Done,
    }

    fn visit(
        node: usize,
        depth: usize,
        edges: &[Vec<usize>],
        states: &mut [Visit],
    ) -> Result<(), MirConstantValidationError> {
        if depth > MAX_CONSTANT_GRAPH_DEPTH {
            return Err(MirConstantValidationError::new(
                format!("constants.allocation[{node}].relocations"),
                "allocation graph exceeds the maximum traversal depth",
            ));
        }
        match states[node] {
            Visit::Done => return Ok(()),
            Visit::Active => {
                return Err(MirConstantValidationError::new(
                    format!("constants.allocation[{node}].relocations"),
                    "allocation relocation graph contains a cycle",
                ));
            }
            Visit::New => {}
        }
        states[node] = Visit::Active;
        for &target in &edges[node] {
            visit(target, depth + 1, edges, states)?;
        }
        states[node] = Visit::Done;
        Ok(())
    }

    let mut states = vec![Visit::New; edges.len()];
    for node in 0..edges.len() {
        visit(node, 1, edges, &mut states)?;
    }
    Ok(())
}

fn write_allocation_text(output: &mut String, allocation: &MirConstantAllocation) {
    write!(output, "allocation(id={};origin=", allocation.id.0)
        .expect("writing to a String cannot fail");
    write_origin_text(output, &allocation.origin);
    output.push_str(";repr=");
    output.push_str(match allocation.representation {
        MirConstantRepresentation::Scalar => "scalar",
        MirConstantRepresentation::Aggregate => "aggregate",
    });
    write!(
        output,
        ";align={};addrspace={};mut={};bytes=",
        allocation.alignment.0,
        allocation.address_space.0,
        mutability_text(allocation.mutability)
    )
    .expect("writing to a String cannot fail");
    write_hex(output, &allocation.bytes);
    output.push_str(";init=");
    write_hex(output, &allocation.initialized.bits);
    output.push_str(";relocs=[");
    for (index, relocation) in allocation.relocations.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(
            output,
            "reloc(offset={};width={};provenance=",
            relocation.offset.0, relocation.width.0
        )
        .expect("writing to a String cannot fail");
        write_provenance_text(output, &relocation.provenance);
        write!(
            output,
            ";target_offset={};addrspace={};mut={})",
            relocation.target_offset.0,
            relocation.address_space.0,
            mutability_text(relocation.mutability)
        )
        .expect("writing to a String cannot fail");
    }
    output.push_str("])");
}

fn write_origin_text(output: &mut String, origin: &MirAllocationOrigin) {
    match origin {
        MirAllocationOrigin::Constant(identity) => {
            output.push_str("const(");
            write_name(output, &identity.0);
            output.push(')');
        }
        MirAllocationOrigin::Promoted(identity) => {
            output.push_str("promoted(owner=");
            write_name(output, &identity.owner.0);
            write!(output, ";index={})", identity.index).expect("writing to a String cannot fail");
        }
        MirAllocationOrigin::Static(identity) => {
            output.push_str("static(");
            write_name(output, &identity.0);
            output.push(')');
        }
        MirAllocationOrigin::Memory(identity) => {
            output.push_str("memory(");
            write_name(output, &identity.0);
            output.push(')');
        }
    }
}

fn write_provenance_text(output: &mut String, provenance: &MirPointerProvenance) {
    match provenance {
        MirPointerProvenance::Allocation(id) => {
            write!(output, "allocation({})", id.0).expect("writing to a String cannot fail");
        }
        MirPointerProvenance::Static(identity) => {
            output.push_str("static(");
            write_name(output, &identity.0);
            output.push(')');
        }
        MirPointerProvenance::Function(identity) => {
            output.push_str("function(");
            write_name(output, &identity.0);
            output.push(')');
        }
        MirPointerProvenance::VTable(identity) => {
            output.push_str("vtable(");
            write_name(output, &identity.0);
            output.push(')');
        }
        MirPointerProvenance::ThreadLocal(identity) => {
            output.push_str("tls(");
            write_name(output, &identity.0);
            output.push(')');
        }
        MirPointerProvenance::Unknown(tag) => {
            write!(output, "unknown({tag})").expect("writing to a String cannot fail");
        }
    }
}

fn mutability_text(mutability: MirMutability) -> &'static str {
    match mutability {
        MirMutability::Immutable => "const",
        MirMutability::Mutable => "mut",
    }
}

fn write_name(output: &mut String, value: &str) {
    write!(output, "{}:{value}", value.len()).expect("writing to a String cannot fail");
}

fn write_hex(output: &mut String, bytes: &[u8]) {
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
}

fn encode_allocation(writer: &mut Writer, allocation: &MirConstantAllocation) {
    writer.u32(allocation.id.0);
    match &allocation.origin {
        MirAllocationOrigin::Constant(identity) => {
            writer.u8(1);
            writer.string(&identity.0);
        }
        MirAllocationOrigin::Promoted(identity) => {
            writer.u8(2);
            writer.string(&identity.owner.0);
            writer.u32(identity.index);
        }
        MirAllocationOrigin::Static(identity) => {
            writer.u8(3);
            writer.string(&identity.0);
        }
        MirAllocationOrigin::Memory(identity) => {
            writer.u8(4);
            writer.string(&identity.0);
        }
    }
    writer.u8(match allocation.representation {
        MirConstantRepresentation::Scalar => 1,
        MirConstantRepresentation::Aggregate => 2,
    });
    writer.u64(allocation.alignment.0);
    writer.u32(allocation.address_space.0);
    writer.u8(encode_mutability(allocation.mutability));
    writer.blob(&allocation.bytes);
    writer.u64(allocation.initialized.byte_len);
    writer.blob(&allocation.initialized.bits);
    writer.u32(u32::try_from(allocation.relocations.len()).expect("bounded relocation count"));
    for relocation in &allocation.relocations {
        writer.u64(relocation.offset.0);
        writer.u8(relocation.width.0);
        match &relocation.provenance {
            MirPointerProvenance::Allocation(id) => {
                writer.u8(1);
                writer.u32(id.0);
            }
            MirPointerProvenance::Static(identity) => {
                writer.u8(2);
                writer.string(&identity.0);
            }
            MirPointerProvenance::Function(identity) => {
                writer.u8(3);
                writer.string(&identity.0);
            }
            MirPointerProvenance::VTable(identity) => {
                writer.u8(4);
                writer.string(&identity.0);
            }
            MirPointerProvenance::ThreadLocal(identity) => {
                writer.u8(5);
                writer.string(&identity.0);
            }
            MirPointerProvenance::Unknown(tag) => {
                writer.u8(6);
                writer.u32(*tag);
            }
        }
        writer.u64(relocation.target_offset.0);
        writer.u32(relocation.address_space.0);
        writer.u8(encode_mutability(relocation.mutability));
    }
}

fn decode_allocation(
    reader: &mut Reader<'_>,
) -> Result<MirConstantAllocation, MirConstantDecodeError> {
    let id = MirAllocationId(reader.u32()?);
    let origin = match reader.u8()? {
        1 => MirAllocationOrigin::Constant(MirConstantIdentity(reader.string()?)),
        2 => MirAllocationOrigin::Promoted(MirPromotedIdentity {
            owner: MirConstantIdentity(reader.string()?),
            index: reader.u32()?,
        }),
        3 => MirAllocationOrigin::Static(MirStaticIdentity(reader.string()?)),
        4 => MirAllocationOrigin::Memory(MirMemoryIdentity(reader.string()?)),
        tag => {
            return Err(MirConstantDecodeError::UnknownTag {
                field: "allocation origin",
                tag,
            });
        }
    };
    let representation = match reader.u8()? {
        1 => MirConstantRepresentation::Scalar,
        2 => MirConstantRepresentation::Aggregate,
        tag => {
            return Err(MirConstantDecodeError::UnknownTag {
                field: "constant representation",
                tag,
            });
        }
    };
    let alignment = MirAlignment(reader.u64()?);
    let address_space = MirAddressSpace(reader.u32()?);
    let mutability = decode_mutability(reader.u8()?)?;
    let bytes = reader.blob(MAX_CONSTANT_ALLOCATION_BYTES, "allocation byte length")?;
    let byte_len = reader.u64()?;
    let mask_limit = MAX_CONSTANT_ALLOCATION_BYTES.div_ceil(8);
    let bits = reader.blob(mask_limit, "initialized mask byte length")?;
    let relocation_count = reader.bounded_count(MAX_CONSTANT_RELOCATIONS, "relocation count")?;
    let mut relocations = Vec::with_capacity(relocation_count);
    for _ in 0..relocation_count {
        let offset = MirByteOffset(reader.u64()?);
        let width = MirPointerWidth(reader.u8()?);
        let provenance = match reader.u8()? {
            1 => MirPointerProvenance::Allocation(MirAllocationId(reader.u32()?)),
            2 => MirPointerProvenance::Static(MirStaticIdentity(reader.string()?)),
            3 => MirPointerProvenance::Function(MirSymbolIdentity(reader.string()?)),
            4 => MirPointerProvenance::VTable(MirSymbolIdentity(reader.string()?)),
            5 => MirPointerProvenance::ThreadLocal(MirStaticIdentity(reader.string()?)),
            6 => MirPointerProvenance::Unknown(reader.u32()?),
            tag => {
                return Err(MirConstantDecodeError::UnknownTag {
                    field: "pointer provenance",
                    tag,
                });
            }
        };
        relocations.push(MirPointerRelocation {
            offset,
            width,
            provenance,
            target_offset: MirByteOffset(reader.u64()?),
            address_space: MirAddressSpace(reader.u32()?),
            mutability: decode_mutability(reader.u8()?)?,
        });
    }
    Ok(MirConstantAllocation {
        id,
        origin,
        representation,
        bytes,
        initialized: MirInitializedMask { byte_len, bits },
        alignment,
        address_space,
        mutability,
        relocations,
    })
}

fn encode_mutability(mutability: MirMutability) -> u8 {
    match mutability {
        MirMutability::Immutable => 1,
        MirMutability::Mutable => 2,
    }
}

fn decode_mutability(tag: u8) -> Result<MirMutability, MirConstantDecodeError> {
    match tag {
        1 => Ok(MirMutability::Immutable),
        2 => Ok(MirMutability::Mutable),
        tag => Err(MirConstantDecodeError::UnknownTag {
            field: "mutability",
            tag,
        }),
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn blob(&mut self, value: &[u8]) {
        self.u32(u32::try_from(value.len()).expect("validated constant blob length"));
        self.bytes.extend_from_slice(value);
    }

    fn string(&mut self, value: &str) {
        self.blob(value.as_bytes());
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], MirConstantDecodeError> {
        if self.remaining.len() < count {
            return Err(MirConstantDecodeError::UnexpectedEnd);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, MirConstantDecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, MirConstantDecodeError> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .expect("reader returned exactly two bytes"),
        ))
    }

    fn u32(&mut self) -> Result<u32, MirConstantDecodeError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .expect("reader returned exactly four bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, MirConstantDecodeError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .expect("reader returned exactly eight bytes"),
        ))
    }

    fn bounded_count(
        &mut self,
        maximum: usize,
        name: &'static str,
    ) -> Result<usize, MirConstantDecodeError> {
        let count = usize::try_from(self.u32()?)
            .map_err(|_| MirConstantDecodeError::LimitExceeded(name))?;
        if count > maximum {
            return Err(MirConstantDecodeError::LimitExceeded(name));
        }
        Ok(count)
    }

    fn blob(
        &mut self,
        maximum: usize,
        name: &'static str,
    ) -> Result<Vec<u8>, MirConstantDecodeError> {
        let length = self.bounded_count(maximum, name)?;
        Ok(self.take(length)?.to_vec())
    }

    fn string(&mut self) -> Result<String, MirConstantDecodeError> {
        let bytes = self.blob(MAX_CONSTANT_IDENTITY_BYTES, "identity byte length")?;
        String::from_utf8(bytes).map_err(|_| MirConstantDecodeError::InvalidUtf8)
    }
}
